//! Spell-check command — bridges WKWebView's textareas to macOS's native
//! `NSSpellChecker`.
//!
//! ## Why this exists
//!
//! WKWebView honors HTML `spellcheck="true"` for the right-click suggestion
//! menu, but Tauri's webview hides the red wavy underlines that normally
//! mark misspelled words (upstream bug tauri-apps/tauri#7705). Without
//! visual feedback the spell-checker isn't actually useful day-to-day.
//!
//! Workaround: the frontend mirrors each textarea into a positioned `<div>`
//! overlay and draws its own wavy-red underlines on misspelled spans. That
//! component needs a list of `(start, length)` ranges in UTF-16 code units
//! (matching JS string indexing). This module produces that list by calling
//! `NSSpellChecker::checkSpellingOfString_startingAt` in a loop and
//! returning every range it finds.
//!
//! ## Threading
//!
//! `NSSpellChecker.shared` is documented as main-thread-only. Tauri command
//! handlers run on tokio workers, so we hop to the main thread via
//! `app.run_on_main_thread()` and shuttle the result back over a oneshot.
//!
//! ## Indexing
//!
//! NSString stores UTF-16. NSRange location/length are UTF-16 code units.
//! JavaScript string indices are also UTF-16. So the values returned here
//! pass straight through to the frontend without conversion — they map
//! correctly to `text.slice(start, start + length)` in JS, even when the
//! string contains emoji or other astral-plane characters.
//!
//! ## Cross-platform
//!
//! macOS-only today. Non-macOS targets return an empty Vec — the overlay
//! component just renders without highlights, which is the same UX as
//! "no misspellings found." A Windows backend would call
//! `Windows.Globalization.SpellChecker`; Linux would use hunspell-rs or
//! similar. Behind the same `check_spelling` command signature, neither
//! affects the frontend.

use serde::Serialize;
use tauri::AppHandle;

/// A misspelled span. `start` and `length` are UTF-16 code-unit offsets
/// into the checked text (matches JavaScript's `String.prototype.slice`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpellRange {
    pub start: usize,
    pub length: usize,
}

/// Find every misspelled span in `text`. Empty input → empty result.
/// macOS-only today; other platforms return `Ok(vec![])`.
#[tauri::command]
pub async fn check_spelling(
    app: AppHandle,
    text: String,
) -> Result<Vec<SpellRange>, String> {
    if text.is_empty() {
        return Ok(Vec::new());
    }

    #[cfg(target_os = "macos")]
    {
        let (tx, rx) = tokio::sync::oneshot::channel();
        app.run_on_main_thread(move || {
            let result = check_spelling_macos(&text);
            let _ = tx.send(result);
        })
        .map_err(|e| e.to_string())?;
        rx.await.map_err(|e| e.to_string())
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        let _ = text;
        Ok(Vec::new())
    }
}

#[cfg(target_os = "macos")]
fn check_spelling_macos(text: &str) -> Vec<SpellRange> {
    use objc2_app_kit::NSSpellChecker;
    use objc2_foundation::NSString;

    // Cap iterations defensively. A document with > 10k misspellings is
    // not a real-world input; if we ever hit this we'd rather degrade
    // gracefully than hang the main thread.
    const MAX_RANGES: usize = 10_000;

    let mut ranges = Vec::new();

    // NSSpellChecker.sharedSpellChecker returns the process-wide singleton,
    // safe to call from the main thread where this function runs.
    let checker = NSSpellChecker::sharedSpellChecker();
    let ns_text = NSString::from_str(text);
    let text_len_utf16 = ns_text.length();
    let mut start: usize = 0;

    while start < text_len_utf16 && ranges.len() < MAX_RANGES {
        let range = checker.checkSpellingOfString_startingAt(&ns_text, start as isize);
        if range.length == 0 {
            break;
        }
        ranges.push(SpellRange {
            start: range.location,
            length: range.length,
        });
        let next = range.location.saturating_add(range.length);
        if next <= start {
            // Defensive: should not happen, but prevents an infinite loop
            // if the API ever returns a non-advancing range.
            break;
        }
        start = next;
    }

    ranges
}
