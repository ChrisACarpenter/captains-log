//! Phase 5 — Performance Review Preparation doc generator.
//!
//! Assembles a self-contained markdown document that the user hands
//! off to an LLM (Claude, or whatever they prefer) to get a scaffold
//! of "here's what you did during your review period, here's how it
//! maps to your review questions, here's where to look for details."
//!
//! **The document is not the answer.** By design, the generated doc
//! instructs the LLM to surface material and produce point-form
//! suggestions per review question — NOT to write draft answers. The
//! user still owns the writing; we just save them the archaeology.
//!
//! **Failure modes we don't have.** Captain's Log doesn't fetch any
//! external URLs at generate time. Linked docs (Google Docs, Jira,
//! Confluence, etc.) pass through as URLs; the doc's instructions
//! tell the LLM to fetch via its own connectors. That's an explicit
//! design choice — every generate call is deterministic, offline, and
//! bounded by what's already on disk.
//!
//! **Missing data is fine.** Every input field is optional. The
//! generator gates each section on presence and skips gracefully;
//! the frontend surfaces a "less useful without X" warning before
//! the user hits Generate.

use crate::notes::{iso_year_week, iso_week_start, parse_weekly_summary};
use crate::storage::{StorageBackend, StorageResult};
use chrono::{Datelike, Duration, NaiveDate};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------
// Input shape (mirrors the wizard's collected state)
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPrepInput {
    /// User name, e.g. "Chris Carpenter". Empty → skip identity line.
    pub user_name: Option<String>,
    pub user_email: Option<String>,
    pub job_title: Option<String>,
    pub manager_name: Option<String>,
    pub manager_email: Option<String>,
    /// Uppercased tokens like ["MAGE", "LIVE"]. Empty → skip Jira
    /// lookup instruction.
    #[serde(default)]
    pub jira_project_keys: Vec<String>,
    /// ISO YYYY-MM-DD. Required — the doc has no meaning without a range.
    pub start_date: String,
    pub end_date: String,
    /// Freeform: prose, a URL, or both mixed. Empty → note that the
    /// user didn't provide review questions.
    pub review_questions: Option<String>,
    pub okrs: Option<String>,
    /// Freeform: the user's career development plan — growth goals,
    /// stretch skills, personal milestones. Same shape as `okrs` /
    /// `review_questions` — prose, a URL, or both mixed. Feeds the
    /// LLM's "progress toward development goals" cross-reference.
    pub career_dev_plan: Option<String>,
    /// When true, the assembled doc includes the raw Weekly Notes
    /// section of each week (potentially a lot of text). When false,
    /// only the curated Weekly Summary subsections appear.
    #[serde(default)]
    pub include_notes: bool,
    /// The current date, sent from the frontend as an ISO string so
    /// the backend's "generated on" line is consistent with what the
    /// user sees. Optional — if unset we fall back to omitting the
    /// date line (avoids injecting server-time into the doc).
    pub today_iso: Option<String>,
}

// ---------------------------------------------------------------------
// Best-practice reference URLs (Phase 5 discovery agent, 2026-07-16)
// ---------------------------------------------------------------------

/// Evergreen sources the doc points the LLM at when it moves from
/// "surface material" mode to "proofread the user's drafts" mode.
/// Sourced from a discovery pass on 2026-07-16; kept static (no
/// runtime fetch) so the doc renders identically every time.
const BEST_PRACTICE_URLS: &[(&str, &str)] = &[
    (
        "https://lattice.com/articles/tips-for-writing-a-strong-self-evaluation-plus-specific-examples-to-make-yours-shine",
        "Lattice — Tips for Writing a Strong Self-Evaluation (with examples)",
    ),
    (
        "https://www.cultureamp.com/blog/self-performance-review-examples",
        "Culture Amp — Self-performance review examples by role and category",
    ),
    (
        "https://www.cultureamp.com/blog/how-to-write-a-self-evaluation",
        "Culture Amp — Writing a self-evaluation: STAR-method walkthrough",
    ),
    (
        "https://hbr.org/2023/12/how-to-write-an-effective-self-assessment",
        "Harvard Business Review — How to Write an Effective Self-Assessment",
    ),
];

// ---------------------------------------------------------------------
// ISO week enumeration
// ---------------------------------------------------------------------

/// Enumerate every ISO year+week pair that touches the closed range
/// `[start, end]`. Weeks are ordered chronologically, one entry per
/// week — a range spanning three months yields ~13 entries. The
/// range endpoints are always represented: a start date that falls
/// mid-week emits its own week; an end date does the same.
///
/// Panics only if `start > end` — callers should validate first.
pub fn enumerate_iso_weeks(start: NaiveDate, end: NaiveDate) -> Vec<(u32, u32)> {
    if start > end {
        return Vec::new();
    }
    let mut out = Vec::new();
    let (mut year, mut week) = iso_year_week(start);
    let (end_year, end_week) = iso_year_week(end);
    loop {
        out.push((year, week));
        if year == end_year && week == end_week {
            break;
        }
        // Advance one week using the Monday-plus-seven rule; that
        // handles cross-year rollovers (52 vs 53 weeks) correctly
        // because it always reads the ISO year from chrono's
        // iso_week() rather than doing arithmetic on the week number.
        let next_monday = iso_week_start(year, week) + Duration::days(7);
        let (ny, nw) = iso_year_week(next_monday);
        year = ny;
        week = nw;
    }
    out
}

// ---------------------------------------------------------------------
// Human-readable week label (matches notes.rs's format for consistency)
// ---------------------------------------------------------------------

/// "Jan 6 – Jan 12, 2026" style label for a week's date range. Uses
/// en-dash (U+2013) to match the rest of the app.
fn format_week_range_short(year: u32, week: u32) -> String {
    let start = iso_week_start(year, week);
    let end = start + Duration::days(6);
    let start_month = start.format("%b").to_string();
    let end_month = end.format("%b").to_string();
    if start.year() != end.year() {
        format!(
            "{} {}, {} \u{2013} {} {}, {}",
            start_month, start.day(), start.year(),
            end_month, end.day(), end.year()
        )
    } else if start_month == end_month {
        format!("{} {} \u{2013} {}, {}", start_month, start.day(), end.day(), end.year())
    } else {
        format!(
            "{} {} \u{2013} {} {}, {}",
            start_month, start.day(), end_month, end.day(), end.year()
        )
    }
}

// ---------------------------------------------------------------------
// Doc assembly (pure, testable)
// ---------------------------------------------------------------------

/// One week's contribution to the doc — the raw markdown that lives
/// in `<year>/<year>-W<ww>.md`, or None when the file is missing.
pub struct WeekContent {
    pub year: u32,
    pub week: u32,
    /// Full file text if the week has data on disk, None if the week
    /// is empty (not-yet-journaled). Empty weeks are elided from the
    /// output — no need to advertise gaps to the LLM.
    pub content: Option<String>,
}

/// Assemble the review-prep markdown from the wizard input + the
/// per-week markdown loaded from disk. Pure function — no I/O.
pub fn assemble_review_prep_doc(
    input: &ReviewPrepInput,
    weeks: &[WeekContent],
) -> String {
    let mut out = String::with_capacity(8192);

    // ---- Title + metadata ----
    out.push_str("# Captain's Log — Performance Review Preparation\n\n");

    if let Some(name) = trimmed(&input.user_name) {
        out.push_str(&format!("Prepared for **{}**", name));
        if let Some(today) = trimmed(&input.today_iso) {
            out.push_str(&format!(" on {}", today));
        }
        out.push_str(".\n\n");
    } else if let Some(today) = trimmed(&input.today_iso) {
        out.push_str(&format!("Generated on {}.\n\n", today));
    }

    out.push_str(&format!(
        "Review period: **{} to {}**.\n\n",
        input.start_date, input.end_date
    ));

    // ---- Instructions to the LLM ----
    write_instructions(&mut out, input);

    // ---- Reviewer profile ----
    write_reviewer_profile(&mut out, input);

    // ---- Review questions ----
    write_review_questions(&mut out, input);

    // ---- OKRs ----
    write_okrs(&mut out, input);

    // ---- Career development plan ----
    write_career_dev_plan(&mut out, input);

    // ---- Best-practice references ----
    write_best_practice_references(&mut out);

    // ---- Journal entries ----
    write_journal_entries(&mut out, weeks, input.include_notes);

    out
}

fn trimmed(s: &Option<String>) -> Option<&str> {
    s.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty())
}

fn write_instructions(out: &mut String, input: &ReviewPrepInput) {
    out.push_str("## Instructions for the reviewing LLM\n\n");
    out.push_str(
        "Read this document carefully. Your job is to help me prepare for my performance \
review by surfacing relevant material from my journal — **not** to write review answers \
for me. My goal is to save time hunting through old work; your goal is to give me a \
well-organized starting point so I can write the review myself.\n\n",
    );
    out.push_str("Please do the following, in order:\n\n");

    let mut step = 1;

    out.push_str(&format!(
        "{step}. **Fetch any linked documents.** Some sections below reference external URLs \
(Google Docs, Confluence pages, Jira tickets, spreadsheets, etc.). If you have connectors that \
can access them, please read those materials. If you don't have access, ask me to enable the \
appropriate connector — or I can paste the content directly into our conversation. Some sources \
may be large — a multi-tab OKR sheet, a Jira board, a long Confluence page. Don't ingest a whole \
source; pull only what's relevant: the objectives for my team and period, tickets tied to my \
work within {}–{}, and passages that map to my review questions. If a single fetch is still too \
large, retrieve it in narrower slices.\n\n",
        input.start_date, input.end_date,
    ));
    step += 1;

    if trimmed(&input.review_questions).is_some() {
        out.push_str(&format!(
            "{step}. **Understand the review questions.** Read the *Performance review questions* \
section carefully. These are the questions I need to answer. If any question refers to a \
document not included here — a development plan, competency framework, prior goals, or feedback \
notes — flag it NOW; don't wait until after you've drafted.\n\n",
        ));
        step += 1;
    }

    if trimmed(&input.okrs).is_some() {
        out.push_str(&format!(
            "{step}. **Understand the OKRs.** Read the *Company or team OKRs* section — these \
are the objectives my work is being evaluated against.\n\n",
        ));
        step += 1;
    }

    if trimmed(&input.career_dev_plan).is_some() {
        out.push_str(&format!(
            "{step}. **Understand my career development plan.** Read the *Career development plan* \
section — these are the growth goals and stretch skills I've set for myself. When surfacing \
material from my journal, flag entries that show progress (or a lack of it) against these \
development goals — that thread is often the most useful part of a self-review.\n\n",
        ));
        step += 1;
    }

    if let Some(title) = trimmed(&input.job_title) {
        out.push_str(&format!(
            "{step}. **Research what makes for a good {title}.** Do a brief external search on \
the qualities and behaviours that distinguish a strong practitioner in my role. Use this as \
calibration when scanning my journal entries.\n\n",
        ));
        step += 1;
    }

    if !input.jira_project_keys.is_empty() {
        out.push_str(&format!(
            "{step}. **Look up my Jira work if references are cited.** My Jira project keys are: \
**{}**. If I mention specific tickets or work items, use those keys to look up context (via a \
Jira connector if you have one). Where you can identify a ticket that maps to a bullet you \
surface, include the ticket key + link in the bullet.\n\n",
            input.jira_project_keys.join(", "),
        ));
        step += 1;
    }

    out.push_str(&format!(
        "{step}. **Read through my journal entries.** They cover the weeks I journaled within \
the review period — see the Coverage line at the top of the Journal entries section. Weekly \
Summaries are the curated version of each week; if Notes are included, they're the raw \
material behind the summaries.\n\n",
    ));
    step += 1;

    out.push_str(&format!(
        "{step}. **Confirm you can calibrate before you generate.** Take stock of the review \
questions, OKRs, and any artifact the questions lean on — a development/growth plan, competency \
framework, prior-cycle goals, or recorded manager feedback. If a question depends on material \
not present in this document, STOP and ask me to paste or link it before generating bullets \
for it.\n\n",
    ));
    step += 1;

    out.push_str(&format!(
        "{step}. **For each review question, produce point-form suggestions.** First, scan every \
question against my journal and call out (a) any question the entries support only thinly and \
(b) any that reference material not in this document — for each, tell me what to paste or link. \
Then, for EVERY question — including thinly-supported ones, using whatever partial evidence \
exists rather than skipping them — do the following:\n\
   - Lead each bullet with impact, not activity: state the outcome or change the work produced \
— who/what it affected, and any metric recorded in the journal — not just the task. Let impact \
also drive the ranking below.\n\
   - Up to 3–8 bullets per question is a ceiling for well-supported questions, not a quota. \
For thinly-supported questions, list what evidence you actually have — one honest bullet beats \
five speculative ones.\n\
   - Each bullet should include:\n\
     - A brief description of the work (1–2 sentences), framed around impact/outcome; if my \
journal doesn't record a result or metric, note that so I can fill it in.\n\
     - The week number(s) where the work is documented (e.g. \"See 2026-W12\").\n\
     - A Jira ticket link ONLY when the key appears verbatim in my journal or a connector \
actually returned that ticket. Never construct a key from the project prefix (e.g. don't guess \
\"MAGE-1234\") — omit the link instead.\n\
   - Rank the bullets from most-compelling to least-compelling for that specific question, \
using impact as the primary ranking signal.\n\
   - Distinguish plans from delivered work: \"Plans and priorities for next week\" subsections \
are intentions recorded before the work happened. Don't present a stated plan as completed \
unless corroborated elsewhere (a later week's Key accomplishments, a Jira ticket, or an OKR); \
otherwise label it planned/in-flight and cite the week stated.\n\
   - If my journal gives conflicting or evolving accounts of the same work — a win later \
reversed, a metric that moves — surface the discrepancy and cite every relevant week, not just \
the most flattering. Don't decide which version is \"real\"; an earlier ship and a later \
regression are usually both true.\n\
   - If one accomplishment answers more than one question, surface it under each — but on \
repeats, compress to one line, add a cross-reference, and state the different angle rather \
than repeating the earlier text.\n\n",
    ));
    step += 1;

    out.push_str("**Do not write draft answers.** I want a scaffold, not a script. The value \
is in the point-form suggestions I can turn into my own words.\n\n");

    out.push_str(
        "**Cite only what is in the material I gave you.** Every bullet must trace to a journal \
week or to a document/ticket you actually read. Do not invent or estimate ticket keys, metrics, \
percentages, figures, dates, or outcomes to make a bullet more compelling. If a number isn't \
recorded, write \"(no metric recorded)\" — a truthful vague bullet beats a fabricated precise \
one.\n\n",
    );

    out.push_str(&format!(
        "{step}. **Offer to proofread when I'm ready.** Once I've written my drafts, work \
through them one answer at a time:\n\
   - Confirm each answer addresses the specific question it's under — flag verbatim reuse from \
another answer that would read as filler.\n\
   - **Verify each claim against the journal above.** Flag anything unsupported or overstated, \
including claims of continuous coverage the entries don't back.\n\
   - Push for concrete evidence and first-person ownership language over passive or hedged \
phrasing.\n\
   - Tighten clarity where the draft gets tangled.\n\
   Line-level rewrites are welcome, but keep the final wording mine — and never introduce an \
accomplishment or claim I didn't already write.\n\n",
    ));
}

fn write_reviewer_profile(out: &mut String, input: &ReviewPrepInput) {
    let name = trimmed(&input.user_name);
    let email = trimmed(&input.user_email);
    let title = trimmed(&input.job_title);
    let mgr_name = trimmed(&input.manager_name);
    let mgr_email = trimmed(&input.manager_email);
    let jira = if input.jira_project_keys.is_empty() {
        None
    } else {
        Some(input.jira_project_keys.join(", "))
    };

    // Skip the section entirely when every field is empty — no point
    // rendering an empty heading.
    if name.is_none()
        && email.is_none()
        && title.is_none()
        && mgr_name.is_none()
        && mgr_email.is_none()
        && jira.is_none()
    {
        return;
    }

    out.push_str("## Reviewer profile\n\n");
    if let Some(v) = name {
        out.push_str(&format!("- **Name**: {}\n", v));
    }
    if let Some(v) = email {
        out.push_str(&format!("- **Email**: {}\n", v));
    }
    if let Some(v) = title {
        out.push_str(&format!("- **Job title**: {}\n", v));
    }
    match (mgr_name, mgr_email) {
        (Some(n), Some(e)) => out.push_str(&format!("- **Manager**: {} <{}>\n", n, e)),
        (Some(n), None) => out.push_str(&format!("- **Manager**: {}\n", n)),
        (None, Some(e)) => out.push_str(&format!("- **Manager email**: {}\n", e)),
        (None, None) => {}
    }
    if let Some(v) = jira {
        out.push_str(&format!("- **Jira project keys**: {}\n", v));
    }
    out.push('\n');
}

fn write_review_questions(out: &mut String, input: &ReviewPrepInput) {
    out.push_str("## Performance review questions\n\n");
    match trimmed(&input.review_questions) {
        Some(v) => {
            out.push_str(v);
            out.push_str("\n\n");
        }
        None => {
            out.push_str(
                "> The user did not provide review questions. Before producing suggestions, \
ask them to share the questions (either paste them into our conversation, or link a document \
you have connector access to).\n\n",
            );
        }
    }
}

fn write_okrs(out: &mut String, input: &ReviewPrepInput) {
    out.push_str("## Company or team OKRs\n\n");
    match trimmed(&input.okrs) {
        Some(v) => {
            out.push_str(v);
            out.push_str("\n\n");
        }
        None => {
            out.push_str(
                "> The user did not provide OKR context. If OKRs are important calibration for \
this review, ask them to share the OKR document (paste it, or link a doc you can access via a \
connector).\n\n",
            );
        }
    }
}

fn write_career_dev_plan(out: &mut String, input: &ReviewPrepInput) {
    out.push_str("## Career development plan\n\n");
    match trimmed(&input.career_dev_plan) {
        Some(v) => {
            out.push_str(v);
            out.push_str("\n\n");
        }
        None => {
            out.push_str(
                "> The user did not provide a career development plan. If they have one on file \
(a BambooHR growth doc, a personal dev plan, a manager 1:1 tracker), ask them to share it or link \
it — progress against development goals is often the most useful evidence in a self-review. \
Otherwise proceed without it.\n\n",
            );
        }
    }
}

fn write_best_practice_references(out: &mut String) {
    out.push_str("## Best-practice references for proofreading\n\n");
    out.push_str(
        "When you move from the surfacing pass into the proofreading pass, watch for these \
recurring self-review pitfalls:\n\n\
- **Activities-not-impact** — a bullet that describes what was done but not what changed.\n\
- **Vague or unquantified claims** — \"improved reliability\" with no number, no scope, no \
comparison point.\n\
- **Passive or hedged ownership** — \"was involved in,\" \"helped with,\" \"contributed to\" \
where the journal shows real ownership.\n\
- **False modesty** — a real win understated so much the reviewer can't tell it happened.\n\
- **Recency bias** — the last month over-represented because it's freshest in memory, older \
work under-cited.\n\n\
The links below expand on these:\n\n",
    );
    for (url, title) in BEST_PRACTICE_URLS {
        out.push_str(&format!("- [{}]({})\n", title, url));
    }
    out.push('\n');
}

fn write_journal_entries(out: &mut String, weeks: &[WeekContent], include_notes: bool) {
    out.push_str("## Journal entries\n\n");
    if weeks.iter().all(|w| w.content.is_none()) {
        out.push_str(
            "> No weekly files were found in the review period. Ask the user to confirm the \
date range covers weeks they actually journaled.\n\n",
        );
        return;
    }

    // Coverage line — deterministic gap-truth up front so the LLM
    // can't mistake elided empty weeks (see the `else { continue }`
    // below) for continuous coverage. Journaled weeks are listed
    // explicitly; blank weeks are counted rather than enumerated so
    // long sparse ranges don't produce a wall of week labels.
    let total = weeks.len();
    let journaled_labels: Vec<String> = weeks
        .iter()
        .filter(|w| w.content.is_some())
        .map(|w| format!("{:04}-W{:02}", w.year, w.week))
        .collect();
    let blank_count = total - journaled_labels.len();
    out.push_str(&format!(
        "**Coverage:** {total} weeks in the review period. Journaled: {}. No entry on disk: \
{blank_count}. A missing week means nothing was journaled that week — not necessarily that no \
work happened; ask me if a gap matters.\n\n",
        journaled_labels.join(", "),
    ));

    for week in weeks {
        let Some(content) = &week.content else { continue };
        let label = format!("{:04}-W{:02}", week.year, week.week);
        let range = format_week_range_short(week.year, week.week);

        out.push_str(&format!("### {} ({})\n\n", label, range));

        let summary = parse_weekly_summary(content);

        // Weekly Summary block — omit any subsection that's empty
        // (users often leave one or two blank; no need to advertise
        // that).
        let mut wrote_summary = false;
        let mut push_subsection = |out: &mut String, heading: &str, body: &str| {
            let trimmed = body.trim();
            if trimmed.is_empty() {
                return;
            }
            out.push_str(&format!("#### {}\n\n", heading));
            out.push_str(trimmed);
            out.push_str("\n\n");
        };

        push_subsection(out, "Key accomplishments", &summary.key_accomplishments);
        push_subsection(out, "Plans and priorities for next week", &summary.plans_and_priorities);
        push_subsection(out, "Challenges or roadblocks", &summary.challenges_or_roadblocks);
        push_subsection(out, "Anything else on your mind", &summary.anything_else);
        if !summary.labels.is_empty() {
            out.push_str("#### Labels\n\n");
            for label in &summary.labels {
                out.push_str(&format!("#{} ", label));
            }
            out.push_str("\n\n");
            wrote_summary = true;
        }
        wrote_summary |= !summary.key_accomplishments.trim().is_empty()
            || !summary.plans_and_priorities.trim().is_empty()
            || !summary.challenges_or_roadblocks.trim().is_empty()
            || !summary.anything_else.trim().is_empty();

        if include_notes {
            let notes_body = extract_notes_body(content);
            if !notes_body.trim().is_empty() {
                out.push_str("#### Weekly Notes\n\n");
                out.push_str(notes_body.trim());
                out.push_str("\n\n");
            }
        }

        if !wrote_summary && !include_notes {
            out.push_str("_(No summary content this week.)_\n\n");
        }
    }
}

/// Slice everything after the `## Weekly Notes` heading. Returns "" if
/// the file has no notes section.
fn extract_notes_body(content: &str) -> &str {
    const NOTES: &str = "## Weekly Notes";
    match content.find(NOTES) {
        Some(idx) => {
            let after = &content[idx + NOTES.len()..];
            // Skip the newline right after the heading.
            after.strip_prefix('\n').unwrap_or(after)
        }
        None => "",
    }
}

// ---------------------------------------------------------------------
// Fetch orchestration (async, hits storage)
// ---------------------------------------------------------------------

pub async fn fetch_week_contents<B: StorageBackend + ?Sized>(
    backend: &B,
    weeks: &[(u32, u32)],
) -> StorageResult<Vec<WeekContent>> {
    let mut out = Vec::with_capacity(weeks.len());
    for &(year, week) in weeks {
        let content = backend.read_week(year, week).await?;
        out.push(WeekContent { year, week, content });
    }
    Ok(out)
}

/// Validate + parse the input's date range into NaiveDate. Returns an
/// error string suitable for the Tauri command's `Err` arm.
pub fn parse_date_range(start_iso: &str, end_iso: &str) -> Result<(NaiveDate, NaiveDate), String> {
    let start = NaiveDate::parse_from_str(start_iso.trim(), "%Y-%m-%d")
        .map_err(|_| format!("start_date must be YYYY-MM-DD (got: {:?})", start_iso))?;
    let end = NaiveDate::parse_from_str(end_iso.trim(), "%Y-%m-%d")
        .map_err(|_| format!("end_date must be YYYY-MM-DD (got: {:?})", end_iso))?;
    if start > end {
        return Err(format!(
            "start_date ({}) is after end_date ({})",
            start_iso, end_iso
        ));
    }
    Ok((start, end))
}

// ---------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn mk_input(start: &str, end: &str) -> ReviewPrepInput {
        ReviewPrepInput {
            user_name: Some("Chris Carpenter".into()),
            user_email: Some("chris@example.com".into()),
            job_title: Some("QA Analyst".into()),
            manager_name: Some("Jane Doe".into()),
            manager_email: Some("jane@example.com".into()),
            jira_project_keys: vec!["MAGE".into(), "LIVE".into()],
            start_date: start.into(),
            end_date: end.into(),
            review_questions: Some("What did you ship this year?".into()),
            okrs: Some("Deliver Q3 initiatives on time.".into()),
            career_dev_plan: Some(
                "Grow into a senior QA role over the next 12 months.".into(),
            ),
            include_notes: false,
            today_iso: Some("2026-07-16".into()),
        }
    }

    // ---- ISO week enumeration ----

    #[test]
    fn enumerate_single_week_range() {
        let weeks = enumerate_iso_weeks(d(2026, 7, 13), d(2026, 7, 19));
        assert_eq!(weeks, vec![(2026, 29)]);
    }

    #[test]
    fn enumerate_multiple_weeks_within_year() {
        let weeks = enumerate_iso_weeks(d(2026, 7, 13), d(2026, 7, 26));
        assert_eq!(weeks, vec![(2026, 29), (2026, 30)]);
    }

    #[test]
    fn enumerate_month_span_gives_5_or_6_weeks() {
        // Jan 1 to Jan 31, 2026.
        let weeks = enumerate_iso_weeks(d(2026, 1, 1), d(2026, 1, 31));
        // 2026 W1 starts Dec 29 2025. Jan 1 falls in W1. Jan 31 falls
        // in W5. 5 weeks total.
        assert_eq!(weeks.len(), 5);
        assert_eq!(weeks.first(), Some(&(2026, 1)));
        assert_eq!(weeks.last(), Some(&(2026, 5)));
    }

    #[test]
    fn enumerate_cross_year_boundary() {
        // Dec 15, 2025 to Jan 15, 2026 — spans the year boundary. The
        // ISO year switches somewhere in the middle.
        let weeks = enumerate_iso_weeks(d(2025, 12, 15), d(2026, 1, 15));
        // Should include weeks from both years, contiguous.
        assert!(weeks.len() >= 4);
        let last_2025 = weeks.iter().rev().find(|(y, _)| *y == 2025);
        let first_2026 = weeks.iter().find(|(y, _)| *y == 2026);
        assert!(last_2025.is_some(), "expected at least one 2025 week");
        assert!(first_2026.is_some(), "expected at least one 2026 week");
    }

    #[test]
    fn enumerate_full_6_month_review_period() {
        // Jan 6, 2026 (Mon of W2) to July 5, 2026 (Sun of W27).
        let weeks = enumerate_iso_weeks(d(2026, 1, 6), d(2026, 7, 5));
        assert_eq!(weeks.first(), Some(&(2026, 2)));
        assert_eq!(weeks.last(), Some(&(2026, 27)));
        assert_eq!(weeks.len(), 26);
    }

    #[test]
    fn enumerate_start_after_end_returns_empty() {
        let weeks = enumerate_iso_weeks(d(2026, 7, 20), d(2026, 7, 13));
        assert!(weeks.is_empty());
    }

    #[test]
    fn enumerate_iso_week_1_edge_case() {
        // 2025-12-29 is Mon of ISO W1 2026 — enumerating a range that
        // starts there should NOT double-count.
        let weeks = enumerate_iso_weeks(d(2025, 12, 29), d(2026, 1, 4));
        assert_eq!(weeks, vec![(2026, 1)]);
    }

    // ---- Date parsing ----

    #[test]
    fn parse_date_range_valid_input() {
        let (s, e) = parse_date_range("2026-01-01", "2026-07-01").unwrap();
        assert_eq!(s, d(2026, 1, 1));
        assert_eq!(e, d(2026, 7, 1));
    }

    #[test]
    fn parse_date_range_rejects_start_after_end() {
        let err = parse_date_range("2026-07-01", "2026-01-01").unwrap_err();
        assert!(err.contains("after"), "unexpected message: {err}");
    }

    #[test]
    fn parse_date_range_rejects_garbage_input() {
        assert!(parse_date_range("nope", "2026-07-01").is_err());
        assert!(parse_date_range("2026-07-01", "not-a-date").is_err());
    }

    // ---- Human-readable week range ----

    #[test]
    fn format_week_range_same_month() {
        // 2026-W29 is Jul 13-19, 2026. Same year, same month.
        assert_eq!(format_week_range_short(2026, 29), "Jul 13 \u{2013} 19, 2026");
    }

    #[test]
    fn format_week_range_cross_month_same_year() {
        // 2026-W5 spans late-Jan into early-Feb.
        let s = format_week_range_short(2026, 5);
        assert!(s.contains("Jan"), "expected Jan in {s}");
        assert!(s.contains("Feb"), "expected Feb in {s}");
        assert!(s.contains("2026"), "expected 2026 in {s}");
    }

    #[test]
    fn format_week_range_cross_year() {
        // 2026-W1 spans Dec 29 2025 → Jan 4 2026.
        let s = format_week_range_short(2026, 1);
        assert!(s.contains("2025"), "expected 2025 in {s}");
        assert!(s.contains("2026"), "expected 2026 in {s}");
    }

    // ---- Assembly ----

    #[test]
    fn assemble_includes_title_and_meta() {
        let input = mk_input("2026-01-06", "2026-07-05");
        let doc = assemble_review_prep_doc(&input, &[]);
        assert!(doc.starts_with("# Captain's Log — Performance Review Preparation"));
        assert!(doc.contains("Prepared for **Chris Carpenter** on 2026-07-16"));
        assert!(doc.contains("2026-01-06 to 2026-07-05"));
    }

    #[test]
    fn assemble_skips_meta_line_when_name_and_today_missing() {
        let mut input = mk_input("2026-01-06", "2026-07-05");
        input.user_name = None;
        input.today_iso = None;
        let doc = assemble_review_prep_doc(&input, &[]);
        assert!(!doc.contains("Prepared for"));
        assert!(!doc.contains("Generated on"));
        // Review period line still present.
        assert!(doc.contains("2026-01-06 to 2026-07-05"));
    }

    #[test]
    fn assemble_includes_instructions_section() {
        let input = mk_input("2026-01-06", "2026-07-05");
        let doc = assemble_review_prep_doc(&input, &[]);
        assert!(doc.contains("## Instructions for the reviewing LLM"));
        assert!(doc.contains("Do not write draft answers"));
        assert!(doc.contains("point-form suggestions"));
    }

    #[test]
    fn assemble_instructions_omit_jira_step_when_keys_missing() {
        let mut input = mk_input("2026-01-06", "2026-07-05");
        input.jira_project_keys.clear();
        let doc = assemble_review_prep_doc(&input, &[]);
        assert!(!doc.contains("Look up my Jira work"));
        assert!(!doc.contains("Jira project keys"));
    }

    #[test]
    fn assemble_instructions_omit_job_title_research_when_title_missing() {
        let mut input = mk_input("2026-01-06", "2026-07-05");
        input.job_title = None;
        let doc = assemble_review_prep_doc(&input, &[]);
        assert!(!doc.contains("makes for a good"));
    }

    #[test]
    fn assemble_instructions_omit_okr_step_when_okrs_missing() {
        let mut input = mk_input("2026-01-06", "2026-07-05");
        input.okrs = None;
        let doc = assemble_review_prep_doc(&input, &[]);
        assert!(!doc.contains("Understand the OKRs"));
    }

    #[test]
    fn assemble_instructions_omit_career_dev_step_when_missing() {
        let mut input = mk_input("2026-01-06", "2026-07-05");
        input.career_dev_plan = None;
        let doc = assemble_review_prep_doc(&input, &[]);
        assert!(!doc.contains("Understand my career development plan"));
    }

    #[test]
    fn assemble_surfaces_missing_career_dev_plan_as_prompt_to_llm() {
        let mut input = mk_input("2026-01-06", "2026-07-05");
        input.career_dev_plan = None;
        let doc = assemble_review_prep_doc(&input, &[]);
        assert!(doc.contains("## Career development plan"));
        assert!(doc.contains("The user did not provide a career development plan"));
    }

    #[test]
    fn assemble_includes_career_dev_plan_when_present() {
        let input = mk_input("2026-01-06", "2026-07-05");
        let doc = assemble_review_prep_doc(&input, &[]);
        assert!(doc.contains("## Career development plan"));
        assert!(doc.contains("Grow into a senior QA role"));
    }

    #[test]
    fn assemble_includes_reviewer_profile_with_manager() {
        let input = mk_input("2026-01-06", "2026-07-05");
        let doc = assemble_review_prep_doc(&input, &[]);
        assert!(doc.contains("## Reviewer profile"));
        assert!(doc.contains("Chris Carpenter"));
        assert!(doc.contains("Jane Doe <jane@example.com>"));
    }

    #[test]
    fn assemble_omits_reviewer_profile_when_all_fields_empty() {
        let input = ReviewPrepInput {
            user_name: None,
            user_email: None,
            job_title: None,
            manager_name: None,
            manager_email: None,
            jira_project_keys: vec![],
            start_date: "2026-01-06".into(),
            end_date: "2026-07-05".into(),
            review_questions: None,
            okrs: None,
            career_dev_plan: None,
            include_notes: false,
            today_iso: None,
        };
        let doc = assemble_review_prep_doc(&input, &[]);
        assert!(!doc.contains("## Reviewer profile"));
    }

    #[test]
    fn assemble_surfaces_missing_review_questions_as_prompt_to_llm() {
        let mut input = mk_input("2026-01-06", "2026-07-05");
        input.review_questions = None;
        let doc = assemble_review_prep_doc(&input, &[]);
        assert!(doc.contains("## Performance review questions"));
        assert!(doc.contains("The user did not provide review questions"));
    }

    #[test]
    fn assemble_surfaces_missing_okrs_as_prompt_to_llm() {
        let mut input = mk_input("2026-01-06", "2026-07-05");
        input.okrs = None;
        let doc = assemble_review_prep_doc(&input, &[]);
        assert!(doc.contains("## Company or team OKRs"));
        assert!(doc.contains("did not provide OKR context"));
    }

    #[test]
    fn assemble_includes_best_practice_references() {
        let input = mk_input("2026-01-06", "2026-07-05");
        let doc = assemble_review_prep_doc(&input, &[]);
        assert!(doc.contains("## Best-practice references"));
        assert!(doc.contains("Lattice"));
        assert!(doc.contains("Culture Amp"));
        assert!(doc.contains("Harvard Business Review"));
    }

    #[test]
    fn assemble_journal_section_notes_empty_when_no_weeks() {
        let input = mk_input("2026-01-06", "2026-07-05");
        let doc = assemble_review_prep_doc(&input, &[]);
        assert!(doc.contains("## Journal entries"));
        assert!(doc.contains("No weekly files were found"));
    }

    #[test]
    fn assemble_journal_extracts_summary_subsections() {
        let input = mk_input("2026-07-13", "2026-07-19");
        let content = r#"---
period: 2026-W29
---
# Week of Jul 13 - 19, 2026
## Weekly Summary
*Last updated: 2026-07-15 08:00*

### Key accomplishments
Shipped Phase 4 link chips.

### Plans and priorities for next week
Kick off Phase 5.

### Challenges or roadblocks

### Anything else on your mind
Also fixed a WebKit rendering bug.

### Labels

### Tasks
<!-- captainslog:tasks:incomplete -->
<!-- captainslog:tasks:completed -->
- [x] Ship Phase 4
## Weekly Notes
"#;
        let doc = assemble_review_prep_doc(
            &input,
            &[WeekContent {
                year: 2026,
                week: 29,
                content: Some(content.into()),
            }],
        );
        assert!(doc.contains("### 2026-W29"));
        assert!(doc.contains("#### Key accomplishments"));
        assert!(doc.contains("Shipped Phase 4 link chips."));
        assert!(doc.contains("#### Plans and priorities for next week"));
        assert!(doc.contains("Kick off Phase 5."));
        // Empty Challenges section should be omitted.
        assert!(!doc.contains("#### Challenges or roadblocks"));
        assert!(doc.contains("Also fixed a WebKit rendering bug."));
    }

    #[test]
    fn assemble_journal_omits_notes_when_include_notes_false() {
        let mut input = mk_input("2026-07-13", "2026-07-19");
        input.include_notes = false;
        let content = "## Weekly Summary\n\
### Key accomplishments\nBig win.\n\
## Weekly Notes\n\
### 2026-07-14 10:00 — Standup\nMet with team.\n";
        let doc = assemble_review_prep_doc(
            &input,
            &[WeekContent {
                year: 2026,
                week: 29,
                content: Some(content.into()),
            }],
        );
        assert!(doc.contains("Big win."));
        assert!(!doc.contains("Met with team."));
        assert!(!doc.contains("#### Weekly Notes"));
    }

    #[test]
    fn assemble_journal_includes_notes_when_include_notes_true() {
        let mut input = mk_input("2026-07-13", "2026-07-19");
        input.include_notes = true;
        let content = "## Weekly Summary\n\
### Key accomplishments\nBig win.\n\
## Weekly Notes\n\
### 2026-07-14 10:00 — Standup\nMet with team.\n";
        let doc = assemble_review_prep_doc(
            &input,
            &[WeekContent {
                year: 2026,
                week: 29,
                content: Some(content.into()),
            }],
        );
        assert!(doc.contains("Big win."));
        assert!(doc.contains("Met with team."));
        assert!(doc.contains("#### Weekly Notes"));
    }

    #[test]
    fn assemble_journal_skips_empty_weeks_from_disk() {
        // Two weeks in range; only one has content. Empty week is
        // silently omitted (no "no data this week" placeholder).
        let input = mk_input("2026-07-06", "2026-07-19");
        let weeks = vec![
            WeekContent { year: 2026, week: 28, content: None },
            WeekContent {
                year: 2026,
                week: 29,
                content: Some(
                    "## Weekly Summary\n### Key accomplishments\nSomething.\n".into(),
                ),
            },
        ];
        let doc = assemble_review_prep_doc(&input, &weeks);
        assert!(!doc.contains("2026-W28"));
        assert!(doc.contains("2026-W29"));
        assert!(doc.contains("Something."));
    }

    #[test]
    fn extract_notes_body_returns_empty_when_notes_section_missing() {
        let content = "## Weekly Summary\nSome content, no notes header.\n";
        assert_eq!(extract_notes_body(content).trim(), "");
    }

    #[test]
    fn extract_notes_body_returns_content_after_heading() {
        let content = "## Weekly Summary\n\n## Weekly Notes\n### note 1\nbody\n";
        let body = extract_notes_body(content);
        assert!(body.contains("### note 1"));
        assert!(body.contains("body"));
    }

    #[test]
    fn empty_string_fields_treated_as_missing() {
        let mut input = mk_input("2026-01-06", "2026-07-05");
        input.user_name = Some("   ".into());
        input.job_title = Some("".into());
        let doc = assemble_review_prep_doc(&input, &[]);
        assert!(!doc.contains("Prepared for"));
        assert!(!doc.contains("makes for a good"));
    }

    // ---------------------------------------------------------------
    // Prompt-sharpening pass (2026-07-21) — every load-bearing edit
    // in the LLM instructions has a test so a future refactor can't
    // silently drop the guardrail.
    // ---------------------------------------------------------------

    #[test]
    fn pre_generation_checkpoint_always_present() {
        // Even with every optional field empty, the calibration STOP-
        // and-ask step must still render. It's the load-bearing gate
        // that catches "question refers to a doc we didn't include".
        let input = ReviewPrepInput {
            user_name: None,
            user_email: None,
            job_title: None,
            manager_name: None,
            manager_email: None,
            jira_project_keys: vec![],
            start_date: "2026-01-06".into(),
            end_date: "2026-07-05".into(),
            review_questions: None,
            okrs: None,
            career_dev_plan: None,
            include_notes: false,
            today_iso: None,
        };
        let doc = assemble_review_prep_doc(&input, &[]);
        assert!(doc.contains("Confirm you can calibrate before you generate"));
        assert!(doc.contains("STOP and ask me to paste or link it"));
    }

    #[test]
    fn step_2_flags_referenced_but_absent_material() {
        // When review questions are provided, the step-2 instruction
        // must include the "flag it NOW" appendix pushing the LLM to
        // surface missing-artifact dependencies up front.
        let input = mk_input("2026-01-06", "2026-07-05");
        let doc = assemble_review_prep_doc(&input, &[]);
        assert!(doc.contains("flag it NOW"));
        assert!(doc.contains("don't wait until after you've drafted"));
    }

    #[test]
    fn selective_extraction_hint_present_and_interpolates_date_range() {
        // Step 1 must advise pulling only relevant slices from large
        // sources, and must interpolate the actual review-period
        // range so the LLM has a concrete filter.
        let input = mk_input("2026-01-06", "2026-07-05");
        let doc = assemble_review_prep_doc(&input, &[]);
        assert!(doc.contains("Don't ingest a whole source"));
        assert!(doc.contains("2026-01-06"));
        assert!(doc.contains("2026-07-05"));
        assert!(doc.contains("narrower slices"));
    }

    #[test]
    fn anti_fabrication_clause_present() {
        let input = mk_input("2026-01-06", "2026-07-05");
        let doc = assemble_review_prep_doc(&input, &[]);
        assert!(doc.contains("Cite only what is in the material I gave you"));
        assert!(doc.contains("Do not invent or estimate"));
        assert!(doc.contains("(no metric recorded)"));
    }

    #[test]
    fn answerability_guard_and_ceiling_not_quota_present() {
        let input = mk_input("2026-01-06", "2026-07-05");
        let doc = assemble_review_prep_doc(&input, &[]);
        assert!(doc.contains("scan every question against my journal"));
        assert!(doc.contains("thinly-supported"));
        assert!(doc.contains("ceiling for well-supported questions, not a quota"));
    }

    #[test]
    fn impact_lead_directive_present() {
        let input = mk_input("2026-01-06", "2026-07-05");
        let doc = assemble_review_prep_doc(&input, &[]);
        assert!(doc.contains("Lead each bullet with impact, not activity"));
        assert!(doc.contains("framed around impact/outcome"));
        assert!(doc.contains("using impact as the primary ranking signal"));
    }

    #[test]
    fn plans_vs_delivered_and_contradictions_rules_present() {
        let input = mk_input("2026-01-06", "2026-07-05");
        let doc = assemble_review_prep_doc(&input, &[]);
        assert!(doc.contains("Distinguish plans from delivered work"));
        assert!(doc.contains("Plans and priorities"));
        assert!(doc.contains("conflicting or evolving accounts"));
        assert!(doc.contains("surface the discrepancy"));
    }

    #[test]
    fn cross_question_dedupe_rule_present() {
        let input = mk_input("2026-01-06", "2026-07-05");
        let doc = assemble_review_prep_doc(&input, &[]);
        assert!(doc.contains("If one accomplishment answers more than one question"));
        assert!(doc.contains("compress to one line"));
        assert!(doc.contains("cross-reference"));
    }

    #[test]
    fn jira_key_verbatim_rule_replaces_context_guessing() {
        let input = mk_input("2026-01-06", "2026-07-05");
        let doc = assemble_review_prep_doc(&input, &[]);
        assert!(doc.contains("A Jira ticket link ONLY when the key appears verbatim"));
        assert!(doc.contains("don't guess"));
        assert!(doc.contains("omit the link instead"));
        // The pre-critique phrasing licensed guessing from context —
        // make sure it's gone from the bullet spec.
        assert!(
            !doc.contains("A Jira ticket link where you can identify one from context"),
            "old context-guessing phrasing must be removed"
        );
    }

    #[test]
    fn proofread_step_verifies_claims_against_journal() {
        let input = mk_input("2026-01-06", "2026-07-05");
        let doc = assemble_review_prep_doc(&input, &[]);
        assert!(doc.contains("Verify each claim against the journal above"));
        assert!(doc.contains("first-person ownership"));
    }

    #[test]
    fn proofread_step_bounds_line_level_rewrites() {
        let input = mk_input("2026-01-06", "2026-07-05");
        let doc = assemble_review_prep_doc(&input, &[]);
        assert!(doc.contains(
            "never introduce an accomplishment or claim I didn't already write"
        ));
    }

    #[test]
    fn pitfalls_list_precedes_reference_urls() {
        let input = mk_input("2026-01-06", "2026-07-05");
        let doc = assemble_review_prep_doc(&input, &[]);
        // All five named pitfalls must render.
        assert!(doc.contains("Activities-not-impact"));
        assert!(doc.contains("Vague or unquantified claims"));
        assert!(doc.contains("Passive or hedged ownership"));
        assert!(doc.contains("False modesty"));
        assert!(doc.contains("Recency bias"));
        // The pitfalls block must sit above the reference URLs; find
        // the first pitfall and the first reference URL and compare
        // positions.
        let pitfall_ix = doc.find("Activities-not-impact").unwrap();
        let first_ref_ix = doc.find("The links below expand on these").unwrap();
        assert!(pitfall_ix < first_ref_ix);
    }

    #[test]
    fn coverage_line_lists_journaled_weeks_and_counts_blanks() {
        // Mixed range: 2 journaled weeks, 1 blank in between.
        let weeks = vec![
            WeekContent {
                year: 2026,
                week: 10,
                content: Some(
                    "## Weekly Summary\n### Key accomplishments\nAlpha work.\n".into(),
                ),
            },
            WeekContent {
                year: 2026,
                week: 11,
                content: None,
            },
            WeekContent {
                year: 2026,
                week: 12,
                content: Some(
                    "## Weekly Summary\n### Key accomplishments\nBeta work.\n".into(),
                ),
            },
        ];
        let input = mk_input("2026-03-02", "2026-03-22");
        let doc = assemble_review_prep_doc(&input, &weeks);
        assert!(doc.contains("**Coverage:** 3 weeks in the review period"));
        assert!(doc.contains("Journaled: 2026-W10, 2026-W12"));
        assert!(doc.contains("No entry on disk: 1"));
        assert!(doc.contains("ask me if a gap matters"));
    }

    #[test]
    fn read_journal_step_no_longer_overclaims_continuous_coverage() {
        let input = mk_input("2026-01-06", "2026-07-05");
        let doc = assemble_review_prep_doc(&input, &[]);
        // The old verbatim phrasing that misled the LLM must be gone.
        assert!(
            !doc.contains("They cover the review period. Weekly"),
            "old overclaim wording must not remain in the read-journal step"
        );
        assert!(doc.contains("They cover the weeks I journaled within the review period"));
    }
}
