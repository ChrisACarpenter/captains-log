// Captain's Log — Tauri backend.
//
// Module layout:
//   storage  — StorageBackend trait + LocalFilesystem impl
//   notes    — Note struct, markdown serialization, ISO week math, append_note
//   commands — Tauri command handlers exposed to the frontend
//   labels   — Label index management [TODO]

pub mod commands;
pub mod notes;
pub mod storage;

use std::path::PathBuf;

use storage::LocalFilesystem;

/// Default journal root for v1. The first-run setup flow will write a
/// settings file pointing wherever the user picks, and a later iteration
/// will read that here. For now we hardcode the recommended default.
fn default_journal_root() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join("Documents").join("CaptainsLog")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let storage = LocalFilesystem::new(default_journal_root());

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(storage)
        .invoke_handler(tauri::generate_handler![
            commands::create_note,
            commands::read_week,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
