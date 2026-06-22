//! Tauri commands exposed to the frontend.
//!
//! For Phase 1 we expose:
//!   - [`create_note`] — append a Note to the current week's file
//!   - [`read_week`] — return the raw markdown of a given (year, week), if any
//!
//! State: the `LocalFilesystem` storage backend is registered as managed
//! Tauri state in `lib::run()`. Phase 1 hardcodes its root to
//! `~/Documents/CaptainsLog/`. First-run setup (Phase 1 stretch / Phase 2)
//! will read the chosen location from `.metadata/settings.json`.

use chrono::Local;
use serde::Deserialize;
use tauri::State;

use crate::labels::record_note_labels;
use crate::notes::{append_note, iso_year_week, Note};
use crate::storage::{LocalFilesystem, StorageBackend};

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
///
/// The "current week" is computed from the server-side clock (the Tauri
/// backend), not the frontend, so the timestamp is consistent regardless
/// of any frontend clock skew.
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

    // Best-effort: update the label index. If this fails, we don't want to
    // surface an error to the user — the Note itself has already been saved.
    if let Err(e) = record_note_labels(&*storage, &note, now.date_naive()).await {
        eprintln!("warning: label index update failed: {e}");
    }

    Ok(())
}

/// Read the raw markdown of a weekly file. Returns `None` if the file does
/// not exist yet.
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
