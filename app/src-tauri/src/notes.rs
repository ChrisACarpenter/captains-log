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
         ### Labels\n\
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

// ---------------------------------------------------------------------------
// Weekly Summary
// ---------------------------------------------------------------------------

/// The four-field Lattice-style summary that lives at the top of every weekly
/// file. Each field is free markdown.
///
/// `last_updated` is a human-readable string (`YYYY-MM-DD HH:MM` in the user's
/// local time when last saved) or `None` for never-saved.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeeklySummary {
    pub key_accomplishments: String,
    pub plans_and_priorities: String,
    pub challenges_or_roadblocks: String,
    pub anything_else: String,
    /// Labels for the week as a whole — sits as a `### Labels` subsection
    /// at the end of the Weekly Summary section, rendered as `#tag1 #tag2`.
    #[serde(default)]
    pub labels: Vec<String>,
    pub last_updated: Option<String>,
}

const SUMMARY_HEADER: &str = "## Weekly Summary";
const NOTES_HEADER: &str = "## Weekly Notes";
const SECTION_KEY_ACC: &str = "### Key accomplishments";
const SECTION_PLANS: &str = "### Plans and priorities for next week";
const SECTION_CHALLENGES: &str = "### Challenges or roadblocks";
const SECTION_OTHER: &str = "### Anything else on your mind";
const SECTION_LABELS: &str = "### Labels";
const LAST_UPDATED_PREFIX: &str = "*Last updated: ";

/// Parse the Weekly Summary section out of a weekly file's full markdown.
/// Missing/unparseable sections yield empty strings. Never panics.
pub fn parse_weekly_summary(file_content: &str) -> WeeklySummary {
    let mut summary = WeeklySummary::default();

    let Some(summary_start) = file_content.find(SUMMARY_HEADER) else {
        return summary;
    };
    let summary_end = file_content[summary_start..]
        .find(NOTES_HEADER)
        .map(|i| summary_start + i)
        .unwrap_or(file_content.len());

    let section = &file_content[summary_start..summary_end];

    // last_updated line: "*Last updated: VALUE*"
    if let Some(line_start) = section.find(LAST_UPDATED_PREFIX) {
        let after_prefix = line_start + LAST_UPDATED_PREFIX.len();
        if let Some(end_offset) = section[after_prefix..].find('*') {
            let value = &section[after_prefix..after_prefix + end_offset];
            let trimmed = value.trim();
            if !trimmed.is_empty() && trimmed != "never" {
                summary.last_updated = Some(trimmed.to_string());
            }
        }
    }

    summary.key_accomplishments = extract_subsection(section, SECTION_KEY_ACC);
    summary.plans_and_priorities = extract_subsection(section, SECTION_PLANS);
    summary.challenges_or_roadblocks = extract_subsection(section, SECTION_CHALLENGES);
    summary.anything_else = extract_subsection(section, SECTION_OTHER);

    // Labels live as a free-form `### Labels` subsection. Body is one or more
    // `#tag` tokens (anything starting with #); whitespace between them is fine.
    let labels_text = extract_subsection(section, SECTION_LABELS);
    summary.labels = labels_text
        .split_whitespace()
        .filter_map(|tok| {
            let stripped = tok.trim_start_matches('#').trim();
            if stripped.is_empty() {
                None
            } else {
                Some(stripped.to_string())
            }
        })
        .collect();

    summary
}

/// Pull the body of a `### Subheading` block out of the Weekly Summary section.
/// The body runs from after the heading line to the next `### ` or end of section.
fn extract_subsection(section: &str, header: &str) -> String {
    let Some(start) = section.find(header) else {
        return String::new();
    };
    // Skip past the header line itself.
    let body_start = match section[start..].find('\n') {
        Some(n) => start + n + 1,
        None => return String::new(),
    };
    // Find the next "### " heading after body_start (must be at start of a line).
    let mut body_end = section
        .get(body_start..)
        .and_then(|s| s.find("\n### "))
        .map(|i| body_start + i)
        .unwrap_or(section.len());
    // Defensive: also stop at any "## " heading (covers ## Weekly Notes if the
    // section boundary check missed it).
    if let Some(idx) = section[body_start..].find("\n## ") {
        let candidate = body_start + idx;
        if candidate < body_end {
            body_end = candidate;
        }
    }
    section[body_start..body_end].trim().to_string()
}

/// Render a Weekly Summary section back to markdown, preserving the structure
/// the scaffold uses (so the file stays diff-clean).
pub fn render_weekly_summary(summary: &WeeklySummary) -> String {
    let last_updated = summary.last_updated.as_deref().unwrap_or("never");
    let labels_line = if summary.labels.is_empty() {
        String::new()
    } else {
        summary
            .labels
            .iter()
            .map(|l| format!("#{}", l.trim_start_matches('#')))
            .collect::<Vec<_>>()
            .join(" ")
    };
    format!(
        "## Weekly Summary\n\
         *Last updated: {last_updated}*\n\
         \n\
         ### Key accomplishments\n\
         {key}\n\
         \n\
         ### Plans and priorities for next week\n\
         {plans}\n\
         \n\
         ### Challenges or roadblocks\n\
         {challenges}\n\
         \n\
         ### Anything else on your mind\n\
         {other}\n\
         \n\
         ### Labels\n\
         {labels}\n",
        last_updated = last_updated,
        key = trim_body(&summary.key_accomplishments),
        plans = trim_body(&summary.plans_and_priorities),
        challenges = trim_body(&summary.challenges_or_roadblocks),
        other = trim_body(&summary.anything_else),
        labels = labels_line,
    )
}

fn trim_body(s: &str) -> &str {
    s.trim_end()
}

/// Splice a new Weekly Summary into an existing weekly file's full content,
/// preserving everything outside the summary section (frontmatter, the week
/// heading, Weekly Notes, etc.).
pub fn replace_weekly_summary_in_file(file_content: &str, new_summary: &WeeklySummary) -> String {
    let Some(summary_start) = file_content.find(SUMMARY_HEADER) else {
        // No Weekly Summary section yet — append before Weekly Notes if present,
        // otherwise at the end.
        if let Some(notes_start) = file_content.find(NOTES_HEADER) {
            let before = &file_content[..notes_start];
            let after = &file_content[notes_start..];
            return format!("{}{}\n{}", before, render_weekly_summary(new_summary), after);
        }
        let mut out = file_content.to_string();
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&render_weekly_summary(new_summary));
        return out;
    };

    let summary_end = file_content[summary_start..]
        .find(NOTES_HEADER)
        .map(|i| summary_start + i)
        .unwrap_or(file_content.len());

    let before = &file_content[..summary_start];
    let after = &file_content[summary_end..];

    let mut new_section = render_weekly_summary(new_summary);
    // Ensure exactly one blank line between the new Summary section and what
    // follows (Weekly Notes header, or EOF).
    if !new_section.ends_with('\n') {
        new_section.push('\n');
    }

    format!("{before}{new_section}\n{after}")
}

// ---------------------------------------------------------------------------
// append_note
// ---------------------------------------------------------------------------

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
        assert!(scaffold.contains("### Labels"));
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

    // ---- WeeklySummary parsing / serialization ----

    fn scaffold_with_summary(extra_body: &str) -> String {
        // A weekly file with a scaffold + content stuffed into the first
        // summary subsection. Used to verify parsing without coupling tests
        // to the exact byte layout.
        let now = ts("2026-06-22T09:00:00-04:00");
        let scaffold = weekly_file_scaffold(2026, 26, now);
        // Insert content under "### Key accomplishments\n"
        scaffold.replacen(
            "### Key accomplishments\n",
            &format!("### Key accomplishments\n{extra_body}\n"),
            1,
        )
    }

    #[test]
    fn parse_empty_summary_returns_empty_fields() {
        let now = ts("2026-06-22T09:00:00-04:00");
        let file = weekly_file_scaffold(2026, 26, now);
        let s = parse_weekly_summary(&file);
        assert_eq!(s.key_accomplishments, "");
        assert_eq!(s.plans_and_priorities, "");
        assert_eq!(s.challenges_or_roadblocks, "");
        assert_eq!(s.anything_else, "");
        assert_eq!(s.last_updated, None);
    }

    #[test]
    fn parse_summary_extracts_each_subsection() {
        let file = "## Weekly Summary\n*Last updated: 2026-06-22 17:00*\n\
                    \n### Key accomplishments\n- shipped foo\n- fixed bar\n\
                    \n### Plans and priorities for next week\n- ship baz\n\
                    \n### Challenges or roadblocks\nnone today\n\
                    \n### Anything else on your mind\nfeeling good\n\
                    \n## Weekly Notes\n\
                    \n### 2026-06-22 09:00 — Hi\nhi body\n";

        let s = parse_weekly_summary(file);
        assert_eq!(s.last_updated.as_deref(), Some("2026-06-22 17:00"));
        assert_eq!(s.key_accomplishments, "- shipped foo\n- fixed bar");
        assert_eq!(s.plans_and_priorities, "- ship baz");
        assert_eq!(s.challenges_or_roadblocks, "none today");
        assert_eq!(s.anything_else, "feeling good");
    }

    #[test]
    fn parse_summary_ignores_never_marker_as_last_updated() {
        let now = ts("2026-06-22T09:00:00-04:00");
        let file = weekly_file_scaffold(2026, 26, now);
        let s = parse_weekly_summary(&file);
        assert_eq!(s.last_updated, None);
    }

    #[test]
    fn parse_summary_handles_content_in_one_field_only() {
        let file = scaffold_with_summary("Did the thing.");
        let s = parse_weekly_summary(&file);
        assert_eq!(s.key_accomplishments, "Did the thing.");
        assert_eq!(s.plans_and_priorities, "");
    }

    #[test]
    fn render_summary_roundtrips_through_parse() {
        let original = WeeklySummary {
            key_accomplishments: "- one\n- two".to_string(),
            plans_and_priorities: "- three".to_string(),
            challenges_or_roadblocks: "- four".to_string(),
            anything_else: "five".to_string(),
            labels: vec!["release".to_string(), "captains-log".to_string()],
            last_updated: Some("2026-06-22 17:00".to_string()),
        };
        let rendered = render_weekly_summary(&original);
        let parsed = parse_weekly_summary(&rendered);
        assert_eq!(parsed.key_accomplishments, original.key_accomplishments);
        assert_eq!(parsed.plans_and_priorities, original.plans_and_priorities);
        assert_eq!(parsed.challenges_or_roadblocks, original.challenges_or_roadblocks);
        assert_eq!(parsed.anything_else, original.anything_else);
        assert_eq!(parsed.labels, original.labels);
        assert_eq!(parsed.last_updated, original.last_updated);
    }

    #[test]
    fn parse_summary_extracts_labels() {
        let file = "## Weekly Summary\n*Last updated: never*\n\
                    \n### Key accomplishments\n\
                    \n### Plans and priorities for next week\n\
                    \n### Challenges or roadblocks\n\
                    \n### Anything else on your mind\n\
                    \n### Labels\n#release #planning #captains-log\n\
                    \n## Weekly Notes\n";
        let s = parse_weekly_summary(file);
        assert_eq!(s.labels, vec!["release", "planning", "captains-log"]);
    }

    #[test]
    fn parse_summary_empty_labels_subsection() {
        // Scaffolded file with no labels typed yet — should yield empty Vec.
        let now = ts("2026-06-22T09:00:00-04:00");
        let file = weekly_file_scaffold(2026, 26, now);
        let s = parse_weekly_summary(&file);
        assert!(s.labels.is_empty());
    }

    #[test]
    fn render_summary_strips_extra_hash_prefixes() {
        // If callers send labels with leading #'s (e.g. from a chip input that
        // didn't strip them), render them with exactly one # each.
        let original = WeeklySummary {
            labels: vec!["##leading-hashes".to_string(), "plain".to_string()],
            ..Default::default()
        };
        let rendered = render_weekly_summary(&original);
        assert!(rendered.contains("#leading-hashes #plain"));
        assert!(!rendered.contains("##leading-hashes"));
    }

    #[test]
    fn replace_summary_preserves_notes_below() {
        let original = "## Weekly Summary\n\
                        *Last updated: never*\n\
                        \n### Key accomplishments\n\
                        \n### Plans and priorities for next week\n\
                        \n### Challenges or roadblocks\n\
                        \n### Anything else on your mind\n\
                        \n## Weekly Notes\n\
                        \n### 2026-06-22 09:00 — Hi\n\
                        **Labels:** #release\n\
                        \nFirst note body.\n";

        let new_summary = WeeklySummary {
            key_accomplishments: "- shipped Captain's Log".to_string(),
            plans_and_priorities: "- testing!".to_string(),
            challenges_or_roadblocks: String::new(),
            anything_else: String::new(),
            labels: vec![],
            last_updated: Some("2026-06-22 17:30".to_string()),
        };

        let updated = replace_weekly_summary_in_file(original, &new_summary);

        // New content in
        assert!(updated.contains("- shipped Captain's Log"));
        assert!(updated.contains("*Last updated: 2026-06-22 17:30*"));
        // Existing note preserved
        assert!(updated.contains("### 2026-06-22 09:00 — Hi"));
        assert!(updated.contains("**Labels:** #release"));
        assert!(updated.contains("First note body."));
        // Single Weekly Notes header still present
        assert_eq!(updated.matches("## Weekly Notes").count(), 1);
        // Single Weekly Summary header still present
        assert_eq!(updated.matches("## Weekly Summary").count(), 1);
    }

    #[test]
    fn replace_summary_works_when_file_has_no_summary_section() {
        // Hypothetical legacy/malformed file that's missing the summary.
        let original = "# Week of June 22 - June 28, 2026\n\
                        \n## Weekly Notes\n\
                        \n### 2026-06-22 09:00 — Hi\n\
                        body\n";
        let new_summary = WeeklySummary {
            key_accomplishments: "recovered".to_string(),
            ..Default::default()
        };
        let updated = replace_weekly_summary_in_file(original, &new_summary);
        assert!(updated.contains("## Weekly Summary"));
        assert!(updated.contains("recovered"));
        assert!(updated.contains("## Weekly Notes"));
        assert!(updated.contains("### 2026-06-22 09:00 — Hi"));
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
