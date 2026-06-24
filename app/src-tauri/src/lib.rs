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
pub mod email;
pub mod labels;
pub mod notes;
pub mod reminders;
pub mod sent_log;
pub mod settings;
pub mod spellcheck;
pub mod storage;

use std::collections::HashMap;
use std::sync::Mutex;

use serde::Deserialize;
use tauri::{
    image::Image,
    menu::{AboutMetadata, MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, WindowEvent,
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

/// One unsaved-work surface tracked by the dirty registry. `what` is the
/// human-readable label that appears in the quit-confirmation dialog
/// ("the weekly summary", "the quick-capture note").
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirtyEntry {
    pub dirty: bool,
    pub what: String,
}

/// Tauri-managed cross-window dirty-work registry. Each form-bearing route
/// (currently `/summary` and the capture popup) publishes its dirty state
/// here via the `set_window_dirty` command. `try_quit` reads the snapshot
/// synchronously and surfaces unsaved surfaces before exit.
///
/// Std Mutex (not tokio's) because reads/writes are trivial — no `.await`
/// is ever held under the lock.
#[derive(Default)]
pub struct DirtyRegistry(pub Mutex<HashMap<String, DirtyEntry>>);

/// PNG for the macOS menu bar template icon (book outline). Black-with-alpha
/// at 22pt @ 2x so the system can recolor for light/dark menu bar mode.
const TRAY_ICON_PNG: &[u8] = include_bytes!("../icons/tray-template@2x.png");

/// Label of the small quick-capture popup window. Must match `tauri.conf.json`.
const CAPTURE_WINDOW_LABEL: &str = "capture";

/// Menu-item id for the unified Quit action. Wired to BOTH the tray context
/// menu AND the macOS app menu (Cmd+Q). The `on_menu_event` listener routes
/// both occurrences through `try_quit`.
///
/// SAFETY: do not change this to `PredefinedMenuItem::quit` anywhere — that
/// predefined item dispatches AppKit's `terminate:` selector directly and
/// bypasses Tauri's event listener, defeating the unsaved-work guard.
const QUIT_MENU_ID: &str = "quit-app";

/// Menu-item id for the "Show Captain's Log" tray action. Pairs with
/// `restore_main_window` which flips activation policy back to `.Regular`
/// before unhiding the main window so the Dock icon reappears.
const SHOW_MAIN_MENU_ID: &str = "show-main";

// ---------------------------------------------------------------------------
// Close-flow helpers
// ---------------------------------------------------------------------------

/// Hide the main window and switch the app to `.Accessory` activation policy
/// (Dock icon disappears, app continues to run with only the tray icon).
/// Called from the main-window CloseRequested handler when the user clicks
/// the red traffic-light X.
fn hide_main_to_accessory(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
}

/// Restore the main window from `.Accessory` mode. Policy MUST flip back to
/// `.Regular` BEFORE `.show()` — otherwise the Dock icon stays hidden and
/// the window appears in a half-state (visible window, no Cmd-Tab presence).
///
/// Called from the tray "Show Captain's Log" menu item AND from the
/// notification "Write" action (so opening the summary from a notification
/// while the app is hidden does the right thing).
pub(crate) fn restore_main_window(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Quit flow: check the dirty registry, prompt if any surface has unsaved
/// work, otherwise exit immediately. Routed from BOTH the tray context
/// menu Quit item AND the macOS app menu Cmd+Q (via the unified
/// `on_menu_event` listener).
fn try_quit(app: &AppHandle) {
    let dirty_what: Vec<String> = {
        let registry = app.state::<DirtyRegistry>();
        let guard = registry.0.lock().expect("dirty registry mutex poisoned");
        guard
            .values()
            .filter(|e| e.dirty)
            .map(|e| e.what.clone())
            .collect()
    };

    if dirty_what.is_empty() {
        app.exit(0);
        return;
    }

    let what_text = format_dirty_list(&dirty_what);
    let app_h = app.clone();
    app.dialog()
        .message(format!(
            "You have unsaved work in {what_text}. Quit anyway? \
             Unsaved changes will be lost."
        ))
        .title("Quit Captain's Log?")
        // Cancel sits in the OK slot so Return / Escape default to the
        // safer choice (per Apple HIG: make the safest action default
        // when data could be lost). The callback receives `confirmed=true`
        // for Cancel — we exit only on the inverse.
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Cancel".to_string(),
            "Discard & Quit".to_string(),
        ))
        .kind(MessageDialogKind::Warning)
        .show(move |confirmed| {
            if !confirmed {
                app_h.exit(0);
            }
        });
}

/// Main-window close handler — Option B with auto-save.
///
/// Red X always hides silently. No prompt, ever. The capture popup is also
/// hidden if it's currently visible so both windows tuck away together
/// (otherwise the popup would float around with no main app behind it,
/// which looked weird in testing).
///
/// In-flight typed work is never lost: /summary auto-saves on 1.5s debounce,
/// and the capture popup auto-saves its draft to disk on the same cadence.
/// Cmd+Q / tray-menu Quit still routes through `try_quit`, which prompts
/// only inside the rare debounce gap when a save was pending. For hide
/// (red X), there's no data loss — the window's webview state persists
/// across hide, the summary file is written, the capture draft is written
/// — so the prompt was theatre, not protection.
fn handle_main_close(app: &AppHandle) {
    if let Some(capture) = app.get_webview_window(CAPTURE_WINDOW_LABEL) {
        if capture.is_visible().unwrap_or(false) {
            let _ = capture.hide();
        }
    }
    hide_main_to_accessory(app);
}

/// Seed NSUserDefaults flags that gate WKWebView's continuous spell-checker.
/// macOS apps that ship without a standard Edit > Spelling and Grammar menu
/// (Tauri's PredefinedMenuItem set doesn't include one — muda issue) have no
/// UI for users to toggle these flags, and the underlying defaults are FALSE
/// on a fresh bundle. WKWebView checks `WebContinuousSpellCheckingEnabled`
/// on each editable focus; with it false, `spellcheck="true"` on the HTML
/// element is a no-op and the right-click "Show Spelling and Grammar"
/// affordance never appears.
///
/// We call standardUserDefaults().setBool(true, forKey:) for the three
/// relevant keys — this both writes to disk AND updates NSUserDefaults's
/// in-memory cache, so WKWebView sees the new value when the user first
/// focuses an editable.
///
/// Idempotent; safe to call on every launch. Wrapped in unsafe FFI because
/// objc2 needs explicit unsafe for `msg_send!`.
///
/// (Known follow-up: tauri-apps/tauri#7705 reports that even with the
/// checker active, the red squiggle underlines don't render inside Tauri's
/// WKWebView. The right-click suggestion menu is the authoritative
/// behavior; squiggles are out of scope.)
#[cfg(target_os = "macos")]
fn seed_spellcheck_defaults() {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    use objc2_foundation::{NSString, NSUserDefaults};

    // SAFETY: NSUserDefaults.standardUserDefaults returns a singleton
    // shared instance; setBool:forKey: is a safe Obj-C method. NSString
    // construction is the standard objc2-foundation idiom.
    unsafe {
        let defaults = NSUserDefaults::standardUserDefaults();
        for key in [
            "WebContinuousSpellCheckingEnabled",
            "WebGrammarCheckingEnabled",
            "NSAllowContinuousSpellChecking",
        ] {
            let ns_key = NSString::from_str(key);
            let _: () = msg_send![&*defaults, setBool: true, forKey: &*ns_key];
        }
        // Touch _ so the AnyObject import isn't flagged as unused even
        // if some objc2 version trims the msg_send return path.
        let _ = std::ptr::null::<AnyObject>();
    }
}

#[cfg(not(target_os = "macos"))]
fn seed_spellcheck_defaults() {}

/// English list-join with serial comma: ["A"] → "A"; ["A","B"] → "A and B";
/// ["A","B","C"] → "A, B, and C". Only used in the quit-confirmation copy.
fn format_dirty_list(items: &[String]) -> String {
    match items.len() {
        0 => String::new(),
        1 => items[0].clone(),
        2 => format!("{} and {}", items[0], items[1]),
        _ => {
            let (last, rest) = items.split_last().expect("len >= 3");
            format!("{}, and {}", rest.join(", "), last)
        }
    }
}

// ---------------------------------------------------------------------------
// Tauri app entry point
// ---------------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        // Auto-saves and restores window size/position/maximized state across
        // launches. VISIBLE flag dropped (the default would force-show every
        // window on launch, overriding tauri.conf.json's `"visible": false`
        // on the capture popup); skip_initial_state belt-and-suspenders for
        // the capture window. Geometry still saves on CloseRequested + Exit.
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(StateFlags::all() - StateFlags::VISIBLE)
                .skip_initial_state(CAPTURE_WINDOW_LABEL)
                .build(),
        )
        // Disable Tauri's default macOS app menu. It uses PredefinedMenuItem::quit
        // which dispatches AppKit's `terminate:` selector directly and bypasses
        // our unsaved-work guard. We install our own app menu below in setup().
        .enable_macos_default_menu(false)
        // Single menu-event listener for ALL menu sources (tray menu + app menu).
        // Per Tauri docs this fires for ANY menu event regardless of which menu
        // emitted it — so Cmd+Q in our app menu and "Quit Captain's Log" in the
        // tray context menu both land here.
        .on_menu_event(|app, event| match event.id().as_ref() {
            SHOW_MAIN_MENU_ID => restore_main_window(app),
            QUIT_MENU_ID => try_quit(app),
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            commands::create_note,
            commands::read_week,
            commands::write_week,
            commands::list_years,
            commands::list_weeks,
            commands::get_labels,
            commands::get_settings,
            commands::complete_first_run,
            commands::update_settings,
            commands::get_current_year_week,
            commands::get_weekly_summary,
            commands::update_weekly_summary,
            commands::set_window_dirty,
            commands::load_capture_draft,
            commands::save_capture_draft,
            commands::clear_capture_draft,
            commands::get_sent_record,
            commands::compose_weekly_email,
            commands::mark_weekly_summary_sent,
            commands::get_summary_hash,
            spellcheck::check_spelling,
        ])
        .setup(|app| {
            // Seed NSUserDefaults so WKWebView's continuous spell-checker
            // turns on for editable HTML controls. Without this AND a real
            // Edit menu (installed below), spellcheck="true" on the
            // textareas is a no-op. Idempotent — runs every launch.
            seed_spellcheck_defaults();

            // Sweep stale .eml files from the Send-to-manager fallback path.
            // Fire-and-forget; failures are logged but never block startup
            // (the janitor's worst-case cost is leaving a few KB of temp
            // drafts behind, not anything user-visible).
            email::prune_old_eml_files();

            // Cross-window dirty registry — read at quit time by try_quit.
            app.manage(DirtyRegistry::default());

            // Sanity-check the macOS bundle identity needed by
            // UNUserNotificationCenter. Tauri's embed_plist + our bundle.macOS
            // codesign config provide the identifier in both `tauri dev` and
            // bundled `.app` builds. No-op on other platforms.
            reminders::check_macos_bundle();

            // Determine the journal root: from app-settings.json if present,
            // otherwise the platform default (~/Documents/CaptainsLog/).
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

            // Spawn the weekly reminder task if enabled.
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

            // Install a complete macOS app menu — replaces Tauri's default
            // (disabled via `.enable_macos_default_menu(false)` on the
            // Builder above). Four submenus:
            //
            //   1. Captain's Log — About, Services, Hide* (×3), and our
            //      custom Quit item that flows through `try_quit`. CRITICAL:
            //      do NOT use `PredefinedMenuItem::quit` here — it dispatches
            //      AppKit's `terminate:` directly and bypasses our
            //      unsaved-work guard.
            //   2. Edit — Undo/Redo + Cut/Copy/Paste/SelectAll. Restores the
            //      standard editing shortcuts AND, more importantly, gives
            //      AppKit the responder-chain context it expects to surface
            //      the spell-check context menu on right-click in editable
            //      controls. Without an Edit menu, WKWebView's right-click
            //      "Show Spelling and Grammar" affordance never appears.
            //   3. View — Toggle full screen.
            //   4. Window — Minimize / Zoom / Bring All to Front /
            //      Close Window. Note Cmd+W closes the focused window but
            //      doesn't quit; Cmd+Q stays the only path through the
            //      unsaved-work guard.
            //
            // PredefinedMenuItem variants come from muda (re-exported via
            // tauri::menu) and dispatch to the standard AppKit responder
            // selectors — we never need to intercept them.
            #[cfg(target_os = "macos")]
            {
                let quit_item = MenuItemBuilder::new("Quit Captain\u{2019}s Log")
                    .id(QUIT_MENU_ID)
                    .accelerator("CmdOrCtrl+Q")
                    .build(app)?;

                let app_submenu = SubmenuBuilder::new(app, "Captain\u{2019}s Log")
                    .item(&PredefinedMenuItem::about(
                        app,
                        None,
                        Some(AboutMetadata::default()),
                    )?)
                    .separator()
                    .item(&PredefinedMenuItem::services(app, None)?)
                    .separator()
                    .item(&PredefinedMenuItem::hide(app, None)?)
                    .item(&PredefinedMenuItem::hide_others(app, None)?)
                    .item(&PredefinedMenuItem::show_all(app, None)?)
                    .separator()
                    .item(&quit_item)
                    .build()?;

                let edit_submenu = SubmenuBuilder::new(app, "Edit")
                    .item(&PredefinedMenuItem::undo(app, None)?)
                    .item(&PredefinedMenuItem::redo(app, None)?)
                    .separator()
                    .item(&PredefinedMenuItem::cut(app, None)?)
                    .item(&PredefinedMenuItem::copy(app, None)?)
                    .item(&PredefinedMenuItem::paste(app, None)?)
                    .item(&PredefinedMenuItem::select_all(app, None)?)
                    .build()?;

                let view_submenu = SubmenuBuilder::new(app, "View")
                    .item(&PredefinedMenuItem::fullscreen(app, None)?)
                    .build()?;

                let window_submenu = SubmenuBuilder::new(app, "Window")
                    .item(&PredefinedMenuItem::minimize(app, None)?)
                    .item(&PredefinedMenuItem::maximize(app, None)?)
                    .separator()
                    .item(&PredefinedMenuItem::bring_all_to_front(app, None)?)
                    .separator()
                    .item(&PredefinedMenuItem::close_window(app, None)?)
                    .build()?;

                let app_menu = MenuBuilder::new(app)
                    .item(&app_submenu)
                    .item(&edit_submenu)
                    .item(&view_submenu)
                    .item(&window_submenu)
                    .build()?;
                app.set_menu(app_menu)?;
            }

            // Intercept the main window's red traffic-light X. Hide instead
            // of destroy, switch to .Accessory (Dock icon disappears), and
            // optionally prompt if the capture popup has unsaved text.
            {
                let main_window = app
                    .get_webview_window("main")
                    .expect("main window declared in tauri.conf.json");
                let app_handle = app.handle().clone();
                main_window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        handle_main_close(&app_handle);
                    }
                });
            }

            // Capture popup close button (red-X / Cmd-W) hides instead of
            // destroying. Default Tauri behavior would drop the window from
            // the app's registry, after which `get_webview_window("capture")`
            // returns None and the tray click handler silently no-ops.
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

            // Tray context menu — shown on right-click. Left-click still
            // toggles the capture popup (existing behavior). The Quit item
            // shares `QUIT_MENU_ID` with the app menu's Cmd+Q; the unified
            // on_menu_event listener routes both to `try_quit`.
            let tray_menu = MenuBuilder::new(app)
                .text(SHOW_MAIN_MENU_ID, "Show Captain's Log")
                .separator()
                .text(QUIT_MENU_ID, "Quit Captain's Log")
                .build()?;

            // Tray icon — left-click toggles the quick-capture popup window;
            // right-click shows the context menu (Show / Quit).
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
                .menu(&tray_menu)
                // Critical: default is true, which would steal the left-click
                // and show the menu instead of toggling the capture popup.
                .show_menu_on_left_click(false)
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
