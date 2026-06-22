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

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::notes::Note;
use crate::storage::{StorageBackend, StorageError, StorageResult};

const METADATA_FILE: &str = "labels.json";
const CURRENT_VERSION: u32 = 1;

/// One label's aggregate stats.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LabelEntry {
    pub name: String,
    pub count: u32,
    pub first_used: NaiveDate,
    pub last_used: NaiveDate,
}

/// The on-disk label index.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
