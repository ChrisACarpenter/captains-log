//! Weekly reminder scheduling.
//!
//! When journal settings say `reminder.enabled = true`, [`restart_reminder_task`]
//! starts a long-running async task that:
//!   1. Computes the next occurrence of `(day_of_week, hour, minute)` in local time
//!   2. Sleeps until then
//!   3. Fires a notification — on macOS via `UNUserNotificationCenter` (action
//!      buttons + persistent until interacted with); on other platforms via
//!      `tauri-plugin-notification` as a fallback
//!   4. Sleeps a minute (so we don't immediately fire again within the same wall-clock minute)
//!   5. Loops forever (until the app shuts down)
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

use chrono::{DateTime, Datelike, Duration, Local, Timelike, Weekday};
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter, Manager};
#[cfg(not(target_os = "macos"))]
use tauri_plugin_notification::NotificationExt;

use crate::settings::ReminderSettings;

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

/// Compute the next time the reminder should fire, strictly in the future.
/// Pure function so we can unit-test the time math without spawning tasks.
pub fn next_reminder_time_after(
    now: DateTime<Local>,
    day_of_week: u8,
    hour: u8,
    minute: u8,
) -> DateTime<Local> {
    let target_weekday = day_of_week_to_weekday(day_of_week);

    // 0..=6 days from `now` to the target weekday (0 means "today").
    let now_dow = now.weekday().num_days_from_monday() as i64;
    let target_dow = target_weekday.num_days_from_monday() as i64;
    let mut days_until = (target_dow - now_dow + 7) % 7;

    let mut candidate = now
        .with_hour(hour as u32)
        .and_then(|d| d.with_minute(minute as u32))
        .and_then(|d| d.with_second(0))
        .and_then(|d| d.with_nanosecond(0))
        .expect("hour/minute should be in range 0..24, 0..60")
        + Duration::days(days_until);

    // If the candidate is in the past (today's slot already passed),
    // bump by a week so we're strictly in the future.
    if candidate <= now {
        candidate += Duration::days(7);
        days_until += 7;
        let _ = days_until; // silence unused warning
    }

    candidate
}

/// Wrapper for convenient calling from the scheduler.
pub fn next_reminder_time(day_of_week: u8, hour: u8, minute: u8) -> DateTime<Local> {
    next_reminder_time_after(Local::now(), day_of_week, hour, minute)
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
/// - future `update_settings` command (Phase 2 settings panel)
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

    let new_handle = tauri::async_runtime::spawn(async move {
        loop {
            let next = next_reminder_time(config.day_of_week, config.hour, config.minute);
            let now = Local::now();
            let delta = next - now;

            let duration = match delta.to_std() {
                Ok(d) => d,
                Err(_) => {
                    // Defensive: if we computed something in the past somehow,
                    // sleep a minute and retry.
                    eprintln!("reminder: non-positive duration; sleeping 60s and retrying");
                    tokio::time::sleep(StdDuration::from_secs(60)).await;
                    continue;
                }
            };

            println!(
                "[reminders] next fire at {} (in {} seconds)",
                next.format("%Y-%m-%d %H:%M:%S %z"),
                duration.as_secs()
            );

            tokio::time::sleep(duration).await;

            let greeting = user_name.as_deref().unwrap_or("Captain");
            let body = format!("Time to log this week's summary, {greeting}.");
            let icon_path = notification_icon_path();

            fire_notification(&app, &body, icon_path.as_deref()).await;

            println!("[reminders] fired at {}", Local::now().format("%H:%M:%S"));

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
#[cfg(target_os = "macos")]
fn open_summary(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
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
    use chrono::TimeZone;

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

    // ---- Same-week future slot ----

    #[test]
    fn friday_4pm_from_monday_noon_is_this_week() {
        // Monday noon -> next Friday at 4pm should be in the same calendar week.
        let target = next_reminder_time_after(monday_noon(), 4, 16, 0);
        assert_eq!(target.weekday(), Weekday::Fri);
        assert_eq!(target.hour(), 16);
        assert_eq!(target.minute(), 0);
        assert_eq!((target - monday_noon()).num_days(), 4);
    }

    #[test]
    fn same_day_later_today_returns_today() {
        // Monday noon -> reminder is Monday 6pm.
        let target = next_reminder_time_after(monday_noon(), 0, 18, 0);
        assert_eq!(target.weekday(), Weekday::Mon);
        assert_eq!(target.day(), 22);
        assert_eq!(target.hour(), 18);
    }

    // ---- Slot already passed today ----

    #[test]
    fn same_day_earlier_today_returns_next_week() {
        // Monday noon -> reminder is Monday 9am (already passed today).
        let target = next_reminder_time_after(monday_noon(), 0, 9, 0);
        assert_eq!(target.weekday(), Weekday::Mon);
        assert_eq!(target.day(), 29); // next Monday
        assert_eq!(target.hour(), 9);
    }

    #[test]
    fn yesterday_in_iso_week_returns_next_week_not_yesterday() {
        // Tuesday noon -> reminder is Monday 9am.
        let tue_noon = local(2026, 6, 23, 12, 0);
        let target = next_reminder_time_after(tue_noon, 0, 9, 0);
        assert_eq!(target.weekday(), Weekday::Mon);
        // Should be NEXT Monday, not yesterday.
        assert!(target > tue_noon);
        assert_eq!(target.day(), 29);
    }

    // ---- Cross-week ----

    #[test]
    fn sunday_from_monday_noon_is_six_days_away() {
        // Monday noon -> reminder is Sunday 4pm.
        let target = next_reminder_time_after(monday_noon(), 6, 16, 0);
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
        let target = next_reminder_time_after(exact, 0, 16, 0);
        assert_eq!(target.day(), 29);
    }

    #[test]
    fn target_with_seconds_remaining_returns_today() {
        // 12:00:00 now, target is 12:01. Should be today, ~1 min away.
        let now = local(2026, 6, 22, 12, 0);
        let target = next_reminder_time_after(now, 0, 12, 1);
        assert_eq!(target.day(), 22);
        assert_eq!(target.minute(), 1);
        assert_eq!((target - now).num_minutes(), 1);
    }

    // ---- Out-of-range day_of_week ----

    #[test]
    fn out_of_range_day_of_week_falls_back_to_friday() {
        let target = next_reminder_time_after(monday_noon(), 99, 16, 0);
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
                    let t = next_reminder_time_after(now, day, hour, minute);
                    assert!(
                        t > now,
                        "expected strictly future for day={day}, h={hour}, m={minute}: got {t} vs now {now}"
                    );
                }
            }
        }
    }
}
