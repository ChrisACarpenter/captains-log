//! Notes API.
//!
//! A "Note" is a single timestamped entry in a weekly file. This module:
//!   - Defines [`Note`] and its markdown serialization
//!   - Computes the ISO 8601 week + year for a given date (so quick-capture
//!     knows which weekly file to write to)
//!   - Builds the scaffold for a brand-new weekly file
//!   - Provides [`append_note`], the high-level "create or update weekly file
//!     and add a Note" operation
//!
//! Parsing existing weekly files back into structured data (reading past notes,
//! Weekly Summary handling, full frontmatter awareness) lives in a future
//! module — Phase 1 only needs to *write*. See `docs/data-format.md`.

use chrono::{Datelike, FixedOffset, NaiveDate, Weekday};
use serde::{Deserialize, Serialize};

use crate::storage::{StorageBackend, StorageResult};

/// A single timestamped Note.
///
/// Times use a fixed offset (captured at write time) rather than a chrono
/// `DateTime<Local>` so that re-reading old notes doesn't reinterpret the
/// timestamp under whatever the user's current timezone happens to be.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub timestamp: chrono::DateTime<FixedOffset>,
    pub title: Option<String>,
    pub labels: Vec<String>,
    pub body: String,
}

impl Note {
    /// Render the Note as the markdown that gets appended into a weekly file.
    ///
    /// Format follows `docs/data-format.md`:
    ///
    /// ```text
    /// ### YYYY-MM-DD HH:MM — Title
    /// **Labels:** #label1 #label2
    ///
    /// Body...
    /// ```
    ///
    /// The leading blank line ensures the heading separates from any preceding
    /// content. The trailing blank line keeps consecutive notes legible.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();

        // Blank line, then heading.
        out.push('\n');
        out.push_str("### ");
        out.push_str(&self.timestamp.format("%Y-%m-%d %H:%M").to_string());
        if let Some(title) = self.title.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
            out.push_str(" — ");
            out.push_str(title);
        }
        out.push('\n');

        // Labels line, only when present.
        if !self.labels.is_empty() {
            out.push_str("**Labels:**");
            for label in &self.labels {
                out.push_str(" #");
                out.push_str(label);
            }
            out.push('\n');
        }

        // Body (trimmed of trailing whitespace, with a trailing newline).
        out.push('\n');
        out.push_str(self.body.trim_end());
        out.push('\n');

        out
    }
}

/// ISO 8601 (year, week) for a given naive date.
///
/// Note that the ISO year may differ from the calendar year near year
/// boundaries. For example, 2025-12-29 is part of ISO week 1 of 2026.
pub fn iso_year_week(date: NaiveDate) -> (u32, u32) {
    let iso = date.iso_week();
    (iso.year() as u32, iso.week())
}

/// Construct the Monday (start) of a given ISO year+week.
///
/// Panics on invalid input. Validate at the boundary before calling.
pub fn iso_week_start(year: u32, week: u32) -> NaiveDate {
    NaiveDate::from_isoywd_opt(year as i32, week, Weekday::Mon)
        .expect("valid ISO year-week")
}

/// Render the scaffold for a brand-new weekly file: frontmatter, empty
/// Weekly Summary, and an empty Weekly Notes section that subsequent Notes
/// get appended into.
pub fn weekly_file_scaffold(year: u32, week: u32, now: chrono::DateTime<FixedOffset>) -> String {
    let start = iso_week_start(year, week);
    let end = start + chrono::Duration::days(6);

    let human_range = format_week_range(start, end, year);

    format!(
        "---\n\
         period: {year:04}-W{week:02}\n\
         start: {start_iso}\n\
         end: {end_iso}\n\
         labels: []\n\
         last_modified: {modified}\n\
         ---\n\
         \n\
         # Week of {human_range}\n\
         \n\
         ## Weekly Summary\n\
         *Last updated: never*\n\
         \n\
         ### Key accomplishments\n\
         \n\
         ### Plans and priorities for next week\n\
         \n\
         ### Challenges or roadblocks\n\
         \n\
         ### Anything else on your mind\n\
         \n\
         ## Weekly Notes\n",
        year = year,
        week = week,
        start_iso = start.format("%Y-%m-%d"),
        end_iso = end.format("%Y-%m-%d"),
        modified = now.format("%Y-%m-%dT%H:%M:%S%:z"),
        human_range = human_range,
    )
}

/// Format the human-readable week range header.
///
/// Same-calendar-year weeks render as `"June 15 - June 21, 2026"`.
/// Cross-year weeks (ISO week 1 spanning two calendar years) render as
/// `"December 29, 2025 - January 4, 2026"`.
fn format_week_range(start: NaiveDate, end: NaiveDate, period_year: u32) -> String {
    if start.year() == end.year() {
        format!(
            "{} - {}, {}",
            start.format("%B %-d"),
            end.format("%B %-d"),
            period_year
        )
    } else {
        format!(
            "{}, {} - {}, {}",
            start.format("%B %-d"),
            start.year(),
            end.format("%B %-d"),
            end.year()
        )
    }
}

/// Append a note into the weekly file for `(year, week)`, creating the file
/// with a fresh scaffold if it does not yet exist.
///
/// Frontmatter's `last_modified` is **not** updated by this function in Phase
/// 1 — it's set once at file creation. Proper frontmatter rewriting comes when
/// we add full parsing (Phase 2).
pub async fn append_note<B: StorageBackend>(
    backend: &B,
    year: u32,
    week: u32,
    note: &Note,
) -> StorageResult<()> {
    let existing = backend.read_week(year, week).await?;

    let mut content = match existing {
        Some(c) => c,
        None => weekly_file_scaffold(year, week, note.timestamp),
    };

    if !content.ends_with('\n') {
        content.push('\n');
    }
    content.push_str(&note.to_markdown());

    backend.write_week(year, week, &content).await
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{LocalFilesystem, StorageBackend};
    use chrono::{NaiveDate, TimeZone};
    use tempfile::TempDir;

    fn ts(s: &str) -> chrono::DateTime<FixedOffset> {
        chrono::DateTime::parse_from_rfc3339(s).expect("parse timestamp")
    }

    fn sample_note() -> Note {
        Note {
            timestamp: ts("2026-06-18T14:23:00-04:00"),
            title: Some("Working on the journal app".to_string()),
            labels: vec!["journal-app".to_string(), "project".to_string()],
            body: "Started planning the new journal tool.".to_string(),
        }
    }

    // ---- Note::to_markdown ----

    #[test]
    fn note_to_markdown_full() {
        let md = sample_note().to_markdown();
        assert!(md.contains("### 2026-06-18 14:23 — Working on the journal app"));
        assert!(md.contains("**Labels:** #journal-app #project"));
        assert!(md.contains("Started planning the new journal tool."));
    }

    #[test]
    fn note_to_markdown_without_title_drops_separator() {
        let mut n = sample_note();
        n.title = None;
        let md = n.to_markdown();
        assert!(md.contains("### 2026-06-18 14:23\n"));
        assert!(!md.contains(" — "));
    }

    #[test]
    fn note_to_markdown_empty_title_drops_separator() {
        let mut n = sample_note();
        n.title = Some("   ".to_string());
        let md = n.to_markdown();
        assert!(!md.contains(" — "));
    }

    #[test]
    fn note_to_markdown_without_labels_drops_labels_line() {
        let mut n = sample_note();
        n.labels = vec![];
        let md = n.to_markdown();
        assert!(!md.contains("**Labels:**"));
    }

    // ---- ISO week math ----

    #[test]
    fn iso_year_week_inside_year() {
        let d = NaiveDate::from_ymd_opt(2026, 6, 18).unwrap();
        assert_eq!(iso_year_week(d), (2026, 25));
    }

    #[test]
    fn iso_year_week_cross_year_boundary() {
        // 2025-12-29 (Monday) is ISO week 1 of 2026.
        let d = NaiveDate::from_ymd_opt(2025, 12, 29).unwrap();
        assert_eq!(iso_year_week(d), (2026, 1));
    }

    #[test]
    fn iso_week_start_returns_monday() {
        let monday = iso_week_start(2026, 25);
        assert_eq!(monday.weekday(), Weekday::Mon);
        assert_eq!(monday, NaiveDate::from_ymd_opt(2026, 6, 15).unwrap());
    }

    // ---- Scaffold ----

    #[test]
    fn scaffold_contains_required_sections() {
        let scaffold = weekly_file_scaffold(2026, 25, ts("2026-06-15T09:00:00-04:00"));
        assert!(scaffold.starts_with("---\n"));
        assert!(scaffold.contains("period: 2026-W25"));
        assert!(scaffold.contains("start: 2026-06-15"));
        assert!(scaffold.contains("end: 2026-06-21"));
        assert!(scaffold.contains("# Week of June 15 - June 21, 2026"));
        assert!(scaffold.contains("## Weekly Summary"));
        assert!(scaffold.contains("### Key accomplishments"));
        assert!(scaffold.contains("### Plans and priorities for next week"));
        assert!(scaffold.contains("### Challenges or roadblocks"));
        assert!(scaffold.contains("### Anything else on your mind"));
        assert!(scaffold.contains("## Weekly Notes"));
    }

    #[test]
    fn scaffold_handles_cross_year_week() {
        // ISO week 1 of 2026 starts on Mon 2025-12-29.
        let scaffold = weekly_file_scaffold(2026, 1, ts("2025-12-29T09:00:00-04:00"));
        assert!(scaffold.contains("start: 2025-12-29"));
        assert!(scaffold.contains("end: 2026-01-04"));
        // Range header should show both years.
        assert!(scaffold.contains("December 29, 2025 - January 4, 2026"));
    }

    // ---- append_note end-to-end ----

    #[tokio::test]
    async fn append_note_creates_file_with_scaffold_and_note() {
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        let note = sample_note();

        append_note(&backend, 2026, 25, &note).await.unwrap();

        let written = backend.read_week(2026, 25).await.unwrap().unwrap();
        assert!(written.starts_with("---\n"));
        assert!(written.contains("## Weekly Notes"));
        assert!(written.contains("### 2026-06-18 14:23 — Working on the journal app"));
        assert!(written.contains("**Labels:** #journal-app #project"));
        assert!(written.contains("Started planning the new journal tool."));
    }

    #[tokio::test]
    async fn append_note_to_existing_file_keeps_prior_content() {
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());

        // First note creates the file.
        let n1 = Note {
            timestamp: ts("2026-06-18T10:00:00-04:00"),
            title: Some("First".to_string()),
            labels: vec!["first".to_string()],
            body: "First body.".to_string(),
        };
        append_note(&backend, 2026, 25, &n1).await.unwrap();

        // Second note appends to it.
        let n2 = Note {
            timestamp: ts("2026-06-18T14:23:00-04:00"),
            title: Some("Second".to_string()),
            labels: vec!["second".to_string()],
            body: "Second body.".to_string(),
        };
        append_note(&backend, 2026, 25, &n2).await.unwrap();

        let written = backend.read_week(2026, 25).await.unwrap().unwrap();
        // Both notes present.
        assert!(written.contains("### 2026-06-18 10:00 — First"));
        assert!(written.contains("### 2026-06-18 14:23 — Second"));
        // Scaffold and frontmatter still present.
        assert!(written.contains("## Weekly Summary"));
        assert!(written.contains("period: 2026-W25"));
        // First note appears before second.
        let first_idx = written.find("First body").unwrap();
        let second_idx = written.find("Second body").unwrap();
        assert!(first_idx < second_idx);
    }

    #[tokio::test]
    async fn append_note_to_specific_year_week_writes_correct_file() {
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        let note = sample_note();

        append_note(&backend, 2026, 25, &note).await.unwrap();

        let expected = dir.path().join("2026").join("2026-W25.md");
        assert!(expected.exists());
    }

    // ---- Local timezone integration check ----

    #[test]
    fn local_now_can_be_converted_to_fixed_offset() {
        // Sanity-check that `chrono::Local::now().fixed_offset()` works — that's
        // the canonical way Tauri commands will obtain the timestamp for new notes.
        let now = chrono::Local::now().fixed_offset();
        // Roundtrip through RFC 3339 to ensure it serializes.
        let _serialized = now.to_rfc3339();
    }

    // suppress unused-import warning on chrono::TimeZone (kept for future use)
    #[allow(dead_code)]
    fn _unused() {
        let _ = chrono::Utc.timestamp_opt(0, 0);
    }
}
