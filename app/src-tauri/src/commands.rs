//! Tauri commands exposed to the frontend.
//!
//! Currently:
//!   - [`create_note`] — append a Note to the current week's file
//!   - [`read_week`] — return the raw markdown of a given (year, week)
//!   - [`get_settings`] — snapshot of app + journal settings; signals first-run
//!   - [`complete_first_run`] — writes both settings files; restarts if root changed
//!
//! State: the `LocalFilesystem` storage backend is registered as managed
//! Tauri state in `lib::run()`. Its root is determined at startup from
//! `app-settings.json` (or the default if first run).

use std::path::PathBuf;

use chrono::Local;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::email::{compose_weekly_email as compose, ComposeResult};
use crate::labels::{record_note_labels, LabelEntry, LabelIndex};
use crate::notes::{
    append_note, iso_year_week, parse_weekly_summary, replace_weekly_summary_in_file,
    weekly_file_scaffold, CaptureDraft, Note, WeeklySummary,
};
use crate::reminders::{
    request_notification_authorization, restart_reminder_task, ReminderHandle,
};
use crate::sent_log::{
    get_sent_record as load_sent_record, hash_weekly_summary, upsert_sent_record, SentRecord,
};
use crate::{DirtyEntry, DirtyRegistry};
use crate::settings::{
    default_journal_root, AppSettings, JournalSettings, ReminderSettings, Theme, CURRENT_VERSION,
};
use crate::storage::{LocalFilesystem, StorageBackend};
use crate::SharedStorage;

// ---------------------------------------------------------------------------
// create_note / read_week
// ---------------------------------------------------------------------------

/// Input payload for [`create_note`]. The frontend sends these fields as a
/// single object argument.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateNoteInput {
    pub title: Option<String>,
    pub body: String,
    #[serde(default)]
    pub labels: Vec<String>,
}

/// Append a Note to the current ISO week's file.
#[tauri::command]
pub async fn create_note(
    storage_state: State<'_, SharedStorage>,
    input: CreateNoteInput,
) -> Result<(), String> {
    let now = Local::now().fixed_offset();
    let (year, week) = iso_year_week(now.date_naive());

    let note = Note {
        timestamp: now,
        title: input.title.and_then(|t| {
            let trimmed = t.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        }),
        labels: input
            .labels
            .into_iter()
            .map(|l| l.trim().trim_start_matches('#').to_string())
            .filter(|l| !l.is_empty())
            .collect(),
        body: input.body,
    };

    let storage = storage_state.read().await;

    append_note(&*storage, year, week, &note)
        .await
        .map_err(|e| e.to_string())?;

    if let Err(e) = record_note_labels(&*storage, &note, now.date_naive()).await {
        eprintln!("warning: label index update failed: {e}");
    }

    Ok(())
}

/// Read the raw markdown of a weekly file. Returns `None` if absent.
#[tauri::command]
pub async fn read_week(
    storage_state: State<'_, SharedStorage>,
    year: u32,
    week: u32,
) -> Result<Option<String>, String> {
    let storage = storage_state.read().await;
    storage
        .read_week(year, week)
        .await
        .map_err(|e| e.to_string())
}

/// Overwrite the entire weekly file with the given markdown. Used by the
/// journal browser's raw-markdown editor (`/journal` route) to save edits
/// to past weeks. The structured Weekly Summary editor at `/summary` uses
/// `update_weekly_summary` instead, which splices changes into the summary
/// section while preserving Weekly Notes below.
#[tauri::command]
pub async fn write_week(
    storage_state: State<'_, SharedStorage>,
    year: u32,
    week: u32,
    content: String,
) -> Result<(), String> {
    let storage = storage_state.read().await;
    storage
        .write_week(year, week, &content)
        .await
        .map_err(|e| e.to_string())
}

/// List ISO years that have any weekly files, sorted ascending. Empty if
/// the journal root has nothing yet.
#[tauri::command]
pub async fn list_years(storage_state: State<'_, SharedStorage>) -> Result<Vec<u32>, String> {
    let storage = storage_state.read().await;
    storage.list_years().await.map_err(|e| e.to_string())
}

/// List ISO week numbers present for the given year, sorted ascending.
/// Empty if the year folder doesn't exist.
#[tauri::command]
pub async fn list_weeks(
    storage_state: State<'_, SharedStorage>,
    year: u32,
) -> Result<Vec<u32>, String> {
    let storage = storage_state.read().await;
    storage.list_weeks(year).await.map_err(|e| e.to_string())
}

/// Return all known labels with their usage stats, sorted by recent-then-frequent
/// (the autocomplete ranking from `docs/label-system.md`).
#[tauri::command]
pub async fn get_labels(
    storage_state: State<'_, SharedStorage>,
) -> Result<Vec<LabelEntry>, String> {
    let storage = storage_state.read().await;
    let index = LabelIndex::load(&*storage)
        .await
        .map_err(|e| e.to_string())?;
    Ok(index.labels)
}

// ---------------------------------------------------------------------------
// Weekly Summary
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YearWeek {
    pub year: u32,
    pub week: u32,
}

/// Return the current ISO year + week as a single struct. Used by the
/// frontend to know which weekly file to load.
#[tauri::command]
pub fn get_current_year_week() -> YearWeek {
    let (year, week) = iso_year_week(Local::now().date_naive());
    YearWeek { year, week }
}

/// Read and parse the Weekly Summary for a given (year, week). Returns
/// an empty/default summary when the weekly file doesn't exist yet — the
/// frontend can render the empty form without distinguishing first-write
/// from existing-file-with-no-summary.
#[tauri::command]
pub async fn get_weekly_summary(
    storage_state: State<'_, SharedStorage>,
    year: u32,
    week: u32,
) -> Result<WeeklySummary, String> {
    let storage = storage_state.read().await;
    let content = storage
        .read_week(year, week)
        .await
        .map_err(|e| e.to_string())?;
    Ok(match content {
        Some(c) => parse_weekly_summary(&c),
        None => WeeklySummary::default(),
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateWeeklySummaryInput {
    pub year: u32,
    pub week: u32,
    pub key_accomplishments: String,
    pub plans_and_priorities: String,
    pub challenges_or_roadblocks: String,
    pub anything_else: String,
    #[serde(default)]
    pub labels: Vec<String>,
}

/// Splice the user's edits back into the weekly file, preserving everything
/// outside the Weekly Summary section (frontmatter, week heading, Weekly Notes
/// with their captured notes). If the file doesn't exist yet, creates the
/// scaffold first and then splices.
///
/// `last_updated` is stamped server-side with the local clock — the frontend
/// doesn't send it.
#[tauri::command]
pub async fn update_weekly_summary(
    storage_state: State<'_, SharedStorage>,
    input: UpdateWeeklySummaryInput,
) -> Result<(), String> {
    let now = Local::now().fixed_offset();
    let new_summary = WeeklySummary {
        key_accomplishments: input.key_accomplishments,
        plans_and_priorities: input.plans_and_priorities,
        challenges_or_roadblocks: input.challenges_or_roadblocks,
        anything_else: input.anything_else,
        labels: input
            .labels
            .into_iter()
            .map(|l| l.trim().trim_start_matches('#').to_string())
            .filter(|l| !l.is_empty())
            .collect(),
        last_updated: Some(now.format("%Y-%m-%d %H:%M").to_string()),
    };

    let storage = storage_state.read().await;

    let existing = storage
        .read_week(input.year, input.week)
        .await
        .map_err(|e| e.to_string())?;

    let updated = match existing {
        Some(content) => replace_weekly_summary_in_file(&content, &new_summary),
        None => {
            let scaffold = weekly_file_scaffold(input.year, input.week, now);
            replace_weekly_summary_in_file(&scaffold, &new_summary)
        }
    };

    storage
        .write_week(input.year, input.week, &updated)
        .await
        .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

/// What the frontend sees when querying settings state.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsBundle {
    /// `true` when app-settings.json doesn't exist yet — the wizard should render.
    pub first_run: bool,
    /// The currently-active journal root (default on first run; configured otherwise).
    pub journal_root: PathBuf,
    /// The recommended default location for the first-run picker.
    pub default_journal_root: PathBuf,
    /// The user's display name, if set.
    pub user_name: Option<String>,
    /// Reminder preferences.
    pub reminder: ReminderSettings,
    /// Active theme — defaults to Dark, persisted in app-settings.json.
    pub theme: Theme,
    /// Manager's email — used by the "Send weekly summary to manager" flow.
    pub manager_email: Option<String>,
    /// Manager's display name — used to personalize the email greeting.
    pub manager_name: Option<String>,
}

#[tauri::command]
pub async fn get_settings(
    app: AppHandle,
    storage_state: State<'_, SharedStorage>,
) -> Result<SettingsBundle, String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;

    let app_settings = AppSettings::load(&app_data_dir)
        .await
        .map_err(|e| e.to_string())?;

    let storage = storage_state.read().await;

    let journal_settings = JournalSettings::load(&*storage)
        .await
        .map_err(|e| e.to_string())?;

    let journal_root = app_settings
        .as_ref()
        .map(|s| s.journal_root.clone())
        .unwrap_or_else(|| storage.root().to_path_buf());
    let theme = app_settings.as_ref().map(|s| s.theme).unwrap_or_default();

    Ok(SettingsBundle {
        first_run: app_settings.is_none(),
        journal_root,
        default_journal_root: default_journal_root(),
        user_name: journal_settings.user_name,
        reminder: journal_settings.reminder,
        theme,
        manager_email: journal_settings.manager_email,
        manager_name: journal_settings.manager_name,
    })
}

/// Payload sent by the first-run wizard on completion.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompleteFirstRunInput {
    pub user_name: Option<String>,
    pub journal_root: PathBuf,
    pub reminder: ReminderSettings,
}

/// Payload sent by the post-first-run settings panel.
///
/// All fields are present (not optional) because the settings panel always
/// renders a full form — partial updates aren't a thing yet.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSettingsInput {
    pub user_name: Option<String>,
    pub journal_root: PathBuf,
    pub reminder: ReminderSettings,
    pub theme: Theme,
    /// Manager email — `None` (or empty after trim) disables the Send button.
    /// `#[serde(default)]` lets older frontends omit the field without erroring.
    #[serde(default)]
    pub manager_email: Option<String>,
    /// Manager display name — purely cosmetic (greeting in the email).
    #[serde(default)]
    pub manager_name: Option<String>,
}

/// Writes both settings files. If the user picked a journal root different
/// from the running storage's root, returns `true` so the frontend can prompt
/// for an app restart. (`app.restart()` is unreliable across Tauri 2 minor
/// versions, so we surface the need to the UI instead of triggering it.)
#[tauri::command]
pub async fn complete_first_run(
    app: AppHandle,
    storage_state: State<'_, SharedStorage>,
    reminder_handle: State<'_, ReminderHandle>,
    input: CompleteFirstRunInput,
) -> Result<(), String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;

    // 1. Save app-level settings (journal_root + theme — theme defaults to Dark on first run).
    let app_settings = AppSettings {
        version: CURRENT_VERSION,
        journal_root: input.journal_root.clone(),
        theme: Theme::default(),
    };
    app_settings
        .save(&app_data_dir)
        .await
        .map_err(|e| e.to_string())?;

    // 2. Save journal-level settings into the CHOSEN root (which may differ
    //    from the storage instance's root if the user picked a non-default).
    //    First-run wizard doesn't collect manager email/name today (those
    //    come in Phase 2.7's onboarding revisit); leave both None.
    let chosen_storage = LocalFilesystem::new(input.journal_root.clone());
    let journal_settings = JournalSettings {
        version: CURRENT_VERSION,
        user_name: input.user_name.clone(),
        reminder: input.reminder.clone(),
        manager_email: None,
        manager_name: None,
    };
    journal_settings
        .save(&chosen_storage)
        .await
        .map_err(|e| e.to_string())?;

    // 3. Hot-swap the running LocalFilesystem if the user picked a non-default
    //    root. After this, subsequent commands write to the chosen location
    //    without an app restart.
    {
        let mut fs = storage_state.write().await;
        if fs.root() != input.journal_root.as_path() {
            *fs = LocalFilesystem::new(input.journal_root.clone());
        }
    }

    // 4. Request notification permission if the user just enabled reminders.
    //    This is the highest-acceptance moment for the prompt — the user
    //    explicitly opted in. Idempotent: subsequent calls return the
    //    remembered decision. No-op on non-macOS platforms.
    if input.reminder.enabled {
        request_notification_authorization().await;
    }

    // 5. Restart the reminder scheduler in-process with the new config.
    //    The wizard's reminder takes effect immediately — no relaunch needed.
    restart_reminder_task(
        app.clone(),
        &reminder_handle,
        input.reminder,
        input.user_name,
    );

    // 6. Broadcast so any open window (main, capture) can re-fetch and apply
    //    the new settings immediately — theme, reminder position, etc.
    let _ = app.emit("settings-changed", ());

    // No restart needed — storage and reminder both hot-swap.
    Ok(())
}

/// Save edits from the post-first-run settings panel.
///
/// Like `complete_first_run`, but also handles `theme` (which the wizard
/// doesn't expose) and is meant for use after the user has already onboarded.
/// Everything applies in-process — no app restart needed, even when
/// journal_root changes (the running `LocalFilesystem` is swapped).
#[tauri::command]
pub async fn update_settings(
    app: AppHandle,
    storage_state: State<'_, SharedStorage>,
    reminder_handle: State<'_, ReminderHandle>,
    input: UpdateSettingsInput,
) -> Result<(), String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;

    // 1. App-level (journal_root + theme).
    let app_settings = AppSettings {
        version: CURRENT_VERSION,
        journal_root: input.journal_root.clone(),
        theme: input.theme,
    };
    app_settings
        .save(&app_data_dir)
        .await
        .map_err(|e| e.to_string())?;

    // 2. Journal-level (write to the chosen root so a root change still lands
    //    the new prefs at the right place). Manager email is trimmed; an
    //    empty string after trimming persists as None so the Send button's
    //    "is this set?" check stays simple.
    let chosen_storage = LocalFilesystem::new(input.journal_root.clone());
    let manager_email = input
        .manager_email
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let manager_name = input
        .manager_name
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let journal_settings = JournalSettings {
        version: CURRENT_VERSION,
        user_name: input.user_name.clone(),
        reminder: input.reminder.clone(),
        manager_email,
        manager_name,
    };
    journal_settings
        .save(&chosen_storage)
        .await
        .map_err(|e| e.to_string())?;

    // 3. Hot-swap the running LocalFilesystem if root changed.
    {
        let mut fs = storage_state.write().await;
        if fs.root() != input.journal_root.as_path() {
            *fs = LocalFilesystem::new(input.journal_root.clone());
        }
    }

    // 4. Request notification permission if the user has reminders enabled.
    //    macOS only — the system prompt fires once, subsequent calls return
    //    the remembered decision.
    if input.reminder.enabled {
        request_notification_authorization().await;
    }

    // 5. Restart the reminder scheduler with the new config (no-op if disabled).
    restart_reminder_task(
        app.clone(),
        &reminder_handle,
        input.reminder,
        input.user_name,
    );

    // 6. Broadcast so all windows refresh (theme on capture popup, Noot
    //    appears/disappears on the week stripe, etc.) without waiting for
    //    the next 60-second tick.
    let _ = app.emit("settings-changed", ());

    Ok(())
}

// ---------------------------------------------------------------------------
// Dirty registry
// ---------------------------------------------------------------------------

/// Publish the dirty state of a frontend surface into the backend's
/// DirtyRegistry. Called by `app/src/lib/dirty.ts` from /summary and the
/// capture popup whenever their form state diverges from the last-saved
/// snapshot. Read at quit time by `try_quit` (in lib.rs).
///
/// `key` is a stable namespace string ("summary", "capture"). Adding more
/// dirty surfaces later doesn't require Rust changes — just call with a
/// new key from the frontend.
#[tauri::command]
pub fn set_window_dirty(
    registry: State<'_, DirtyRegistry>,
    key: String,
    entry: DirtyEntry,
) {
    let mut guard = registry.0.lock().expect("dirty registry mutex poisoned");
    guard.insert(key, entry);
}

// ---------------------------------------------------------------------------
// Capture draft (auto-save Phase 2)
// ---------------------------------------------------------------------------
//
// The quick-capture popup auto-saves its in-flight contents to
// `<journal>/.metadata/capture-draft.json` on a 1.5s debounce. This lets the
// user close the popup, quit the app, or crash without losing what they were
// typing — the draft reloads on next launch. The draft is cleared on a
// successful Submit (when it becomes a real Note in the weekly file).

const CAPTURE_DRAFT_FILE: &str = "capture-draft.json";

/// Load the saved draft, if any. Returns `None` when the file is missing,
/// when it parses but is empty (all fields blank — semantically nothing to
/// restore), or when the file is corrupt (treated as "no draft" rather than
/// surfacing a parse error — a corrupt file simply means the user starts
/// with an empty popup, which is the same as no draft).
#[tauri::command]
pub async fn load_capture_draft(
    storage_state: State<'_, SharedStorage>,
) -> Result<Option<CaptureDraft>, String> {
    let storage = storage_state.read().await;
    let raw = match storage.read_metadata(CAPTURE_DRAFT_FILE).await {
        Ok(Some(c)) => c,
        Ok(None) => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    let draft: CaptureDraft = match serde_json::from_str(&raw) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("[capture-draft] failed to parse {CAPTURE_DRAFT_FILE}: {e}");
            return Ok(None);
        }
    };
    if draft.is_empty() {
        Ok(None)
    } else {
        Ok(Some(draft))
    }
}

/// Persist the current draft. If the draft is empty (after normalization)
/// we delete the file instead of writing empty bytes — keeps the
/// `.metadata/` folder clean for the no-draft case.
#[tauri::command]
pub async fn save_capture_draft(
    storage_state: State<'_, SharedStorage>,
    draft: CaptureDraft,
) -> Result<(), String> {
    let storage = storage_state.read().await;
    if draft.is_empty() {
        return storage
            .delete_metadata(CAPTURE_DRAFT_FILE)
            .await
            .map_err(|e| e.to_string());
    }
    let serialized = serde_json::to_string_pretty(&draft).map_err(|e| e.to_string())?;
    storage
        .write_metadata(CAPTURE_DRAFT_FILE, &serialized)
        .await
        .map_err(|e| e.to_string())
}

/// Delete the draft file. Called after a successful Submit (the draft
/// became a real Note). Idempotent — "file already absent" is fine.
#[tauri::command]
pub async fn clear_capture_draft(
    storage_state: State<'_, SharedStorage>,
) -> Result<(), String> {
    let storage = storage_state.read().await;
    storage
        .delete_metadata(CAPTURE_DRAFT_FILE)
        .await
        .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Send weekly summary to manager (Phase 2.6)
// ---------------------------------------------------------------------------
//
// The frontend's "Send to manager" button drives three commands in sequence:
//
//   1. get_sent_record(year, week) on page load — feeds the disabled/enabled
//      decision (already sent for this week + same content hash → disabled).
//   2. compose_weekly_email(year, week) on click + confirm — returns either
//      a mailto: URL or an .eml file path; frontend hands it to opener.
//   3. mark_weekly_summary_sent(year, week, contentHash, sentTo) after the
//      open returns Ok — stamps sent-log.json so the next load knows.
//
// No live link to lib::run is needed; everything reads/writes through the
// same storage backend the rest of the app already uses.

/// Return the sent-log entry for (year, week), or `None` if this week has
/// never been sent. Cheap — re-reads `sent-log.json` each call (the file is
/// tiny; in-memory caching would just add invalidation bugs).
#[tauri::command]
pub async fn get_sent_record(
    storage_state: State<'_, SharedStorage>,
    year: u32,
    week: u32,
) -> Result<Option<SentRecord>, String> {
    let storage = storage_state.read().await;
    load_sent_record(&*storage, year, week)
        .await
        .map_err(|e| e.to_string())
}

/// Compose the email for (year, week) into either a `mailto:` URL or an
/// `.eml` file (length-based fallback). Reads the current Weekly Summary
/// off disk every time so we never compose stale text — frontend gates the
/// button on `isDirty` to prevent the user from sending unsaved edits.
///
/// Errors:
///   - `"no manager email set"` if the journal settings have no manager
///     email (or it's empty after trim). UI gates on this too; backend
///     check is defense-in-depth.
///   - I/O / serde errors as strings.
#[tauri::command]
pub async fn compose_weekly_email(
    storage_state: State<'_, SharedStorage>,
    year: u32,
    week: u32,
) -> Result<ComposeResult, String> {
    let storage = storage_state.read().await;

    let journal_settings = JournalSettings::load(&*storage)
        .await
        .map_err(|e| e.to_string())?;
    let recipient = journal_settings
        .manager_email
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "no manager email set".to_string())?;
    let manager_name = journal_settings
        .manager_name
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let raw = storage
        .read_week(year, week)
        .await
        .map_err(|e| e.to_string())?;
    let summary = match raw {
        Some(c) => parse_weekly_summary(&c),
        None => return Err(format!("no weekly summary saved for {year}-W{week:02}")),
    };

    // Check the sent-log: if a record for this week already exists, this is
    // a resend (gating ensures the content hash differs from the recorded one,
    // so we wouldn't even be here unless the user edited and saved). Resends
    // use a different subject line so the manager's mail thread shows it's
    // an updated version of an earlier message.
    let is_resend = load_sent_record(&*storage, year, week)
        .await
        .map_err(|e| e.to_string())?
        .is_some();

    let week_label = format_week_label(year, week);
    let now = Local::now().fixed_offset();
    let params = crate::email::ComposeParams {
        summary: &summary,
        week_label: &week_label,
        recipient: &recipient,
        manager_name: manager_name.as_deref(),
        is_resend,
        now,
    };
    compose(params).map_err(|e| e.to_string())
}

/// Stamp the sent-log entry for (year, week) after the user successfully
/// hands off the email to their mail client. The frontend supplies the
/// content hash it observed at compose time so we never compute it
/// differently between the two calls.
#[tauri::command]
pub async fn mark_weekly_summary_sent(
    storage_state: State<'_, SharedStorage>,
    year: u32,
    week: u32,
    content_hash: String,
    sent_to: String,
) -> Result<SentRecord, String> {
    let storage = storage_state.read().await;
    let now = Local::now().fixed_offset();
    let record = SentRecord {
        sent_at: now.to_rfc3339(),
        content_hash,
        sent_to,
    };
    upsert_sent_record(&*storage, year, week, record.clone())
        .await
        .map_err(|e| e.to_string())?;
    Ok(record)
}

/// Compute the SHA-256 hash of the current saved Weekly Summary for
/// (year, week). Used by the frontend to drive the Send-button gating
/// (compare against the hash stored in the sent-log entry to detect
/// "edited since last send"). Returns an empty string if no summary
/// exists yet.
#[tauri::command]
pub async fn get_summary_hash(
    storage_state: State<'_, SharedStorage>,
    year: u32,
    week: u32,
) -> Result<String, String> {
    let storage = storage_state.read().await;
    let raw = storage
        .read_week(year, week)
        .await
        .map_err(|e| e.to_string())?;
    Ok(match raw {
        Some(c) => hash_weekly_summary(&parse_weekly_summary(&c)),
        None => String::new(),
    })
}

/// Format `(year, week)` as the human-readable label used in email subjects,
/// the email body intro line, and the confirmation modal: e.g.
/// `"week of June 22 – June 28, 2026"`. Lowercase leading "week" so the
/// string drops cleanly into sentences ("Weekly update - week of ...",
/// "for the week of ..."); the /summary heading uses its own capitalized
/// variant ("Week of June 22 – June 28, 2026") computed by the frontend.
///
/// ## Stay in sync with the frontend
///
/// The frontend computes its own `weekLabel` in `/summary` for display in
/// the page heading and confirmation modal (via `inlineLabel()` to drop
/// the leading capital). The two strings MUST match character-for-character
/// when lowercased on the leading `W` — otherwise the user sees one rendering
/// in the modal ("for the week of June 22 – June 28, 2026") and a different
/// rendering in the actual email subject they hand off to their mail client.
/// Format conventions kept in lockstep:
///
///   - Full month name (`%B` → "June", not abbreviated "Jun")
///   - En-dash (U+2013, " – ") between start and end dates, not ASCII "-"
///   - No zero-padding on day numbers (`%-d` → "22", not "22")
///   - Cross-year weeks repeat the year on both sides
///
/// If either side changes, the matching test in /summary's weekLabel logic
/// AND `week_label_matches_frontend_format` here must be updated together.
fn format_week_label(year: u32, week: u32) -> String {
    use chrono::{Datelike, Duration, NaiveDate};
    // ISO week 1 is the week containing Jan 4. Walk back to that week's
    // Monday, then offset by (week-1) weeks.
    let Some(jan4) = NaiveDate::from_ymd_opt(year as i32, 1, 4) else {
        return format!("{year}-W{week:02}");
    };
    let dow_from_monday = jan4.weekday().num_days_from_monday();
    let monday_of_week1 = jan4 - Duration::days(dow_from_monday as i64);
    let monday = monday_of_week1 + Duration::weeks((week as i64).saturating_sub(1));
    let sunday = monday + Duration::days(6);

    // "June 22" — %B is the full month name, %-d strips zero-padding.
    let fmt = |d: NaiveDate| d.format("%B %-d").to_string();

    if monday.year() == sunday.year() {
        format!(
            "week of {} \u{2013} {}, {}",
            fmt(monday),
            fmt(sunday),
            monday.year()
        )
    } else {
        format!(
            "week of {}, {} \u{2013} {}, {}",
            fmt(monday),
            monday.year(),
            fmt(sunday),
            sunday.year()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn week_label_matches_frontend_format() {
        // The frontend computes weekLabel for the heading + modal with full
        // month names + en-dash. The backend must produce a string that's
        // identical to inlineLabel(frontend) so the modal and the email
        // subject read the same week.
        let s = format_week_label(2026, 26);
        assert!(s.starts_with("week of June "), "got {s:?}");
        assert!(s.ends_with(", 2026"), "got {s:?}");
        assert!(s.contains(" \u{2013} "), "expected en-dash separator, got {s:?}");
    }

    #[test]
    fn week_label_w01_is_january() {
        let s = format_week_label(2026, 1);
        // W01 always contains Jan 4; the Monday could be late Dec of the
        // previous year, hence the cross-year branch. Either way the label
        // mentions "January" (full month name now).
        assert!(s.contains("January"), "expected January in {s}");
    }
}
