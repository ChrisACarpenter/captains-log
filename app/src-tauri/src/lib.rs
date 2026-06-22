// Captain's Log — Tauri backend.
//
// Module layout:
//   storage  — StorageBackend trait + LocalFilesystem impl
//   notes    — Note struct, markdown serialization, ISO week math, append_note
//   labels   — Label index ( .metadata/labels.json ), inline #hashtag extraction
//   commands — Tauri command handlers exposed to the frontend

pub mod commands;
pub mod labels;
pub mod notes;
pub mod storage;

use std::path::PathBuf;

use tauri::{
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};

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
        .setup(|app| {
            // Tray icon — left-click toggles the main window's visibility.
            // The default app icon is used as a placeholder; a proper macOS
            // template image (black-with-alpha) will replace it later for a
            // cleaner menu bar look.
            TrayIconBuilder::with_id("main-tray")
                .tooltip("Captain's Log")
                .icon(app.default_window_icon().unwrap().clone())
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let visible = window.is_visible().unwrap_or(false);
                            let focused = window.is_focused().unwrap_or(false);
                            if visible && focused {
                                let _ = window.hide();
                            } else {
                                let _ = window.unminimize();
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
