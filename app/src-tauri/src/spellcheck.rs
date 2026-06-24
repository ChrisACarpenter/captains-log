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
    use objc2_foundation::{NSRange, NSString, NSTextCheckingType, NSTextCheckingTypes};

    // Defensive: a document with > 10k findings is not a real-world input.
    // Truncate rather than hang the main thread.
    const MAX_RANGES: usize = 10_000;

    let checker = NSSpellChecker::sharedSpellChecker();
    let ns_text = NSString::from_str(text);
    let text_len_utf16 = ns_text.length();
    if text_len_utf16 == 0 {
        return Vec::new();
    }

    // ## Why this API and not checkSpellingOfString:startingAt:
    //
    // The narrower `checkSpellingOfString:startingAt:` only returns
    // NSTextCheckingTypeSpelling-class results — dictionary misses like
    // "teh", "thatll", "awdawdawd". Apple's NSSpellChecker routes
    // missing-apostrophe contractions ("dont" -> "don't", "im" -> "I'm")
    // through NSTextCheckingTypeCorrection, and contextual issues like
    // "its" vs "it's" through NSTextCheckingTypeGrammar. Neither channel
    // is reachable through the narrow loop API.
    //
    // Pages, Mail, and TextEdit all use `checkString:range:types:...`
    // with a broader type mask, which is why they flag the same words our
    // editor was missing. We match that here.
    //
    // Type mask:
    //   Spelling   — dictionary misses ("teh", "awdawdawd", "thatll")
    //   Correction — autocorrect candidates ("dont", "im", "cant")
    //   Grammar    — contextual issues ("its" vs "it's", agreement)
    //
    // Deliberately omitted: Replacement, Quote, Dash, Link, Date, Address —
    // those are smart-substitution suggestions / data detectors, not
    // errors. Including them would draw spurious squiggles under prose.
    let types: NSTextCheckingTypes = (NSTextCheckingType::Spelling
        | NSTextCheckingType::Correction
        | NSTextCheckingType::Grammar)
        .0;

    let full_range = NSRange {
        location: 0,
        length: text_len_utf16,
    };

    // SAFETY: NSSpellChecker.sharedSpellChecker is documented main-thread
    // only; we're invoked via `app.run_on_main_thread` (see
    // `check_spelling` above). The out-params for orthography and word
    // count are passed null because we don't need them.
    let results = unsafe {
        checker.checkString_range_types_options_inSpellDocumentWithTag_orthography_wordCount(
            &ns_text,
            full_range,
            types,
            None,                 // options
            0,                    // spell-document tag (0 = no doc context)
            None,                 // orthography out
            std::ptr::null_mut(), // word_count out
        )
    };

    let count = results.count();
    let mut ranges = Vec::with_capacity(count.min(MAX_RANGES));
    for result in results.iter().take(MAX_RANGES) {
        let r = result.range();
        if r.length == 0 {
            continue;
        }
        // Clamp defensively in case the engine ever returns a range that
        // extends past the string (shouldn't happen — guarding is cheap).
        let end = r.location.saturating_add(r.length).min(text_len_utf16);
        if end <= r.location {
            continue;
        }
        ranges.push(SpellRange {
            start: r.location,
            length: end - r.location,
        });
    }

    // The engine returns results in document order but Grammar + Correction
    // findings can overlap on the same span. De-dup exact duplicates so the
    // frontend doesn't double-decorate the same word.
    ranges.sort_by_key(|r| (r.start, r.length));
    ranges.dedup_by(|a, b| a.start == b.start && a.length == b.length);

    ranges
}
