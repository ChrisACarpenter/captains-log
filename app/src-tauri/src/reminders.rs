//! Weekly reminder scheduling.
//!
//! When journal settings say `reminder.enabled = true`, [`spawn_reminder_task`]
//! starts a long-running async task that:
//!   1. Computes the next occurrence of `(day_of_week, hour, minute)` in local time
//!   2. Sleeps until then
//!   3. Fires a macOS notification via `tauri-plugin-notification`
//!   4. Sleeps a minute (so we don't immediately fire again within the same wall-clock minute)
//!   5. Loops forever (until the app shuts down)
//!
//! ## Limitations (Phase 3 polish)
//!
//! - Doesn't survive across app restarts in the sense that nothing fires while
//!   the app is closed — macOS-scheduled notifications would be needed for that
//! - Doesn't react to settings changes during the same session — the running
//!   task continues with its initial config; restart the app to apply changes
//! - First fire on macOS will trigger the system permission prompt

use std::sync::Mutex;
use std::time::Duration as StdDuration;

use chrono::{DateTime, Datelike, Duration, Local, Timelike, Weekday};
use tauri::async_runtime::JoinHandle;
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

use crate::settings::ReminderSettings;

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

            let result = app
                .notification()
                .builder()
                .title("Captain's Log")
                .body(&body)
                .show();

            if let Err(e) = result {
                eprintln!("[reminders] notification failed: {e}");
            } else {
                println!("[reminders] fired at {}", Local::now().format("%H:%M:%S"));
            }

            // Sleep a minute so the next iteration doesn't recompute "now" inside
            // the same target minute and re-fire immediately.
            tokio::time::sleep(StdDuration::from_secs(60)).await;
        }
    });

    *slot = Some(new_handle);
}

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
