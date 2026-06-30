//! Reminder scheduling.
//!
//! When journal settings say `reminder.enabled = true` AND at least one
//! day is selected in `reminder.days_of_week`, [`restart_reminder_task`]
//! starts a long-running async task that:
//!   1. Computes the next occurrence of `(any day in days_of_week, hour, minute)`
//!      in local time — the soonest match across all selected days
//!   2. Sleeps in short chunks (≤ 5 min each), re-reading the wall clock
//!      between chunks (see "Chunked sleep design" below)
//!   3. Fires a notification — on macOS via `UNUserNotificationCenter` (action
//!      buttons + persistent until interacted with); on other platforms via
//!      `tauri-plugin-notification` as a fallback
//!   4. Sleeps a minute (so we don't immediately fire again within the same wall-clock minute)
//!   5. Loops forever (until the app shuts down)
//!
//! ## Chunked sleep design
//!
//! `tokio::time::sleep` is backed by a monotonic clock (`Instant`) which on
//! macOS PAUSES while the system is asleep. A naive `sleep(target - now)`
//! scheduled for Friday 6pm that gets slept-through over the weekend wakes
//! at "Friday 6pm + actual awake time elapsed" — i.e. Monday morning — and
//! fires the reminder for the wrong slot, hours or days late.
//!
//! Mitigation: sleep at most [`MAX_SLEEP_CHUNK`] at a time, then re-check
//! `Local::now()` against the target. On wake from a long system sleep the
//! next chunk completes near-instantly, the wall-clock check finds itself
//! past the target, and the reminder fires within ~5 min worst case.
//!
//! When the actual fire time is more than [`LATE_FIRE_THRESHOLD`] past the
//! target (typical sleep-through case) we append a "you missed the
//! {weekday} slot" suffix so the user understands why the notification is
//! arriving outside its scheduled window.
//!
//! ## Limitations (Phase 3 polish)
//!
//! - Doesn't survive across app restarts in the sense that nothing fires while
//!   the app is closed — macOS-scheduled notifications would be needed for that
//! - Reacts to settings changes via `restart_reminder_task` (called from
//!   `commands::complete_first_run` and `commands::update_settings`)

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration as StdDuration;

use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, TimeZone, Weekday};
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter};
#[cfg(not(target_os = "macos"))]
use tauri_plugin_notification::NotificationExt;

use crate::settings::ReminderSettings;

/// Max single `tokio::time::sleep` duration in the reminder loop. Bounds the
/// worst-case lag between wake-from-system-sleep and the wall-clock recheck
/// that triggers the actual fire. See "Chunked sleep design" in the module doc.
const MAX_SLEEP_CHUNK: StdDuration = StdDuration::from_secs(5 * 60);

/// Gap (actual fire time − target time) above which we treat the fire as
/// "late" and append a missed-slot suffix to the notification body. Stored
/// in seconds so the const is usable (`chrono::Duration` constructors aren't
/// `const fn`); convert via `Duration::seconds(LATE_FIRE_THRESHOLD_SECS)` at
/// use sites.
const LATE_FIRE_THRESHOLD_SECS: i64 = 30 * 60;

/// PNG used as the notification icon (Prodigy RPG `ui-raster-icons/scroll.png`).
/// Embedded into the binary so we don't depend on bundle-resource path resolution
/// behaving differently in dev vs production builds.
const NOTIFICATION_ICON_PNG: &[u8] = include_bytes!("../icons/notification-scroll.png");

/// Write the embedded notification icon to the OS temp directory (idempotent)
/// and return its absolute path. macOS's notification API wants a file path,
/// not raw bytes — writing once to a stable temp location is the simplest
/// way to give it one without fighting Tauri's `BaseDirectory` resolution.
fn notification_icon_path() -> Option<PathBuf> {
    let path = std::env::temp_dir().join("captainslog-notification-scroll.png");
    if !path.exists() {
        if let Err(e) = std::fs::write(&path, NOTIFICATION_ICON_PNG) {
            eprintln!("[reminders] failed to write notification icon: {e}");
            return None;
        }
    }
    Some(path)
}

/// Tauri-managed state holding the currently-running reminder task. Lets
/// commands (e.g. `complete_first_run`, future settings-save) cancel and
/// re-spawn the scheduler without a binary restart.
///
/// Internally a `std::sync::Mutex` because the lock is only ever held
/// briefly to swap the handle — never across `.await`.
pub struct ReminderHandle {
    inner: Mutex<Option<JoinHandle<()>>>,
}

impl ReminderHandle {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(None),
        }
    }
}

impl Default for ReminderHandle {
    fn default() -> Self {
        Self::new()
    }
}

/// Map 0..=6 to a chrono Weekday, where 0 = Monday (ISO convention).
/// Out-of-range values fall back to Friday — matches the default settings.
fn day_of_week_to_weekday(day_of_week: u8) -> Weekday {
    match day_of_week {
        0 => Weekday::Mon,
        1 => Weekday::Tue,
        2 => Weekday::Wed,
        3 => Weekday::Thu,
        4 => Weekday::Fri,
        5 => Weekday::Sat,
        6 => Weekday::Sun,
        _ => Weekday::Fri,
    }
}

/// Resolve a naive (year/month/day/hour/minute) into a concrete
/// `DateTime<Local>`. Handles the two DST-edge cases that occur once a
/// year for jurisdictions that observe daylight saving:
///
/// - **Spring-forward (gap)**: the local time doesn't exist (e.g. US
///   `02:30` on the second Sunday of March). chrono returns
///   `LocalResult::None`. We return `None` here too — the caller's
///   responsibility is to bump by a calendar week, preserving the
///   user's chosen weekday. (Earlier shape advanced one day, which
///   meant a "Sunday only" reminder could fire on Monday — silently
///   violating the user's day-of-week selection.)
/// - **Fall-back (ambiguous)**: the local time occurs twice (e.g. US
///   `01:30` on the first Sunday of November). chrono returns
///   `LocalResult::Ambiguous(earliest, latest)`. We pick the EARLIEST
///   (the pre-fall-back instant). Matches what most cron-like systems
///   do and is the conservative choice — fires sooner rather than
///   later, and stays on the user's chosen weekday.
///
/// Returns `None` for both the gap case and the un-constructable date
/// case (e.g. February 31, which can't happen from real-date
/// arithmetic but is defensive).
fn resolve_local_datetime(
    year: i32,
    month: u32,
    day: u32,
    hour: u8,
    minute: u8,
) -> Option<DateTime<Local>> {
    let date = NaiveDate::from_ymd_opt(year, month, day)?;
    let naive = date.and_hms_opt(hour as u32, minute as u32, 0)?;
    match Local.from_local_datetime(&naive) {
        chrono::LocalResult::Single(dt) => Some(dt),
        chrono::LocalResult::Ambiguous(earliest, _) => Some(earliest),
        chrono::LocalResult::None => None,
    }
}

/// Compute the next time the reminder should fire for a SINGLE weekday,
/// strictly in the future. Helper for the multi-day variant.
///
/// Uses naive-date arithmetic + per-candidate local resolution rather
/// than `Duration::days(N)` arithmetic on a `DateTime<Local>`. The
/// older shape stayed in DateTime<Local> the whole way and silently
/// drifted by 1 hour across DST transitions (Duration::days is a fixed
/// 86,400s; calendar days vary across spring-forward and fall-back).
///
/// On a DST gap (target hour:minute doesn't exist on the target date),
/// we BUMP BY 7 DAYS rather than advancing to the next calendar day —
/// the user picked a specific weekday and we must respect that. DST
/// gaps recur at most once a year per jurisdiction, so +7 days from
/// any gap day always lands on a non-gap day. We cap the loop at 53
/// iterations as a hard backstop against an unbounded climb.
fn next_reminder_time_for_day(
    now: DateTime<Local>,
    day_of_week: u8,
    hour: u8,
    minute: u8,
) -> DateTime<Local> {
    let target_weekday = day_of_week_to_weekday(day_of_week);

    // 0..=6 days from `now` to the target weekday (0 means "today").
    let now_dow = now.weekday().num_days_from_monday() as i64;
    let target_dow = target_weekday.num_days_from_monday() as i64;
    let days_until = (target_dow - now_dow + 7) % 7;

    let now_date = now.date_naive();
    let mut target_date = now_date + Duration::days(days_until);

    // Walk forward by 7-day strides looking for the first occurrence
    // that (a) resolves cleanly in local time and (b) is strictly in
    // the future. The bound of 53 covers one year + 1 week of buffer
    // — DST gaps happen at most once per year, so we should never
    // need more than 2 iterations in practice.
    for _ in 0..53 {
        if let Some(candidate) = resolve_local_datetime(
            target_date.year(),
            target_date.month(),
            target_date.day(),
            hour,
            minute,
        ) {
            if candidate > now {
                return candidate;
            }
        }
        target_date += Duration::days(7);
    }

    // Unreachable for any realistic input — keeps the return type free
    // of Option for the common path. If a malformed input ever reached
    // here, panicking is the right loud-failure mode.
    panic!(
        "no valid reminder fire time within 53 weeks for day={day_of_week} hour={hour} minute={minute}"
    );
}

/// Compute the soonest reminder fire time across all selected days,
/// strictly in the future. Returns `None` when `days_of_week` is empty
/// (reminder is enabled-but-has-no-days, which is a configured no-op).
///
/// Pure function so we can unit-test the time math without spawning
/// tasks. The set of days is treated as an unordered collection — order
/// + duplicates don't affect the result.
pub fn next_reminder_time_after(
    now: DateTime<Local>,
    days_of_week: &[u8],
    hour: u8,
    minute: u8,
) -> Option<DateTime<Local>> {
    days_of_week
        .iter()
        .map(|&d| next_reminder_time_for_day(now, d, hour, minute))
        .min()
}

/// Convenience wrapper for the scheduler.
pub fn next_reminder_time(
    days_of_week: &[u8],
    hour: u8,
    minute: u8,
) -> Option<DateTime<Local>> {
    next_reminder_time_after(Local::now(), days_of_week, hour, minute)
}

/// Human weekday name for the missed-slot suffix ("Monday" not "Mon").
fn weekday_long_name(weekday: Weekday) -> &'static str {
    match weekday {
        Weekday::Mon => "Monday",
        Weekday::Tue => "Tuesday",
        Weekday::Wed => "Wednesday",
        Weekday::Thu => "Thursday",
        Weekday::Fri => "Friday",
        Weekday::Sat => "Saturday",
        Weekday::Sun => "Sunday",
    }
}

/// Build the notification body, appending a missed-slot suffix when the
/// fire is more than [`LATE_FIRE_THRESHOLD_SECS`] past the scheduled target.
/// Typical trigger: system slept through the target, the chunked-sleep loop
/// caught up on wake.
fn build_notification_body(
    greeting: &str,
    target: DateTime<Local>,
    fired_at: DateTime<Local>,
) -> String {
    let base = format!("Time to log this week's summary, {greeting}.");
    let lag = fired_at - target;
    if lag > Duration::seconds(LATE_FIRE_THRESHOLD_SECS) {
        format!(
            "{base} — this is the {} slot you missed",
            weekday_long_name(target.weekday())
        )
    } else {
        base
    }
}

/// Cancel any running reminder task and start a fresh one with the new config.
///
/// When `config.enabled` is `false`, any existing task is still aborted —
/// the net effect is "stop reminders." When `true`, the new task supersedes
/// whatever was running.
///
/// Called from:
/// - `lib::run::setup()` on app launch (initial spawn from disk settings)
/// - `commands::complete_first_run` after the wizard saves new settings
/// - `commands::update_settings` after a Settings-panel save
pub fn restart_reminder_task(
    app: AppHandle,
    handle: &ReminderHandle,
    config: ReminderSettings,
    user_name: Option<String>,
) {
    let mut slot = handle
        .inner
        .lock()
        .expect("reminder handle mutex was poisoned");

    if let Some(old) = slot.take() {
        old.abort();
        println!("[reminders] previous task aborted");
    }

    if !config.enabled {
        println!("[reminders] disabled — no task scheduled");
        return;
    }
    if config.days_of_week.is_empty() {
        // Configured-but-has-no-days. The Settings UI surfaces an
        // "enabled but no days selected" hint near the multi-day picker,
        // so this isn't a silent failure as far as the user is concerned.
        println!("[reminders] enabled but no days selected — nothing to schedule");
        return;
    }

    let new_handle = tauri::async_runtime::spawn(async move {
        loop {
            let Some(target) = next_reminder_time(
                &config.days_of_week,
                config.hour,
                config.minute,
            ) else {
                // Should be unreachable given the empty-check above, but
                // keep the loop robust: sleep an hour and try again rather
                // than spin-looping.
                eprintln!(
                    "[reminders] next_reminder_time returned None despite non-empty days; sleeping 1h"
                );
                tokio::time::sleep(StdDuration::from_secs(3600)).await;
                continue;
            };

            println!(
                "[reminders] next fire at {} (chunked-sleep, max {}s per chunk)",
                target.format("%Y-%m-%d %H:%M:%S %z"),
                MAX_SLEEP_CHUNK.as_secs()
            );

            // Chunked sleep: re-read the wall clock between chunks so a
            // long system sleep can't carry us silently past the target.
            // See module-level "Chunked sleep design" comment.
            loop {
                let now = Local::now();
                if now >= target {
                    break;
                }
                let remaining = (target - now).to_std().unwrap_or(StdDuration::ZERO);
                let chunk = remaining.min(MAX_SLEEP_CHUNK);
                tokio::time::sleep(chunk).await;
            }

            let fired_at = Local::now();
            let greeting = user_name.as_deref().unwrap_or("Captain");
            let body = build_notification_body(greeting, target, fired_at);
            let icon_path = notification_icon_path();

            fire_notification(&app, &body, icon_path.as_deref()).await;

            println!(
                "[reminders] fired at {} (target was {}, lag {}s)",
                fired_at.format("%H:%M:%S"),
                target.format("%H:%M:%S"),
                (fired_at - target).num_seconds()
            );

            // Sleep a minute so the next iteration doesn't recompute "now" inside
            // the same target minute and re-fire immediately.
            tokio::time::sleep(StdDuration::from_secs(60)).await;
        }
    });

    *slot = Some(new_handle);
}

// ---------------------------------------------------------------------------
// Fire-the-notification (platform-specific)
// ---------------------------------------------------------------------------

/// Identifier for the "Write" action — comes back in NotificationResponse.action_identifier.
#[cfg(target_os = "macos")]
const WRITE_ACTION: &str = "WRITE";
/// Identifier for the "OK" / dismiss action.
#[cfg(target_os = "macos")]
const OK_ACTION: &str = "OK";

/// True when the running process is launched from inside an `.app` bundle.
/// `UNUserNotificationCenter.current()` calls `bundleProxyForCurrentProcess`
/// internally and aborts the process when no real `.app` bundle is registered
/// with LaunchServices (NSBundle.bundleIdentifier swizzling doesn't help —
/// it's a different lookup path). So we gate the UN code path on this check.
///
/// A real .app path looks like `.../Foo.app/Contents/MacOS/Foo`. `tauri dev`
/// runs the bare binary from `target/debug/`, which doesn't match.
#[cfg(target_os = "macos")]
fn is_running_in_app_bundle() -> bool {
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return false,
    };
    let Some(macos_dir) = exe.parent() else { return false };
    if macos_dir.file_name().and_then(|s| s.to_str()) != Some("MacOS") {
        return false;
    }
    let Some(contents_dir) = macos_dir.parent() else { return false };
    if contents_dir.file_name().and_then(|s| s.to_str()) != Some("Contents") {
        return false;
    }
    let Some(app_dir) = contents_dir.parent() else { return false };
    app_dir.extension().and_then(|s| s.to_str()) == Some("app")
}

/// Display the weekly reminder.
///
/// On macOS, the dispatch is hybrid:
///   - **Running from a real `.app` bundle** (production / `tauri build`):
///     use `UNUserNotificationCenter` via `mac-usernotifications`. Action
///     buttons, persistence, modern API.
///   - **Running from a bare binary** (`tauri dev`): fall back to the
///     deprecated `NSUserNotification` via `mac-notification-sys`. No action
///     buttons in this mode (banner auto-dismiss), but no crash either —
///     UN's `currentNotificationCenter()` aborts when LaunchServices can't
///     find a bundle proxy for the running process, and swizzling NSBundle
///     isn't enough to satisfy that lookup.
///
/// On other platforms, falls back to `tauri-plugin-notification`.
#[cfg(target_os = "macos")]
async fn fire_notification(
    app: &AppHandle,
    body: &str,
    icon_path: Option<&std::path::Path>,
) {
    if is_running_in_app_bundle() {
        fire_via_un(app, body, icon_path).await;
    } else {
        fire_via_nsuser_notification(body, icon_path);
    }
}

/// Modern path — UNUserNotificationCenter via mac-usernotifications.
/// Only safe to call when running from a real `.app` bundle.
#[cfg(target_os = "macos")]
async fn fire_via_un(
    app: &AppHandle,
    body: &str,
    icon_path: Option<&std::path::Path>,
) {
    use mac_usernotifications::{Action, InterruptionLevel, Notification};

    let mut builder = Notification::default()
        .title("Captain's Log")
        .message(body)
        .action(Action::button(WRITE_ACTION, "Write"))
        .action(Action::button(OK_ACTION, "OK"))
        .interruption_level(InterruptionLevel::Active)
        .default_sound()
        .timeout(StdDuration::from_secs(24 * 60 * 60));

    if let Some(path) = icon_path {
        builder = builder.image_path(path.to_string_lossy());
    }

    match builder.send().await {
        Ok(sent) => {
            println!(
                "[reminders] UN notification sent (id: {})",
                sent.notification_id()
            );

            // Don't block the scheduler waiting for the user's response —
            // spawn a task to listen and act on the click whenever it arrives.
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                match sent.response().await {
                    Ok(response) => {
                        if response.action_identifier == WRITE_ACTION
                            || response.is_default_action()
                        {
                            println!("[reminders] user clicked Write");
                            open_summary(&app);
                        } else if response.action_identifier == OK_ACTION {
                            println!("[reminders] user clicked OK");
                        } else if response.is_dismiss_action() {
                            println!("[reminders] user dismissed the notification");
                        } else if response.is_timed_out() {
                            println!("[reminders] response wait timed out");
                        } else {
                            println!(
                                "[reminders] unhandled response: {}",
                                response.action_identifier
                            );
                        }
                    }
                    Err(e) => {
                        eprintln!("[reminders] response error: {e}");
                    }
                }
            });
        }
        Err(e) => {
            eprintln!("[reminders] UN send failed: {e}");
        }
    }
}

/// Dev-mode fallback — NSUserNotification via mac-notification-sys.
/// Deprecated API but works from a bare binary, which UN can't.
///
/// Fire-and-forget; we don't try to wait for action-button responses here
/// because the deprecated path doesn't render them reliably anyway, and
/// dev-mode testing usually just needs "did the notification fire" not
/// "did the click handler navigate." Test the full UN experience by
/// running a bundled build (`npm run tauri build -- --debug` then launch
/// `target/debug/bundle/macos/Captain's Log.app`).
#[cfg(target_os = "macos")]
fn fire_via_nsuser_notification(body: &str, icon_path: Option<&std::path::Path>) {
    use mac_notification_sys::{send_notification, Notification};

    let mut notification = Notification::new();
    let icon_string;
    if let Some(path) = icon_path {
        icon_string = path.to_string_lossy().into_owned();
        notification.app_icon(&icon_string);
    }

    if let Err(e) = send_notification("Captain's Log", None, body, Some(&notification)) {
        eprintln!("[reminders] NSUserNotification send failed: {e}");
    } else {
        println!("[reminders] dev mode: NSUserNotification fired (no action buttons)");
    }
}

#[cfg(not(target_os = "macos"))]
async fn fire_notification(
    app: &AppHandle,
    body: &str,
    icon_path: Option<&std::path::Path>,
) {
    let mut builder = app
        .notification()
        .builder()
        .title("Captain's Log")
        .body(body);

    if let Some(icon_path) = icon_path {
        builder = builder.icon(icon_path.to_string_lossy().into_owned());
    }

    if let Err(e) = builder.show() {
        eprintln!("[reminders] notification failed: {e}");
    }
}

/// Bring the main window to the foreground and tell the frontend to navigate
/// to the weekly summary page. Called when the user clicks the notification's
/// "Write" action button (or the default action, same intent).
///
/// Routes through `crate::restore_main_window` which flips the activation
/// policy back to `.Regular` BEFORE showing — so a notification click while
/// the app is in `.Accessory` mode brings the Dock icon back. Without this,
/// the main window would appear but the app would stay Dock-less and
/// Cmd-Tab-invisible.
#[cfg(target_os = "macos")]
fn open_summary(app: &AppHandle) {
    crate::restore_main_window(app);
    let _ = app.emit("open-summary", ());
}

/// Set up the macOS bundle identity needed by both notification paths.
///
/// `mac_notification_sys::set_application` swizzles
/// `-[NSBundle bundleIdentifier]` to return our id — required for the
/// `NSUserNotification` dev-mode fallback to deliver notifications when
/// running from a bare binary.
///
/// We do NOT call `mac_usernotifications::check_bundle()` here unconditionally.
/// UN's bundle check passes because of the NSBundle swizzle, but the actual
/// notification path uses `UNUserNotificationCenter.current()` which reaches
/// into `bundleProxyForCurrentProcess` / LaunchServices and aborts the
/// process when called from a bare binary. So we only verify UN identity
/// when running from a real `.app`.
///
/// No-op on other platforms.
#[cfg(target_os = "macos")]
pub fn check_macos_bundle() {
    const BUNDLE_ID: &str = "com.prodigygame.captainslog";
    if is_running_in_app_bundle() {
        // Real .app — the on-disk Info.plist provides CFBundleIdentifier and
        // the codesign identifier matches it (via tauri.conf.json's
        // bundle.macOS.signingIdentity). DO NOT swizzle here — adding a
        // second source of bundle-id truth inside a properly-signed bundle
        // causes usernotificationsd to silently deny auth requests as a
        // suspected bundle-id spoof.
        match mac_usernotifications::check_bundle() {
            Ok(()) => println!("[reminders] bundled .app — UN path active ({BUNDLE_ID})"),
            Err(e) => eprintln!(
                "[reminders] UN bundle check unexpectedly failed in .app: {e}"
            ),
        }
    } else {
        // Bare binary (`tauri dev`) — NSUserNotification fallback. The
        // swizzle is required here so NSBundle.bundleIdentifier returns
        // something for the NS API path. UN is NOT used in this mode (it
        // would abort the process), so the swizzle can't interfere with it.
        if let Err(e) = mac_notification_sys::set_application(BUNDLE_ID) {
            eprintln!("[reminders] bundle-id swizzle failed: {e}");
        }
        println!(
            "[reminders] bare binary (dev mode) — NSUserNotification fallback. \
             For full UN experience build a debug bundle: \
             `npm run tauri build -- --debug` then launch the .app from \
             target/debug/bundle/macos/"
        );
    }
}

#[cfg(not(target_os = "macos"))]
pub fn check_macos_bundle() {}

/// Request notification authorization on macOS. Idempotent — the first call
/// shows the system prompt; subsequent calls return immediately with the
/// remembered choice.
///
/// Only meaningful (and only safe) when running from a real `.app` bundle —
/// UN's `current()` call aborts the process from a bare binary. In dev mode
/// (bare binary), the NSUserNotification fallback doesn't have a permission
/// concept, so we skip the prompt entirely.
///
/// No-op on other platforms.
#[cfg(target_os = "macos")]
pub async fn request_notification_authorization() {
    if !is_running_in_app_bundle() {
        println!("[reminders] dev mode: skipping UN auth request (NS fallback doesn't need it)");
        return;
    }
    match mac_usernotifications::request_auth().await {
        Ok(true) => println!("[reminders] notification permission granted"),
        Ok(false) => eprintln!(
            "[reminders] notification permission denied — reminders won't fire \
             until granted via System Settings > Notifications > Captain's Log"
        ),
        Err(e) => eprintln!("[reminders] auth request failed: {e}"),
    }
}

#[cfg(not(target_os = "macos"))]
pub async fn request_notification_authorization() {}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Timelike};

    fn local(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> DateTime<Local> {
        Local
            .with_ymd_and_hms(y, mo, d, h, mi, 0)
            .single()
            .expect("valid datetime")
    }

    /// 2026-06-22 is a Monday. Anchor most tests on this date for clarity.
    fn monday_noon() -> DateTime<Local> {
        local(2026, 6, 22, 12, 0)
    }

    /// Convenience for single-day tests — wraps the day in a slice and
    /// unwraps the Option (panics on empty input, which is fine for tests
    /// that always pass exactly one day).
    fn single(
        now: DateTime<Local>,
        day_of_week: u8,
        hour: u8,
        minute: u8,
    ) -> DateTime<Local> {
        next_reminder_time_after(now, &[day_of_week], hour, minute)
            .expect("single-day input must produce a fire time")
    }

    // ---- Same-week future slot ----

    #[test]
    fn friday_4pm_from_monday_noon_is_this_week() {
        // Monday noon -> next Friday at 4pm should be in the same calendar week.
        let target = single(monday_noon(), 4, 16, 0);
        assert_eq!(target.weekday(), Weekday::Fri);
        assert_eq!(target.hour(), 16);
        assert_eq!(target.minute(), 0);
        assert_eq!((target - monday_noon()).num_days(), 4);
    }

    #[test]
    fn same_day_later_today_returns_today() {
        // Monday noon -> reminder is Monday 6pm.
        let target = single(monday_noon(), 0, 18, 0);
        assert_eq!(target.weekday(), Weekday::Mon);
        assert_eq!(target.day(), 22);
        assert_eq!(target.hour(), 18);
    }

    // ---- Slot already passed today ----

    #[test]
    fn same_day_earlier_today_returns_next_week() {
        // Monday noon -> reminder is Monday 9am (already passed today).
        let target = single(monday_noon(), 0, 9, 0);
        assert_eq!(target.weekday(), Weekday::Mon);
        assert_eq!(target.day(), 29); // next Monday
        assert_eq!(target.hour(), 9);
    }

    #[test]
    fn yesterday_in_iso_week_returns_next_week_not_yesterday() {
        // Tuesday noon -> reminder is Monday 9am.
        let tue_noon = local(2026, 6, 23, 12, 0);
        let target = single(tue_noon, 0, 9, 0);
        assert_eq!(target.weekday(), Weekday::Mon);
        // Should be NEXT Monday, not yesterday.
        assert!(target > tue_noon);
        assert_eq!(target.day(), 29);
    }

    // ---- Cross-week ----

    #[test]
    fn sunday_from_monday_noon_is_six_days_away() {
        // Monday noon -> reminder is Sunday 4pm.
        let target = single(monday_noon(), 6, 16, 0);
        assert_eq!(target.weekday(), Weekday::Sun);
        assert_eq!((target - monday_noon()).num_days(), 6);
    }

    // ---- Exact-minute boundary ----

    #[test]
    fn exact_same_minute_returns_next_week() {
        // If "now" is exactly at the target time with zero seconds, we treat
        // it as "passed" and schedule for next week. (Better than firing
        // immediately on app launch and feeling spammy.)
        let exact = local(2026, 6, 22, 16, 0);
        let target = single(exact, 0, 16, 0);
        assert_eq!(target.day(), 29);
    }

    #[test]
    fn target_with_seconds_remaining_returns_today() {
        // 12:00:00 now, target is 12:01. Should be today, ~1 min away.
        let now = local(2026, 6, 22, 12, 0);
        let target = single(now, 0, 12, 1);
        assert_eq!(target.day(), 22);
        assert_eq!(target.minute(), 1);
        assert_eq!((target - now).num_minutes(), 1);
    }

    // ---- Out-of-range day_of_week ----

    #[test]
    fn out_of_range_day_of_week_falls_back_to_friday() {
        let target = single(monday_noon(), 99, 16, 0);
        assert_eq!(target.weekday(), Weekday::Fri);
    }

    // ---- Always strictly in the future ----

    #[test]
    fn result_is_always_strictly_in_the_future() {
        let now = monday_noon();
        // Sample a bunch of inputs.
        for day in 0..7u8 {
            for hour in [0, 12, 23] {
                for minute in [0, 30, 59] {
                    let t = single(now, day, hour, minute);
                    assert!(
                        t > now,
                        "expected strictly future for day={day}, h={hour}, m={minute}: got {t} vs now {now}"
                    );
                }
            }
        }
    }

    // ---- Multi-day fire-time selection ----

    #[test]
    fn empty_days_of_week_returns_none() {
        let r = next_reminder_time_after(monday_noon(), &[], 16, 0);
        assert!(r.is_none(), "empty days_of_week should produce no fire time");
    }

    #[test]
    fn mwf_from_monday_noon_fires_wednesday() {
        // Mon/Wed/Fri at 4pm, evaluating from Monday at noon. Monday 4pm
        // hasn't passed yet — so Monday wins as the soonest target.
        let target = next_reminder_time_after(monday_noon(), &[0, 2, 4], 16, 0).unwrap();
        assert_eq!(target.weekday(), Weekday::Mon);
        assert_eq!(target.day(), 22);
    }

    #[test]
    fn mwf_from_monday_evening_fires_wednesday() {
        // Mon/Wed/Fri at 4pm, evaluating from Monday at 6pm. Monday 4pm
        // has passed — next is Wednesday.
        let mon_evening = local(2026, 6, 22, 18, 0);
        let target = next_reminder_time_after(mon_evening, &[0, 2, 4], 16, 0).unwrap();
        assert_eq!(target.weekday(), Weekday::Wed);
    }

    #[test]
    fn daily_reminder_fires_tomorrow_when_today_passed() {
        // All 7 days at 9am, evaluating from Monday noon. Today's 9am
        // passed -> next is Tuesday.
        let days: Vec<u8> = (0..=6).collect();
        let target = next_reminder_time_after(monday_noon(), &days, 9, 0).unwrap();
        assert_eq!(target.weekday(), Weekday::Tue);
        assert_eq!((target - monday_noon()).num_hours(), 21); // noon Mon → 9am Tue
    }

    #[test]
    fn daily_reminder_fires_today_when_today_still_future() {
        // All 7 days at 6pm, evaluating from Monday noon. Today's 6pm
        // still ahead -> fires today.
        let days: Vec<u8> = (0..=6).collect();
        let target = next_reminder_time_after(monday_noon(), &days, 18, 0).unwrap();
        assert_eq!(target.weekday(), Weekday::Mon);
        assert_eq!(target.day(), 22);
    }

    #[test]
    fn day_order_does_not_affect_result() {
        // The Set-of-days semantics should be order-independent. Same
        // input expressed two ways must produce the same answer.
        let now = monday_noon();
        let a = next_reminder_time_after(now, &[6, 0, 2, 4], 9, 0).unwrap();
        let b = next_reminder_time_after(now, &[0, 2, 4, 6], 9, 0).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn duplicate_days_do_not_affect_result() {
        // The serde shim dedupes on read; here we double-check that even
        // if duplicates leak in (hand-edited json, future bugs), the
        // result is unchanged.
        let now = monday_noon();
        let a = next_reminder_time_after(now, &[4], 16, 0).unwrap();
        let b = next_reminder_time_after(now, &[4, 4, 4], 16, 0).unwrap();
        assert_eq!(a, b);
    }

    // ---- DST safety ----
    //
    // These three tests use `chrono::Local`, which reads the system
    // timezone at runtime. On a DST-observing TZ (e.g. America/New_York
    // — the maintainer's machine) the gap/ambiguous branches of
    // `resolve_local_datetime` are exercised; on UTC or other non-DST
    // zones the tests pass trivially because every target time
    // resolves on the first try as LocalResult::Single.
    //
    // KNOWN COVERAGE GAP: a CI runner pinned to UTC won't catch a
    // future regression in the gap/ambiguous branches. Mitigation
    // would be a `chrono-tz` dev-dependency + tests pinned to
    // `America/New_York` — deferred until we actually have a CI host
    // where it matters.

    #[test]
    fn dst_gap_target_time_does_not_panic() {
        // 2026-03-08 is the US spring-forward Sunday — at 02:00 local
        // (on US-DST systems) the clock jumps to 03:00, so 02:30 is a
        // non-existent local time on that date. The old code path would
        // have panicked at the `.expect("hour/minute should be in
        // range")` because chrono's with_hour/with_minute returns None
        // for non-existent local datetimes. The new shape resolves to
        // the next valid local instant.
        let now = local(2026, 3, 7, 12, 0); // Saturday noon, pre-transition
        let result = next_reminder_time_after(now, &[6], 2, 30);
        assert!(
            result.is_some(),
            "scheduler must not panic when target time lands in a DST gap"
        );
    }

    #[test]
    fn dst_crossing_preserves_target_hour() {
        // The old code added Duration::days(N) — a fixed 86,400 seconds
        // per "day" — which drifted the wall-clock by ±1 hour across DST
        // transitions. The new code computes via naive-date arithmetic
        // and resolves the local time on the TARGET date, so the wall
        // clock the user picked is what they get.
        //
        // From Saturday 2026-03-07 noon, asking for next Sunday at 9am
        // must produce 9am local on March 8 — not 8am (fall-back error
        // direction) or 10am (spring-forward error direction).
        let now = local(2026, 3, 7, 12, 0);
        let target = next_reminder_time_after(now, &[6], 9, 0).unwrap();
        assert_eq!(target.hour(), 9, "wall-clock hour must survive DST crossing");
        assert_eq!(target.minute(), 0);
        // Should be the very next day.
        assert!((target - now).num_hours() < 30);
    }

    // ---- Late-fire / sleep-drift suffix ----
    //
    // These test the body-builder directly (not the loop) so we don't need
    // a fake-clock harness. The loop itself is a straightforward
    // "while now < target { sleep chunk; recheck }" — its correctness
    // reduces to the body builder + the existing next_reminder_time math.

    #[test]
    fn body_on_time_has_no_suffix() {
        // Target Friday 6pm, fired one minute later — well under the 30-min
        // threshold. No suffix.
        let target = local(2026, 6, 26, 18, 0); // Friday
        let fired = target + Duration::minutes(1);
        let body = build_notification_body("Chris", target, fired);
        assert_eq!(body, "Time to log this week's summary, Chris.");
    }

    #[test]
    fn body_just_under_threshold_has_no_suffix() {
        // 30 minutes exactly — NOT greater than threshold, so no suffix.
        let target = local(2026, 6, 26, 18, 0);
        let fired = target + Duration::minutes(30);
        let body = build_notification_body("Chris", target, fired);
        assert!(
            !body.contains("missed"),
            "30 min exactly is the threshold, must not flag as late: {body}"
        );
    }

    #[test]
    fn body_late_fire_appends_missed_slot_suffix() {
        // Target Friday 6pm, fired Monday morning (typical sleep-through
        // case). Body must call out the missed Friday slot.
        let target = local(2026, 6, 26, 18, 0); // Friday
        let fired = local(2026, 6, 29, 9, 0); // Monday morning
        let body = build_notification_body("Chris", target, fired);
        assert!(
            body.contains("Friday slot you missed"),
            "expected missed-Friday suffix in late-fire body: {body}"
        );
    }

    #[test]
    fn body_late_fire_uses_target_weekday_name() {
        // The weekday in the suffix comes from the TARGET, not the fire
        // time. A Wednesday slot fired Thursday afternoon should say
        // "Wednesday".
        let target = local(2026, 6, 24, 16, 0); // Wednesday
        let fired = local(2026, 6, 25, 15, 0); // Thursday, ~23h late
        let body = build_notification_body("Captain", target, fired);
        assert!(
            body.contains("Wednesday slot you missed"),
            "suffix must name the target weekday, got: {body}"
        );
    }

    #[test]
    fn body_just_over_threshold_appends_suffix() {
        // 31 minutes late — just past the threshold.
        let target = local(2026, 6, 26, 18, 0);
        let fired = target + Duration::minutes(31);
        let body = build_notification_body("Chris", target, fired);
        assert!(
            body.contains("missed"),
            "31 min past target must flag as late: {body}"
        );
    }

    #[test]
    fn fall_back_ambiguous_time_returns_a_concrete_instant() {
        // 2026-11-01 is the US fall-back Sunday — 01:30 occurs twice
        // (once in DST, once in standard time). The new resolver picks
        // the earlier of the two. We can't easily assert which without
        // a fixed TZ, but we CAN assert that the function returns a
        // single concrete instant and doesn't panic / hang.
        let now = local(2026, 10, 31, 12, 0); // Saturday noon, pre-fall-back
        let result = next_reminder_time_after(now, &[6], 1, 30);
        assert!(result.is_some());
        let target = result.unwrap();
        assert_eq!(target.hour(), 1);
        assert_eq!(target.minute(), 30);
    }
}
