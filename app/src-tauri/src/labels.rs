//! Label index management.
//!
//! Every Note can carry labels in two places:
//!   - The dedicated **Labels field** (parsed by the frontend, sent in the
//!     `labels` array of the [`Note`])
//!   - Inline **`#hashtags`** in the body prose
//!
//! Both contribute to the global label index at `.metadata/labels.json`,
//! which feeds autocomplete (Phase 2) and "all labels in journal" queries
//! later. The index is the convenience layer; the markdown files remain
//! the source of truth.
//!
//! Schema (see `docs/label-system.md`):
//!
//! ```json
//! {
//!   "version": 1,
//!   "labels": [
//!     { "name": "release", "count": 47,
//!       "first_used": "2026-01-12", "last_used": "2026-06-18" }
//!   ]
//! }
//! ```

use std::ops::Range;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::notes::Note;
use crate::settings::deserialize_hex6_option;
use crate::storage::{StorageBackend, StorageError, StorageResult};

const METADATA_FILE: &str = "labels.json";
const CURRENT_VERSION: u32 = 1;

/// One label's aggregate stats.
///
/// Serialized in camelCase to match the rest of the IPC + on-disk JSON.
/// The serde `alias` annotations keep older `labels.json` files (which used
/// snake_case `first_used` / `last_used` from the initial release) parseable
/// — they get normalized to camelCase on the next write.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LabelEntry {
    pub name: String,
    pub count: u32,
    #[serde(alias = "first_used")]
    pub first_used: NaiveDate,
    #[serde(alias = "last_used")]
    pub last_used: NaiveDate,
    /// Optional persisted per-label hex color (`#rrggbb`, lowercase).
    ///
    /// Phase 2.8+ "Colorful Labels": when the user sets an explicit
    /// override (or the lazy-assignment hash auto-picks one and writes it
    /// back), it lives here. Most labels won't have a color — the chip
    /// renderer falls back to the deterministic hash either way. The
    /// `skip_serializing_if = None` keeps `labels.json` tidy for the
    /// uncolorized majority; `serde(default)` lets legacy files written
    /// before this field existed load cleanly as `None`. Validation
    /// reuses `settings::deserialize_hex6_option` so the same hex rules
    /// apply here as for `CustomTheme` primaries (lowercase-normalized,
    /// no 3-digit shorthand, strict `#rrggbb`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(deserialize_with = "deserialize_hex6_option")]
    pub color: Option<String>,
}

/// The on-disk label index.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LabelIndex {
    pub version: u32,
    pub labels: Vec<LabelEntry>,
}

impl Default for LabelIndex {
    fn default() -> Self {
        Self {
            version: CURRENT_VERSION,
            labels: Vec::new(),
        }
    }
}

impl LabelIndex {
    /// Load the index from storage, returning a fresh empty index if the
    /// file is absent. **Corrupted JSON falls back to empty** with a stderr
    /// warning — losing autocomplete state is preferable to refusing the
    /// user's note-save. A proper rebuild-from-scan is Phase 2.
    pub async fn load<B: StorageBackend + ?Sized>(backend: &B) -> StorageResult<Self> {
        match backend.read_metadata(METADATA_FILE).await? {
            Some(content) => match serde_json::from_str::<LabelIndex>(&content) {
                Ok(idx) => Ok(idx),
                Err(e) => {
                    eprintln!(
                        "labels.json failed to parse ({}). Starting with an empty index. \
                        Phase 2 will add an automatic rebuild from the weekly files.",
                        e
                    );
                    Ok(Self::default())
                }
            },
            None => Ok(Self::default()),
        }
    }

    pub async fn save<B: StorageBackend + ?Sized>(&self, backend: &B) -> StorageResult<()> {
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| StorageError::Serde(e.to_string()))?;
        backend.write_metadata(METADATA_FILE, &content).await
    }

    /// Record one occurrence of `name` on `date`. New labels are inserted;
    /// existing labels have their counts and date ranges updated.
    pub fn touch(&mut self, name: &str, date: NaiveDate) {
        if let Some(entry) = self.labels.iter_mut().find(|e| e.name == name) {
            entry.count = entry.count.saturating_add(1);
            if date > entry.last_used {
                entry.last_used = date;
            }
            if date < entry.first_used {
                entry.first_used = date;
            }
        } else {
            self.labels.push(LabelEntry {
                name: name.to_string(),
                count: 1,
                first_used: date,
                last_used: date,
                color: None,
            });
        }
        self.sort_for_autocomplete();
    }

    /// Sort by `last_used` desc, then `count` desc, then alphabetical asc.
    /// Matches the autocomplete ranking from `docs/label-system.md`.
    fn sort_for_autocomplete(&mut self) {
        self.labels.sort_by(|a, b| {
            b.last_used
                .cmp(&a.last_used)
                .then(b.count.cmp(&a.count))
                .then(a.name.cmp(&b.name))
        });
    }
}

// ---------------------------------------------------------------------------
// Label extraction
// ---------------------------------------------------------------------------

/// Pull every label off a Note: the explicit `labels` field AND inline
/// `#hashtags` in the body. Result is deduplicated and stable-ordered
/// (insertion order with duplicates removed).
pub fn extract_all_labels(note: &Note) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(note.labels.len() + 4);
    let mut seen = std::collections::HashSet::new();

    for label in &note.labels {
        let normalized = normalize_label(label);
        if !normalized.is_empty() && seen.insert(normalized.clone()) {
            out.push(normalized);
        }
    }

    for label in extract_inline_labels(&note.body) {
        if seen.insert(label.clone()) {
            out.push(label);
        }
    }

    out
}

/// Strip a leading `#` and trim whitespace.
fn normalize_label(s: &str) -> String {
    s.trim().trim_start_matches('#').to_string()
}

/// Find every inline `#label` token in `text`. A token must:
///   - start at the beginning of a string or directly after whitespace
///   - have at least one character after the `#`
///   - contain only word characters (letters, digits, underscore) and hyphens
///
/// The `#` itself is NOT included in the output strings.
pub fn extract_inline_labels(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let prev_is_boundary = i == 0
            || matches!(
                bytes[i - 1],
                b' ' | b'\t' | b'\n' | b'\r' | b'(' | b'[' | b'{' | b','
            );
        if bytes[i] == b'#' && prev_is_boundary {
            let start = i + 1;
            let mut end = start;
            while end < bytes.len() && is_label_char(bytes[end]) {
                end += 1;
            }
            if end > start {
                // SAFETY: bytes consumed are ASCII (label chars), so slicing is valid UTF-8.
                if let Ok(s) = std::str::from_utf8(&bytes[start..end]) {
                    out.push(s.to_string());
                }
            }
            i = end.max(i + 1);
        } else {
            i += 1;
        }
    }
    out
}

#[inline]
fn is_label_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
}

// ---------------------------------------------------------------------------
// Byte-range scanner for explicit-labels sites
// ---------------------------------------------------------------------------

/// Where in a weekly file an explicit-labels chunk lives.
///
/// Two recognized shapes, both produced by `notes.rs`:
///
/// 1. **`NoteLabelsLine`** — the `**Labels:** #a #b\n` line that sits directly
///    under a `### YYYY-MM-DD HH:MM — Title` Note heading (see
///    `Note::to_markdown`). The site's `byte_range` covers the full line
///    *including* its trailing newline so that replacement / removal does not
///    leave a blank stub line behind.
///
/// 2. **`SummaryLabelsSubsection`** — the `### Labels` subsection inside the
///    `## Weekly Summary` section (see `weekly_file_scaffold` and
///    `render_weekly_summary`). The site's `byte_range` covers the heading
///    line, the one-paragraph body of space-separated `#tag` tokens, and the
///    trailing whitespace up to the next `### ` / `## ` heading (or EOF) so
///    that a rename / delete operation can splice in a freshly-rendered
///    replacement without disturbing surrounding subsections.
///
/// Inline `#hashtag` tokens scattered through Note bodies or summary prose are
/// **deliberately not** scanned here — Phase 3a's delete-cascade only strips
/// explicit labels arrays (see Chris's locked-decision #2). Inline tags
/// remain in the prose where the user wrote them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelSite {
    pub kind: LabelSiteKind,
    pub byte_range: Range<usize>,
    /// Label names in the order they appear, with the leading `#` stripped.
    pub names: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelSiteKind {
    NoteLabelsLine,
    SummaryLabelsSubsection,
}

const NOTE_LABELS_PREFIX: &str = "**Labels:**";

/// Scan `file_content` and return every explicit-labels site (Note line +
/// Weekly Summary subsection) in document order.
///
/// The returned byte ranges are intended to be reused by rename and delete
/// passes for in-place editing — splicing a replacement string into
/// `file_content[range]` produces the post-edit file content.
pub fn scan_label_sites(file_content: &str) -> Vec<LabelSite> {
    let mut sites = Vec::new();
    let bytes = file_content.as_bytes();

    // --- Pass 1: Note labels lines ---------------------------------------
    //
    // A Note heading is `### YYYY-MM-DD HH:MM` optionally followed by ` — Title`.
    // The labels line, when present, is the next non-empty line and begins
    // with `**Labels:**`. We don't require the line to be the literal next
    // byte after the heading — a renderer in the future might leave a blank
    // line between them — but we do require it to appear before any blank
    // line *or* the next heading.
    let mut search_from = 0usize;
    while let Some(rel_idx) = file_content[search_from..].find("\n### ") {
        let heading_line_start = search_from + rel_idx + 1; // skip the '\n'
        // The heading needs to start with a date — distinguishes Note
        // headings from the Weekly Summary subsection headings
        // (`### Key accomplishments`, `### Labels`, …).
        let heading_rest = &file_content[heading_line_start + 4..]; // skip "### "
        let looks_like_note = heading_rest
            .as_bytes()
            .get(..10)
            .is_some_and(is_iso_date_prefix);
        // Advance the outer cursor past this heading line so the next
        // iteration moves forward regardless of whether this turned out to
        // be a Note or a Summary subsection.
        let next_newline_after_heading = file_content[heading_line_start..]
            .find('\n')
            .map(|i| heading_line_start + i + 1)
            .unwrap_or(file_content.len());
        search_from = next_newline_after_heading;

        if !looks_like_note {
            continue;
        }

        // Now scan forward from the start of the line after the heading,
        // skipping at most a small run of blank lines, looking for a
        // `**Labels:**` line. Bail out if we hit another `###`/`##`/`#`
        // heading first.
        let probe = next_newline_after_heading;
        loop {
            if probe >= bytes.len() {
                break;
            }
            let line_end = file_content[probe..]
                .find('\n')
                .map(|i| probe + i)
                .unwrap_or(bytes.len());
            let line = &file_content[probe..line_end];
            let trimmed = line.trim_start();
            if trimmed.is_empty() {
                // Blank line — for a Note rendered by `to_markdown`, the
                // labels line is the very next line, not separated by a
                // blank. If we see a blank line first, there is no labels
                // line for this Note. Move on.
                break;
            }
            if trimmed.starts_with("###") || trimmed.starts_with("## ") || trimmed.starts_with("# ") {
                // Reached the next heading — no labels line for this Note.
                break;
            }
            if let Some(rest) = line.strip_prefix(NOTE_LABELS_PREFIX) {
                let names = parse_tag_tokens(rest);
                // Range covers the full line including its trailing '\n'
                // (or EOF if there isn't one).
                let end = if line_end < bytes.len() {
                    line_end + 1
                } else {
                    line_end
                };
                sites.push(LabelSite {
                    kind: LabelSiteKind::NoteLabelsLine,
                    byte_range: probe..end,
                    names,
                });
            }
            // Whether we matched or not, only inspect the immediately-
            // following line under a Note heading. The labels line, by the
            // shape `to_markdown` produces, is always the very first line
            // after the heading.
            break;
        }
    }

    // Also handle a Note heading that starts at byte 0 (no preceding '\n').
    if file_content.starts_with("### ") {
        let heading_rest = &file_content[4..];
        if heading_rest
            .as_bytes()
            .get(..10)
            .is_some_and(is_iso_date_prefix)
        {
            if let Some(nl) = file_content.find('\n') {
                let probe = nl + 1;
                if probe < bytes.len() {
                    let line_end = file_content[probe..]
                        .find('\n')
                        .map(|i| probe + i)
                        .unwrap_or(bytes.len());
                    let line = &file_content[probe..line_end];
                    if let Some(rest) = line.strip_prefix(NOTE_LABELS_PREFIX) {
                        let names = parse_tag_tokens(rest);
                        let end = if line_end < bytes.len() {
                            line_end + 1
                        } else {
                            line_end
                        };
                        // Avoid double-recording if Pass 1 already caught
                        // this (it can't, since Pass 1 keys off "\n### "
                        // and this branch covers the no-preceding-newline
                        // case, but be safe).
                        if !sites.iter().any(|s| s.byte_range.start == probe) {
                            sites.insert(
                                0,
                                LabelSite {
                                    kind: LabelSiteKind::NoteLabelsLine,
                                    byte_range: probe..end,
                                    names,
                                },
                            );
                        }
                    }
                }
            }
        }
    }

    // --- Pass 2: Weekly Summary `### Labels` subsection ------------------
    //
    // Bounded by the `## Weekly Summary` header on the top and either the
    // `## Weekly Notes` header (or any other `## `) or EOF on the bottom.
    // Inside that section we look for `### Labels` as a line and treat the
    // body up to the next `### ` / `## ` heading (or section end) as the
    // subsection contents.
    if let Some(summary_start) = file_content.find("## Weekly Summary") {
        // Section ends at the next `\n## ` (next H2) or EOF.
        let section_end = file_content[summary_start..]
            .find("\n## ")
            .map(|i| summary_start + i + 1) // point at the '#' of the next H2
            .unwrap_or(file_content.len());

        let section = &file_content[summary_start..section_end];
        // Match `### Labels` at the start of a line (either the very first
        // char or after a '\n'), followed by end-of-line.
        let mut search_pos = 0usize;
        while let Some(rel) = section[search_pos..].find("### Labels") {
            let abs_in_section = search_pos + rel;
            let is_line_start = abs_in_section == 0
                || section.as_bytes()[abs_in_section - 1] == b'\n';
            // Must be exactly "### Labels" followed by newline or EOF, not
            // "### Labels of Doom".
            let after = abs_in_section + "### Labels".len();
            let ends_cleanly = match section.as_bytes().get(after) {
                None => true,
                Some(&b) => b == b'\n' || b == b'\r',
            };
            if is_line_start && ends_cleanly {
                let heading_line_start = summary_start + abs_in_section;
                // End of subsection: first newline that is followed by
                // `### ` or `## `, or EOF / end of summary section.
                let body_search_start = heading_line_start + "### Labels".len();
                // Skip the heading's own newline to start scanning body.
                let after_heading_newline = file_content[body_search_start..]
                    .find('\n')
                    .map(|i| body_search_start + i + 1)
                    .unwrap_or(section_end);

                // Find next heading boundary within section.
                let mut sub_end = section_end;
                let mut cursor = after_heading_newline;
                while cursor < section_end {
                    let line_end = file_content[cursor..section_end]
                        .find('\n')
                        .map(|i| cursor + i)
                        .unwrap_or(section_end);
                    let line = &file_content[cursor..line_end];
                    let trimmed = line.trim_start();
                    if trimmed.starts_with("### ") || trimmed.starts_with("## ") {
                        sub_end = cursor;
                        break;
                    }
                    if line_end >= section_end {
                        sub_end = section_end;
                        break;
                    }
                    cursor = line_end + 1;
                }

                // Parse tag tokens out of the body.
                let body = &file_content[after_heading_newline..sub_end];
                let names = parse_tag_tokens(body);

                sites.push(LabelSite {
                    kind: LabelSiteKind::SummaryLabelsSubsection,
                    byte_range: heading_line_start..sub_end,
                    names,
                });

                search_pos = abs_in_section + "### Labels".len();
                continue;
            }
            search_pos = abs_in_section + "### Labels".len();
        }
    }

    sites.sort_by_key(|s| s.byte_range.start);
    sites
}

/// Parse `#tag1 #tag2` tokens out of a fragment of text. Mirrors the rules
/// of [`extract_inline_labels`] but is lenient about leading whitespace
/// (these sites are explicit, not free-prose).
fn parse_tag_tokens(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for tok in text.split_whitespace() {
        let stripped = tok.trim_start_matches('#');
        // Trim trailing punctuation that wouldn't be a label char.
        let end = stripped
            .as_bytes()
            .iter()
            .position(|b| !is_label_char(*b))
            .unwrap_or(stripped.len());
        let name = &stripped[..end];
        if !name.is_empty() {
            out.push(name.to_string());
        }
    }
    out
}

/// Cheap check: does this 10-byte prefix look like `YYYY-MM-DD`?
///
/// `pub(crate)` so drill-down commands in `commands.rs` (Phase 3a Slice 1)
/// can reuse it when discriminating Note headings from Summary subsection
/// headings while walking backward from a `LabelSite` byte range.
pub(crate) fn is_iso_date_prefix(b: &[u8]) -> bool {
    b.len() == 10
        && b[0].is_ascii_digit()
        && b[1].is_ascii_digit()
        && b[2].is_ascii_digit()
        && b[3].is_ascii_digit()
        && b[4] == b'-'
        && b[5].is_ascii_digit()
        && b[6].is_ascii_digit()
        && b[7] == b'-'
        && b[8].is_ascii_digit()
        && b[9].is_ascii_digit()
}

// ---------------------------------------------------------------------------
// High-level integration helper
// ---------------------------------------------------------------------------

/// Convenience: load the index, record the labels from `note` against `date`,
/// save the index back. Used by `create_note` after a successful write.
pub async fn record_note_labels<B: StorageBackend + ?Sized>(
    backend: &B,
    note: &Note,
    date: NaiveDate,
) -> StorageResult<()> {
    let labels = extract_all_labels(note);
    if labels.is_empty() {
        return Ok(());
    }

    let mut index = LabelIndex::load(backend).await?;
    for label in labels {
        index.touch(&label, date);
    }
    index.save(backend).await
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::LocalFilesystem;
    use chrono::FixedOffset;
    use tempfile::TempDir;

    fn ts(s: &str) -> chrono::DateTime<FixedOffset> {
        chrono::DateTime::parse_from_rfc3339(s).expect("parse timestamp")
    }

    fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    // ---- extract_inline_labels ----

    #[test]
    fn inline_labels_at_start_of_text() {
        assert_eq!(extract_inline_labels("#release happened today"), vec!["release"]);
    }

    #[test]
    fn inline_labels_after_whitespace() {
        let out = extract_inline_labels("Today we shipped #release and #mage updates.");
        assert_eq!(out, vec!["release", "mage"]);
    }

    #[test]
    fn inline_labels_ignores_url_fragments() {
        let out = extract_inline_labels("see https://example.com/foo#section for details");
        assert!(out.is_empty(), "URL fragment shouldn't be a label");
    }

    #[test]
    fn inline_labels_supports_hyphens_underscores_digits() {
        let out = extract_inline_labels("#journal-app and #qa_skill and #v2");
        assert_eq!(out, vec!["journal-app", "qa_skill", "v2"]);
    }

    #[test]
    fn inline_labels_after_punctuation_boundary() {
        let out = extract_inline_labels("Labels include (#release, #mage).");
        assert_eq!(out, vec!["release", "mage"]);
    }

    #[test]
    fn inline_labels_stops_at_non_label_char() {
        let out = extract_inline_labels("#release.next #mage!stop");
        assert_eq!(out, vec!["release", "mage"]);
    }

    #[test]
    fn inline_labels_empty_hash_ignored() {
        assert!(extract_inline_labels("# heading").is_empty());
        assert!(extract_inline_labels("just a # sign").is_empty());
    }

    // ---- extract_all_labels ----

    #[test]
    fn all_labels_combines_field_and_body() {
        let note = Note {
            timestamp: ts("2026-06-18T14:23:00-04:00"),
            title: None,
            labels: vec!["journal-app".to_string(), "project".to_string()],
            body: "Working on #release with #mage today.".to_string(),
        };
        let labels = extract_all_labels(&note);
        assert_eq!(labels, vec!["journal-app", "project", "release", "mage"]);
    }

    #[test]
    fn all_labels_dedupes_across_sources() {
        let note = Note {
            timestamp: ts("2026-06-18T14:23:00-04:00"),
            title: None,
            labels: vec!["release".to_string()],
            body: "Note about #release things.".to_string(),
        };
        let labels = extract_all_labels(&note);
        assert_eq!(labels, vec!["release"]);
    }

    #[test]
    fn all_labels_strips_hash_prefix_from_field() {
        // The frontend usually strips the '#', but be defensive.
        let note = Note {
            timestamp: ts("2026-06-18T14:23:00-04:00"),
            title: None,
            labels: vec!["#release".to_string(), "##mage".to_string()],
            body: String::new(),
        };
        let labels = extract_all_labels(&note);
        assert_eq!(labels, vec!["release", "mage"]);
    }

    // ---- LabelIndex::touch ----

    #[test]
    fn touch_adds_new_label() {
        let mut idx = LabelIndex::default();
        idx.touch("release", ymd(2026, 6, 18));
        assert_eq!(idx.labels.len(), 1);
        let entry = &idx.labels[0];
        assert_eq!(entry.name, "release");
        assert_eq!(entry.count, 1);
        assert_eq!(entry.first_used, ymd(2026, 6, 18));
        assert_eq!(entry.last_used, ymd(2026, 6, 18));
    }

    #[test]
    fn touch_increments_existing_label() {
        let mut idx = LabelIndex::default();
        idx.touch("release", ymd(2026, 6, 18));
        idx.touch("release", ymd(2026, 6, 19));
        assert_eq!(idx.labels.len(), 1);
        let entry = &idx.labels[0];
        assert_eq!(entry.count, 2);
        assert_eq!(entry.first_used, ymd(2026, 6, 18));
        assert_eq!(entry.last_used, ymd(2026, 6, 19));
    }

    #[test]
    fn touch_extends_first_used_when_older_date_seen() {
        let mut idx = LabelIndex::default();
        idx.touch("release", ymd(2026, 6, 18));
        idx.touch("release", ymd(2026, 1, 5));
        let entry = &idx.labels[0];
        assert_eq!(entry.first_used, ymd(2026, 1, 5));
        assert_eq!(entry.last_used, ymd(2026, 6, 18));
    }

    #[test]
    fn sort_puts_recent_then_frequent_then_alphabetical_first() {
        let mut idx = LabelIndex::default();
        idx.touch("old-but-popular", ymd(2026, 1, 1));
        idx.touch("old-but-popular", ymd(2026, 1, 2));
        idx.touch("old-but-popular", ymd(2026, 1, 3));
        idx.touch("recent", ymd(2026, 6, 22));
        idx.touch("a-tiebreaker", ymd(2026, 6, 22));
        idx.touch("a-tiebreaker", ymd(2026, 6, 22));

        // a-tiebreaker should come first (most recent and highest count tie,
        // but it has higher count than recent + alphabetical tiebreaker).
        assert_eq!(idx.labels[0].name, "a-tiebreaker");
        assert_eq!(idx.labels[1].name, "recent");
        assert_eq!(idx.labels[2].name, "old-but-popular");
    }

    // ---- LabelIndex roundtrip + record_note_labels ----

    #[tokio::test]
    async fn load_missing_returns_default() {
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        let idx = LabelIndex::load(&backend).await.unwrap();
        assert_eq!(idx, LabelIndex::default());
    }

    #[tokio::test]
    async fn save_then_load_roundtrips() {
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());

        let mut idx = LabelIndex::default();
        idx.touch("release", ymd(2026, 6, 18));
        idx.touch("mage", ymd(2026, 6, 22));
        idx.save(&backend).await.unwrap();

        let read = LabelIndex::load(&backend).await.unwrap();
        assert_eq!(read, idx);
    }

    #[tokio::test]
    async fn legacy_snake_case_labels_json_still_loads() {
        // Simulate a labels.json written before the camelCase sweep — the
        // serde aliases on first_used/last_used should let it parse, and
        // the next save will normalize to camelCase.
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        let legacy_json = r#"{
          "version": 1,
          "labels": [
            {
              "name": "release",
              "count": 47,
              "first_used": "2026-01-12",
              "last_used": "2026-06-18"
            }
          ]
        }"#;
        backend.write_metadata("labels.json", legacy_json).await.unwrap();

        let idx = LabelIndex::load(&backend).await.unwrap();
        assert_eq!(idx.labels.len(), 1);
        assert_eq!(idx.labels[0].name, "release");
        assert_eq!(idx.labels[0].count, 47);
        assert_eq!(idx.labels[0].first_used, ymd(2026, 1, 12));
        assert_eq!(idx.labels[0].last_used, ymd(2026, 6, 18));
    }

    #[test]
    fn label_entry_serializes_camel_case() {
        let entry = LabelEntry {
            name: "release".to_string(),
            count: 1,
            first_used: ymd(2026, 6, 22),
            last_used: ymd(2026, 6, 22),
            color: None,
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"firstUsed\""), "expected camelCase firstUsed in {json}");
        assert!(json.contains("\"lastUsed\""), "expected camelCase lastUsed in {json}");
        assert!(!json.contains("first_used"));
        assert!(!json.contains("last_used"));
        // None color must be skipped on serialize so labels.json stays
        // tidy for the uncolorized majority.
        assert!(
            !json.contains("\"color\""),
            "None color should be skipped via skip_serializing_if, got {json}"
        );
    }

    #[tokio::test]
    async fn corrupt_json_falls_back_to_empty() {
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        backend
            .write_metadata("labels.json", "this is not json")
            .await
            .unwrap();
        let idx = LabelIndex::load(&backend).await.unwrap();
        assert_eq!(idx, LabelIndex::default());
    }

    #[tokio::test]
    async fn record_note_labels_creates_and_appends() {
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());

        let note1 = Note {
            timestamp: ts("2026-06-18T14:23:00-04:00"),
            title: None,
            labels: vec!["release".to_string()],
            body: "Shipped #mage updates.".to_string(),
        };
        record_note_labels(&backend, &note1, ymd(2026, 6, 18))
            .await
            .unwrap();

        let note2 = Note {
            timestamp: ts("2026-06-19T10:00:00-04:00"),
            title: None,
            labels: vec!["release".to_string(), "journal-app".to_string()],
            body: "Worked on the journal.".to_string(),
        };
        record_note_labels(&backend, &note2, ymd(2026, 6, 19))
            .await
            .unwrap();

        let idx = LabelIndex::load(&backend).await.unwrap();
        let names: Vec<&str> = idx.labels.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"release"));
        assert!(names.contains(&"mage"));
        assert!(names.contains(&"journal-app"));

        let release = idx.labels.iter().find(|e| e.name == "release").unwrap();
        assert_eq!(release.count, 2);
        assert_eq!(release.first_used, ymd(2026, 6, 18));
        assert_eq!(release.last_used, ymd(2026, 6, 19));
    }

    // ---- Phase 2.8 follow-on: per-label color override ----

    #[tokio::test]
    async fn legacy_labels_json_without_color_loads_with_none() {
        // Any labels.json written before the color field existed must
        // load cleanly with color = None on every entry. The
        // serde(default) attribute on the field is what makes this work;
        // pinning it here means a future refactor that drops the default
        // surfaces immediately instead of breaking onboarded users.
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        let legacy_json = r#"{
          "version": 1,
          "labels": [
            {
              "name": "release",
              "count": 3,
              "firstUsed": "2026-01-12",
              "lastUsed": "2026-06-18"
            },
            {
              "name": "mage",
              "count": 1,
              "firstUsed": "2026-06-22",
              "lastUsed": "2026-06-22"
            }
          ]
        }"#;
        backend.write_metadata("labels.json", legacy_json).await.unwrap();
        let idx = LabelIndex::load(&backend).await.unwrap();
        assert_eq!(idx.labels.len(), 2);
        assert!(idx.labels.iter().all(|e| e.color.is_none()));
    }

    #[tokio::test]
    async fn color_round_trips_through_save_and_load() {
        // Save an entry with a color override, reload, and confirm the
        // value comes back intact (and lowercase-normalized). Catches
        // serde rename mismatches as well as accidental drops of the
        // skip_serializing_if attribute (which would still round-trip
        // but inflate every uncolorized entry on the wire).
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());

        let mut idx = LabelIndex::default();
        idx.touch("release", ymd(2026, 6, 18));
        // Mutate the entry directly — the high-level API for setting a
        // color is `set_label_color` (commands.rs); this test exercises
        // the persistence layer in isolation.
        idx.labels
            .iter_mut()
            .find(|e| e.name == "release")
            .unwrap()
            .color = Some("#FF5C08".to_string()); // mixed case on purpose
        // Skip strict-validator path — we're writing the field directly
        // through the typed struct, where the deserializer doesn't run.
        // The normalization happens at deserialize time; serializing
        // emits the value verbatim, so write the lowercase form to keep
        // the on-disk shape stable.
        idx.labels
            .iter_mut()
            .find(|e| e.name == "release")
            .unwrap()
            .color = Some("#ff5c08".to_string());
        idx.save(&backend).await.unwrap();

        let read = LabelIndex::load(&backend).await.unwrap();
        let entry = read.labels.iter().find(|e| e.name == "release").unwrap();
        assert_eq!(entry.color.as_deref(), Some("#ff5c08"));
    }

    #[tokio::test]
    async fn color_field_omitted_when_none_on_disk() {
        // skip_serializing_if = "Option::is_none" keeps labels.json tidy
        // for the uncolorized majority. Pin the contract: a save of an
        // entry with color=None must not write a "color" key at all.
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());

        let mut idx = LabelIndex::default();
        idx.touch("release", ymd(2026, 6, 18));
        idx.save(&backend).await.unwrap();

        let raw = backend
            .read_metadata("labels.json")
            .await
            .unwrap()
            .expect("labels.json should exist");
        assert!(
            !raw.contains("\"color\""),
            "expected no color key for uncolorized entry, got {raw}"
        );
    }

    #[test]
    fn malformed_hex_color_is_rejected_on_deserialize() {
        // A bad hex value in a labels.json entry must surface as a serde
        // error that mentions the offending value. Same contract as
        // CustomTheme primaries — the shared deserializer is the whole
        // point of routing through settings::deserialize_hex6_option.
        let json = r#"{
          "version": 1,
          "labels": [
            {
              "name": "release",
              "count": 1,
              "firstUsed": "2026-06-22",
              "lastUsed": "2026-06-22",
              "color": "not-a-color"
            }
          ]
        }"#;
        let err = serde_json::from_str::<LabelIndex>(json).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("hex color") && msg.contains("not-a-color"),
            "expected the error to name the bad value, got: {msg}"
        );
    }

    #[test]
    fn three_digit_hex_shorthand_rejected_for_color() {
        // CSS allows `#f00`; we don't. The derivation engine and chip
        // renderer both work off the canonical 6-digit form, so reject
        // shorthand at the serde boundary the same way we do for
        // CustomTheme primaries.
        let json = r##"{
          "version": 1,
          "labels": [
            {
              "name": "release",
              "count": 1,
              "firstUsed": "2026-06-22",
              "lastUsed": "2026-06-22",
              "color": "#f00"
            }
          ]
        }"##;
        assert!(serde_json::from_str::<LabelIndex>(json).is_err());
    }

    #[test]
    fn color_uppercase_normalized_to_lowercase_on_deserialize() {
        // The hex deserializer lowercases on the way in so on-disk
        // round-trips are stable regardless of which case the frontend
        // sent. Pin the contract for the labels.json variant.
        let json = r##"{
          "version": 1,
          "labels": [
            {
              "name": "release",
              "count": 1,
              "firstUsed": "2026-06-22",
              "lastUsed": "2026-06-22",
              "color": "#FF5C08"
            }
          ]
        }"##;
        let idx: LabelIndex = serde_json::from_str(json).unwrap();
        assert_eq!(idx.labels[0].color.as_deref(), Some("#ff5c08"));
    }

    #[tokio::test]
    async fn record_note_labels_preserves_existing_color() {
        // The `touch` path increments counts and updates date ranges on
        // an existing entry. It must NOT clobber a previously-set color
        // override — that would silently undo the user's choice every
        // time they used the label again.
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());

        let mut idx = LabelIndex::default();
        idx.touch("release", ymd(2026, 6, 18));
        idx.labels
            .iter_mut()
            .find(|e| e.name == "release")
            .unwrap()
            .color = Some("#ff5c08".to_string());
        idx.save(&backend).await.unwrap();

        let note = Note {
            timestamp: ts("2026-06-19T10:00:00-04:00"),
            title: None,
            labels: vec!["release".to_string()],
            body: String::new(),
        };
        record_note_labels(&backend, &note, ymd(2026, 6, 19))
            .await
            .unwrap();

        let reloaded = LabelIndex::load(&backend).await.unwrap();
        let entry = reloaded.labels.iter().find(|e| e.name == "release").unwrap();
        assert_eq!(entry.count, 2);
        assert_eq!(entry.color.as_deref(), Some("#ff5c08"));
    }

    // ---- scan_label_sites ----

    /// Convenience: pull the substring of the file that one site covers.
    fn slice_site<'a>(file: &'a str, site: &LabelSite) -> &'a str {
        &file[site.byte_range.clone()]
    }

    #[test]
    fn scan_note_with_labels_line() {
        // Standard `Note::to_markdown` shape: heading, labels line, body.
        let file = "\n### 2026-06-18 14:23 — Working\n\
                    **Labels:** #journal-app #project\n\
                    \n\
                    Started planning.\n";
        let sites = scan_label_sites(file);
        assert_eq!(sites.len(), 1);
        let s = &sites[0];
        assert_eq!(s.kind, LabelSiteKind::NoteLabelsLine);
        assert_eq!(s.names, vec!["journal-app", "project"]);
        // Range must cover the whole line including trailing newline so
        // that splicing in an empty replacement removes the line cleanly.
        assert_eq!(slice_site(file, s), "**Labels:** #journal-app #project\n");
    }

    #[test]
    fn scan_note_without_labels_line() {
        // Heading + body but no `**Labels:**` line — no sites.
        let file = "\n### 2026-06-18 14:23 — No labels\n\
                    \n\
                    Just a body.\n";
        let sites = scan_label_sites(file);
        assert!(sites.is_empty());
    }

    #[test]
    fn scan_summary_with_labels_subsection() {
        let file = "## Weekly Summary\n\
                    *Last updated: never*\n\
                    \n### Key accomplishments\n\
                    \n### Plans and priorities for next week\n\
                    \n### Challenges or roadblocks\n\
                    \n### Anything else on your mind\n\
                    \n### Labels\n\
                    #release #planning #captains-log\n\
                    \n## Weekly Notes\n";
        let sites = scan_label_sites(file);
        assert_eq!(sites.len(), 1);
        let s = &sites[0];
        assert_eq!(s.kind, LabelSiteKind::SummaryLabelsSubsection);
        assert_eq!(s.names, vec!["release", "planning", "captains-log"]);
        // The range must start at "### Labels" and run up to the next "## "
        // heading (exclusive) so rename + delete can splice a freshly-
        // rendered subsection in atomically.
        let slice = slice_site(file, s);
        assert!(slice.starts_with("### Labels\n"));
        assert!(slice.contains("#release #planning #captains-log"));
        assert!(!slice.contains("## Weekly Notes"));
    }

    #[test]
    fn scan_summary_without_subsection() {
        // No `### Labels` heading at all.
        let file = "## Weekly Summary\n\
                    *Last updated: never*\n\
                    \n### Key accomplishments\n\
                    \n## Weekly Notes\n";
        let sites = scan_label_sites(file);
        assert!(sites.is_empty());
    }

    #[test]
    fn scan_summary_with_empty_labels_subsection() {
        // Heading present, body blank.
        let file = "## Weekly Summary\n\
                    *Last updated: never*\n\
                    \n### Labels\n\
                    \n## Weekly Notes\n";
        let sites = scan_label_sites(file);
        assert_eq!(sites.len(), 1);
        let s = &sites[0];
        assert_eq!(s.kind, LabelSiteKind::SummaryLabelsSubsection);
        assert!(s.names.is_empty());
        // Range still spans the heading + the blank body so a rename pass
        // that wants to add a tag can do so by splicing.
        let slice = slice_site(file, s);
        assert!(slice.starts_with("### Labels\n"));
    }

    #[test]
    fn scan_both_note_and_summary_on_same_file() {
        // Whole-file fixture: summary subsection + a Note with a labels
        // line. The two sites should come back in document order.
        let file = "---\n\
                    period: 2026-W25\n\
                    ---\n\
                    \n# Week of June 15 - June 21, 2026\n\
                    \n## Weekly Summary\n\
                    *Last updated: never*\n\
                    \n### Key accomplishments\n\
                    \n### Plans and priorities for next week\n\
                    \n### Challenges or roadblocks\n\
                    \n### Anything else on your mind\n\
                    \n### Labels\n\
                    #release #planning\n\
                    \n## Weekly Notes\n\
                    \n### 2026-06-18 14:23 — Working\n\
                    **Labels:** #journal-app #project\n\
                    \nStarted planning.\n";
        let sites = scan_label_sites(file);
        assert_eq!(sites.len(), 2);
        // Document order: summary subsection appears before the note line.
        assert_eq!(sites[0].kind, LabelSiteKind::SummaryLabelsSubsection);
        assert_eq!(sites[0].names, vec!["release", "planning"]);
        assert_eq!(sites[1].kind, LabelSiteKind::NoteLabelsLine);
        assert_eq!(sites[1].names, vec!["journal-app", "project"]);
        // Sanity: the two byte ranges don't overlap.
        assert!(sites[0].byte_range.end <= sites[1].byte_range.start);
        // Slices are what we'd expect to replace.
        let n_slice = slice_site(file, &sites[1]);
        assert_eq!(n_slice, "**Labels:** #journal-app #project\n");
    }

    #[test]
    fn scan_file_with_only_inline_hashtags() {
        // Inline `#hashtag` tokens inside prose are explicitly NOT sites —
        // Phase 3a delete cascade leaves them in place.
        let file = "\n### 2026-06-18 14:23 — Inline only\n\
                    \nThis body mentions #release and #mage inline.\n\
                    \nNo labels line.\n";
        let sites = scan_label_sites(file);
        assert!(sites.is_empty());
    }

    #[test]
    fn scan_empty_file() {
        let sites = scan_label_sites("");
        assert!(sites.is_empty());
    }

    #[test]
    fn scan_round_trips_with_notes_module() {
        // Build a real file via the existing scaffold + render path, then
        // confirm scan_label_sites finds the expected sites and the byte
        // ranges yield the exact slices a rename/delete pass would mutate.
        let now = ts("2026-06-22T09:00:00-04:00");
        let mut file = crate::notes::weekly_file_scaffold(2026, 26, now);
        // Swap in a real summary with labels.
        let summary = crate::notes::WeeklySummary {
            key_accomplishments: "shipped".to_string(),
            labels: vec!["release".to_string(), "captains-log".to_string()],
            last_updated: Some("2026-06-22 17:00".to_string()),
            ..Default::default()
        };
        file = crate::notes::replace_weekly_summary_in_file(&file, &summary);
        // Append a Note via to_markdown directly so we don't drag the
        // filesystem backend into this test.
        let note = Note {
            timestamp: ts("2026-06-22T10:00:00-04:00"),
            title: Some("First".to_string()),
            labels: vec!["release".to_string(), "ops".to_string()],
            body: "body".to_string(),
        };
        file.push_str(&note.to_markdown());

        let sites = scan_label_sites(&file);
        assert_eq!(sites.len(), 2, "expected one summary subsection + one note line, got {sites:?}");
        assert_eq!(sites[0].kind, LabelSiteKind::SummaryLabelsSubsection);
        assert_eq!(sites[0].names, vec!["release", "captains-log"]);
        assert_eq!(sites[1].kind, LabelSiteKind::NoteLabelsLine);
        assert_eq!(sites[1].names, vec!["release", "ops"]);

        // The Note labels line slice must end with '\n' so splicing in ""
        // removes the line wholesale.
        assert!(slice_site(&file, &sites[1]).ends_with('\n'));
    }

    #[test]
    fn scan_does_not_match_subsection_with_extra_text_on_heading() {
        // "### Labels of Doom" is not the labels subsection.
        let file = "## Weekly Summary\n\
                    *Last updated: never*\n\
                    \n### Labels of Doom\n\
                    #not-a-real-site\n\
                    \n## Weekly Notes\n";
        let sites = scan_label_sites(file);
        assert!(sites.is_empty());
    }

    #[tokio::test]
    async fn record_note_labels_with_empty_label_set_is_noop() {
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        let note = Note {
            timestamp: ts("2026-06-18T14:23:00-04:00"),
            title: None,
            labels: vec![],
            body: "Body with no hashtags.".to_string(),
        };
        record_note_labels(&backend, &note, ymd(2026, 6, 18))
            .await
            .unwrap();
        // No file written.
        let content = backend.read_metadata("labels.json").await.unwrap();
        assert_eq!(content, None);
    }
}
