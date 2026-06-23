// Captain's Log — Tauri backend.
//
// Module layout:
//   storage   — StorageBackend trait + LocalFilesystem impl
//   notes     — Note struct, markdown serialization, ISO week math, append_note
//   labels    — Label index ( .metadata/labels.json ), inline #hashtag extraction
//   settings  — App + journal settings, first-run state
//   reminders — Weekly notification scheduler
//   commands  — Tauri command handlers exposed to the frontend

pub mod commands;
pub mod labels;
pub mod notes;
pub mod reminders;
pub mod settings;
pub mod storage;

use tauri::{
    image::Image,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};

use settings::default_journal_root;
use storage::LocalFilesystem;

/// PNG for the macOS menu bar template icon (book outline). Black-with-alpha
/// at 22pt @ 2x so the system can recolor for light/dark menu bar mode.
const TRAY_ICON_PNG: &[u8] = include_bytes!("../icons/tray-template@2x.png");

/// Label of the small quick-capture popup window. Must match `tauri.conf.json`.
const CAPTURE_WINDOW_LABEL: &str = "capture";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![
            commands::create_note,
            commands::read_week,
            commands::get_settings,
            commands::complete_first_run,
            commands::update_settings,
        ])
        .setup(|app| {
            // Determine the journal root: from app-settings.json if present,
            // otherwise the platform default (~/Documents/CaptainsLog/).
            // This needs the app handle for app.path(), so we do it in setup().
            let app_data_dir = app.path().app_data_dir()?;
            let journal_root = match tauri::async_runtime::block_on(
                settings::AppSettings::load(&app_data_dir),
            ) {
                Ok(Some(s)) => s.journal_root,
                Ok(None) => default_journal_root(),
                Err(e) => {
                    eprintln!("warning: failed to load app settings ({e}); using default root");
                    default_journal_root()
                }
            };
            app.manage(LocalFilesystem::new(journal_root));

            // Manage the reminder task handle so commands can restart the
            // scheduler in-process when settings change.
            app.manage(reminders::ReminderHandle::new());

            // Spawn the weekly reminder task if enabled. Reads journal-level
            // settings (user_name + reminder config) from the just-mounted storage.
            {
                let storage = app.state::<LocalFilesystem>();
                let journal_settings = tauri::async_runtime::block_on(
                    settings::JournalSettings::load(&*storage),
                )
                .unwrap_or_default();
                let reminder_handle = app.state::<reminders::ReminderHandle>();
                reminders::restart_reminder_task(
                    app.handle().clone(),
                    &reminder_handle,
                    journal_settings.reminder,
                    journal_settings.user_name,
                );
            }

            // Tray icon — left-click toggles the quick-capture popup window.
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
