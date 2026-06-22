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
use tauri::{AppHandle, Manager, State};

use crate::labels::record_note_labels;
use crate::notes::{append_note, iso_year_week, Note};
use crate::reminders::{restart_reminder_task, ReminderHandle};
use crate::settings::{
    default_journal_root, AppSettings, JournalSettings, ReminderSettings, CURRENT_VERSION,
};
use crate::storage::{LocalFilesystem, StorageBackend};

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
    storage: State<'_, LocalFilesystem>,
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
    storage: State<'_, LocalFilesystem>,
    year: u32,
    week: u32,
) -> Result<Option<String>, String> {
    storage
        .read_week(year, week)
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
}

#[tauri::command]
pub async fn get_settings(
    app: AppHandle,
    storage: State<'_, LocalFilesystem>,
) -> Result<SettingsBundle, String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;

    let app_settings = AppSettings::load(&app_data_dir)
        .await
        .map_err(|e| e.to_string())?;

    let journal_settings = JournalSettings::load(&*storage)
        .await
        .map_err(|e| e.to_string())?;

    let journal_root = app_settings
        .as_ref()
        .map(|s| s.journal_root.clone())
        .unwrap_or_else(|| storage.root().to_path_buf());

    Ok(SettingsBundle {
        first_run: app_settings.is_none(),
        journal_root,
        default_journal_root: default_journal_root(),
        user_name: journal_settings.user_name,
        reminder: journal_settings.reminder,
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

/// Writes both settings files. If the user picked a journal root different
/// from the running storage's root, returns `true` so the frontend can prompt
/// for an app restart. (`app.restart()` is unreliable across Tauri 2 minor
/// versions, so we surface the need to the UI instead of triggering it.)
#[tauri::command]
pub async fn complete_first_run(
    app: AppHandle,
    storage: State<'_, LocalFilesystem>,
    reminder_handle: State<'_, ReminderHandle>,
    input: CompleteFirstRunInput,
) -> Result<bool, String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;

    // 1. Save app-level settings (journal_root).
    let app_settings = AppSettings {
        version: CURRENT_VERSION,
        journal_root: input.journal_root.clone(),
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

    // 3. Restart the reminder scheduler in-process with the new config.
    //    This is what removes the "second restart" friction — the wizard's
    //    reminder takes effect immediately, no binary relaunch needed.
    restart_reminder_task(
        app.clone(),
        &reminder_handle,
        input.reminder,
        input.user_name,
    );

    // 4. Signal whether a restart is needed (for the journal_root change,
    //    which the running LocalFilesystem can't hot-swap yet).
    let restart_needed = storage.root() != input.journal_root.as_path();
    Ok(restart_needed)
}
