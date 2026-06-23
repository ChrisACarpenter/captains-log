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

use crate::labels::{record_note_labels, LabelEntry, LabelIndex};
use crate::notes::{
    append_note, iso_year_week, parse_weekly_summary, replace_weekly_summary_in_file,
    weekly_file_scaffold, Note, WeeklySummary,
};
use crate::reminders::{restart_reminder_task, ReminderHandle};
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
    let chosen_storage = LocalFilesystem::new(input.journal_root.clone());
    let journal_settings = JournalSettings {
        version: CURRENT_VERSION,
        user_name: input.user_name.clone(),
        reminder: input.reminder.clone(),
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

    // 4. Restart the reminder scheduler in-process with the new config.
    //    The wizard's reminder takes effect immediately — no relaunch needed.
    restart_reminder_task(
        app.clone(),
        &reminder_handle,
        input.reminder,
        input.user_name,
    );

    // 5. Broadcast so any open window (main, capture) can re-fetch and apply
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
    //    the new prefs at the right place).
    let chosen_storage = LocalFilesystem::new(input.journal_root.clone());
    let journal_settings = JournalSettings {
        version: CURRENT_VERSION,
        user_name: input.user_name.clone(),
        reminder: input.reminder.clone(),
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

    // 4. Restart the reminder scheduler with the new config (no-op if disabled).
    restart_reminder_task(
        app.clone(),
        &reminder_handle,
        input.reminder,
        input.user_name,
    );

    // 5. Broadcast so all windows refresh (theme on capture popup, Noot
    //    appears/disappears on the week stripe, etc.) without waiting for
    //    the next 60-second tick.
    let _ = app.emit("settings-changed", ());

    Ok(())
}
