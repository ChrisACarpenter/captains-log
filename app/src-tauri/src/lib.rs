// Captain's Log — Tauri backend entry point.
//
// Module layout (filled in as we build):
//   storage/   — StorageBackend trait + LocalFilesystem impl
//   notes/     — Notes API (create / read / update / parse markdown)
//   labels/    — Label index management
//   commands/  — Tauri command handlers exposed to the frontend
//
// For now this is a bare scaffold — no commands wired up yet.

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
