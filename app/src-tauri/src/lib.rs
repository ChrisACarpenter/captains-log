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
    image::Image,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};

use storage::LocalFilesystem;

/// PNG for the macOS menu bar template icon (book outline). Black-with-alpha
/// at 22pt @ 2x so the system can recolor for light/dark menu bar mode.
const TRAY_ICON_PNG: &[u8] = include_bytes!("../icons/tray-template@2x.png");

/// Label of the small quick-capture popup window. Must match `tauri.conf.json`.
const CAPTURE_WINDOW_LABEL: &str = "capture";

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
            // Tray icon — left-click toggles the quick-capture popup window.
            // The PNG is a black-with-alpha template image; `icon_as_template(true)`
            // tells macOS to recolor it for the menu bar's light/dark mode.
            //
            // Tauri's Image type wants raw RGBA bytes, not encoded PNG — decode
            // once at startup with the `image` crate (png feature only).
            let decoded = image::load_from_memory_with_format(
                TRAY_ICON_PNG,
                image::ImageFormat::Png,
            )?;
            let rgba = decoded.to_rgba8();
            let (icon_w, icon_h) = (rgba.width(), rgba.height());
            let tray_icon = Image::new_owned(rgba.into_raw(), icon_w, icon_h);

            TrayIconBuilder::with_id("main-tray")
                .tooltip("Captain's Log — capture a note")
                .icon(tray_icon)
                .icon_as_template(true)
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window(CAPTURE_WINDOW_LABEL) {
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
