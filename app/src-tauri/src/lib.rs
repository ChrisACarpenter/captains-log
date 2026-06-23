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
    Manager, WindowEvent,
};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_window_state::StateFlags;
use tokio::sync::RwLock;

use settings::default_journal_root;
use storage::LocalFilesystem;

/// Tauri-managed storage state. Wrapped in a `tokio::sync::RwLock` so a
/// settings change can swap the root in-process without an app restart.
/// All commands take a brief read lock; only `update_settings` ever writes.
pub type SharedStorage = RwLock<LocalFilesystem>;

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
        // Auto-saves and restores window size/position/maximized state across
        // launches. Two important deviations from the plugin's defaults:
        //
        //   - VISIBLE flag dropped. The plugin's default includes VISIBLE and
        //     its restore_state forces .show() + .set_focus() on every window,
        //     which overrides tauri.conf.json's `"visible": false` on the
        //     capture popup and pops it open on every launch.
        //   - skip_initial_state for capture. Belt-and-suspenders: even if a
        //     future flag change re-enables VISIBLE, the capture popup is
        //     opted out of initial-state restore so it stays hidden until the
        //     tray is clicked.
        //
        // Geometry is still saved on CloseRequested and on app Exit, so when
        // the popup re-shows it lands on the monitor/position it was last on.
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(StateFlags::all() - StateFlags::VISIBLE)
                .skip_initial_state(CAPTURE_WINDOW_LABEL)
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            commands::create_note,
            commands::read_week,
            commands::get_labels,
            commands::get_settings,
            commands::complete_first_run,
            commands::update_settings,
            commands::get_current_year_week,
            commands::get_weekly_summary,
            commands::update_weekly_summary,
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
            app.manage::<SharedStorage>(RwLock::new(LocalFilesystem::new(journal_root)));

            // Manage the reminder task handle so commands can restart the
            // scheduler in-process when settings change.
            app.manage(reminders::ReminderHandle::new());

            // Spawn the weekly reminder task if enabled. Reads journal-level
            // settings (user_name + reminder config) from the just-mounted storage.
            {
                let storage_state = app.state::<SharedStorage>();
                let journal_settings = tauri::async_runtime::block_on(async {
                    let fs = storage_state.read().await;
                    settings::JournalSettings::load(&*fs).await
                })
                .unwrap_or_default();
                let reminder_handle = app.state::<reminders::ReminderHandle>();
                reminders::restart_reminder_task(
                    app.handle().clone(),
                    &reminder_handle,
                    journal_settings.reminder,
                    journal_settings.user_name,
                );
            }

            // Intercept the main window's close button. When the quick-capture
            // popup is currently visible, prompt the user before exiting —
            // otherwise the popup would orphan itself, surviving the main
            // window's close while having no app shell behind it.
            {
                let main_window = app
                    .get_webview_window("main")
                    .expect("main window declared in tauri.conf.json");
                let app_handle = app.handle().clone();
                main_window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        let capture_visible = app_handle
                            .get_webview_window(CAPTURE_WINDOW_LABEL)
                            .and_then(|w| w.is_visible().ok())
                            .unwrap_or(false);

                        if capture_visible {
                            api.prevent_close();
                            let app_h = app_handle.clone();
                            app_handle
                                .dialog()
                                .message(
                                    "You have an open quick-capture note. Closing now will \
                                     discard whatever's typed there.\n\n\
                                     Close Captain's Log and the note popup together?",
                                )
                                .title("Close Captain's Log?")
                                .buttons(MessageDialogButtons::OkCancelCustom(
                                    "Close both".to_string(),
                                    "Keep open".to_string(),
                                ))
                                .kind(MessageDialogKind::Warning)
                                .show(move |confirmed| {
                                    if confirmed {
                                        app_h.exit(0);
                                    }
                                });
                        }
                        // Otherwise: default close behavior — main window
                        // closes, app continues running in the tray.
                    }
                });
            }

            // Capture popup close button (red-X / Cmd-W) hides instead of
            // destroying. Default Tauri behavior would drop the window from
            // the app's registry, after which `get_webview_window("capture")`
            // returns None and the tray click handler silently no-ops. Hiding
            // keeps the WebviewWindow handle alive for the lifetime of the
            // app so the tray reliably toggles show/hide.
            {
                let capture_window = app
                    .get_webview_window(CAPTURE_WINDOW_LABEL)
                    .expect("capture window declared in tauri.conf.json");
                let capture_clone = capture_window.clone();
                capture_window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = capture_clone.hide();
                    }
                });
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
