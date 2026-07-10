//! Phase 3c task-list aggregator.
//!
//! Extracts `- [ ]` / `- [x]` items from the `### Tasks` section of
//! each weekly file (delimited by `<!-- captainslog:tasks:incomplete -->`
//! and `<!-- captainslog:tasks:completed -->` anchors), and reconciles
//! with sidecars (`task-completions.json`, `task-due-dates.json`,
//! `rollover-log.json`). Completion timestamps are keyed by
//! `(year, week, text_hash, ordinal)`.
//!
//! **Identity model.** A task is identified by the composite key
//! `(year, week, text_hash, ordinal)`. The hash is SHA-256 of the
//! *normalized* text (trim + collapse whitespace + lowercase + strip
//! trailing punctuation). `ordinal` disambiguates duplicate tasks with
//! the same normalized text inside the same week's Plans section: 0
//! for the first occurrence, 1 for the second, etc.
//!
//! **Reconciliation rule.** Markdown wins for *state* (checked / not),
//! sidecar wins for *timestamps*. In particular:
//!
//! - Markdown `[x]` + no sidecar entry → sidecar entry is backfilled
//!   at load time using the file's mtime, so users who check tasks in
//!   an external editor still get an approximate `completed_at`.
//! - Markdown `[ ]` + sidecar entry → sidecar entry is dropped (the
//!   user un-checked externally).
//! - Sidecar entry that matches no current task → garbage-collected.

use crate::notes::{TASKS_ANCHOR_COMPLETED, TASKS_ANCHOR_INCOMPLETE};
use crate::storage::{StorageBackend, StorageError, StorageResult};
use pulldown_cmark::{html, Options, Parser};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

const TASK_COMPLETIONS_FILE: &str = "task-completions.json";
const CURRENT_TASK_COMPLETIONS_VERSION: u32 = 1;

const ROLLOVER_LOG_FILE: &str = "rollover-log.json";

/// Slice 6c-followup — persistent record of "when did the auto-
/// import last run" so the trigger-event handlers can no-op if the
/// same local day has already been serviced. Missing file →
/// treated as "never run" (import runs on the next trigger).
const AUTO_IMPORT_LOG_FILE: &str = "auto-import-log.json";
const CURRENT_ROLLOVER_LOG_VERSION: u32 = 1;

/// Phase 3e — sidecar mapping tasks to their optional due date.
/// Keyed by `(year, week, textHash, ordinal)` — identical shape to
/// `TaskCompletions` + `RolloverLog`. Value is a LOCAL YYYY-MM-DD
/// string (no time-of-day; reminders' time-of-day lives on the
/// reminder settings, not per task). Missing file → empty; corrupt
/// file → empty + stderr warning.
const TASK_DUE_DATES_FILE: &str = "task-due-dates.json";
const CURRENT_TASK_DUE_DATES_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// A single task extracted from a Plans section.
///
/// `byte_offset_in_plans` + `line_length` locate the source line
/// within the Plans-section string, so a future toggle_task command
/// can splice `[ ]` ↔ `[x]` in place without re-parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedTask {
    /// Task text with the `- [ ] ` / `- [x] ` prefix stripped, but
    /// otherwise preserved as-written (case, punctuation, trailing
    /// whitespace inside the line). Use `text_hash` for identity
    /// comparisons — never compare `text` directly.
    pub text: String,
    /// SHA-256 hex of the *normalized* task text.
    pub text_hash: String,
    /// Duplicate disambiguator within a single Plans section. Starts
    /// at 0 and increments per additional occurrence of `text_hash`.
    pub ordinal: u32,
    /// True if the source line was `- [x]` (or `- [X]`).
    pub is_completed: bool,
    /// Byte offset of the start of this task's line within the Plans
    /// section string passed to [`parse_plans_tasks`].
    pub byte_offset_in_plans: usize,
    /// Length of the task line in bytes (from the first non-whitespace
    /// character to end-of-line, excluding the trailing `\n`).
    pub line_length: usize,
}

/// Extract every `- [ ]` / `- [x]` task from a Plans-section body.
///
/// `plans_content` is expected to be the raw text between the
/// `### Plans and priorities for next week` heading and the next
/// heading, i.e. exactly what `parse_weekly_summary` returns as
/// `plans_and_priorities`.
///
/// Non-checkbox bullets (`- Just a regular bullet`) are ignored.
/// Indentation is preserved for offset math but does not affect
/// output — nested tasks surface at the same level as top-level ones.
/// The checkbox marker is case-insensitive on `x`.
pub fn parse_plans_tasks(plans_content: &str) -> Vec<ParsedTask> {
    let mut tasks: Vec<ParsedTask> = Vec::new();
    let mut hash_counts: HashMap<String, u32> = HashMap::new();

    let mut cursor: usize = 0;
    for line_with_newline in plans_content.split_inclusive('\n') {
        let line_start = cursor;
        cursor += line_with_newline.len();

        let line = line_with_newline.trim_end_matches('\n');
        let Some((is_completed, text_start)) = match_task_marker(line) else {
            continue;
        };
        let text = line[text_start..].to_string();
        let normalized = normalize_task_text(&text);
        if normalized.is_empty() {
            // "- [ ] " with no meaningful text.
            continue;
        }
        let text_hash = hash_task_text(&normalized);
        let ordinal = *hash_counts
            .entry(text_hash.clone())
            .and_modify(|n| *n += 1)
            .or_insert(0);

        tasks.push(ParsedTask {
            text,
            text_hash,
            ordinal,
            is_completed,
            byte_offset_in_plans: line_start,
            line_length: line.len(),
        });
    }
    tasks
}

/// Match a line against the `- [ ]` / `- [x]` task-list pattern.
///
/// Returns `Some((is_completed, text_start_offset_in_line))` for a
/// task line, or `None` otherwise. The offset points at the first
/// byte of the task text (i.e. past the closing `]` and any following
/// whitespace).
fn match_task_marker(line: &str) -> Option<(bool, usize)> {
    let trimmed = line.trim_start();
    let leading_ws = line.len() - trimmed.len();
    let after_dash = trimmed.strip_prefix("- [")?;
    let mut chars = after_dash.chars();
    let marker = chars.next()?;
    let is_completed = match marker {
        ' ' => false,
        'x' | 'X' => true,
        _ => return None,
    };
    // Marker must be followed immediately by `]`.
    if chars.next()? != ']' {
        return None;
    }
    // Bytes consumed so far, relative to `line`:
    //   leading_ws + "- [".len() (3) + marker (1) + "]" (1) = leading_ws + 5.
    let after_bracket_offset = leading_ws + 5;
    let after_bracket = &line[after_bracket_offset..];
    let ws_after = after_bracket.len() - after_bracket.trim_start().len();
    Some((is_completed, after_bracket_offset + ws_after))
}

/// Normalize task text for identity comparison.
///
/// - Trim + collapse internal whitespace runs to single spaces.
/// - Lowercase (Unicode-aware via `char::to_lowercase`).
/// - Strip trailing ASCII punctuation (`.,!?:;`).
///
/// Example: `"  Ship  the  Thing!!  "` → `"ship the thing"`.
pub fn normalize_task_text(text: &str) -> String {
    let collapsed: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut lowered: String = collapsed.chars().flat_map(|c| c.to_lowercase()).collect();
    while lowered
        .chars()
        .last()
        .is_some_and(|c| matches!(c, '.' | ',' | '!' | '?' | ':' | ';'))
    {
        lowered.pop();
    }
    lowered
}

/// SHA-256 hex of the normalized task text. Length-prefixed input
/// following the pattern from `sent_log::hash_weekly_summary` — the
/// prefix is cheap insurance against future callers concatenating
/// inputs and expecting the same hash.
pub fn hash_task_text(normalized_text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update((normalized_text.len() as u64).to_le_bytes());
    hasher.update(normalized_text.as_bytes());
    format!("{:x}", hasher.finalize())
}

// ---------------------------------------------------------------------------
// Constants shared across task helpers
// ---------------------------------------------------------------------------

/// Maximum bytes we allow in a single task's text. Well above what a
/// human writes in a to-do line; the cap exists to bound the file's
/// growth and keep the render pipeline predictable.
pub const MAX_TASK_TEXT_LEN: usize = 1024;

// ---------------------------------------------------------------------------
// Import completed tasks into the Key accomplishments field
// ---------------------------------------------------------------------------

/// The sub-heading string the import inserts above imported bullets.
/// Callers walk for it (case-insensitive on the label text) so
/// repeated imports append under the SAME heading instead of stacking
/// new heading blocks on every click.
pub const COMPLETED_TASKS_HEADING: &str = "#### Completed Tasks";

/// Result of [`merge_completed_tasks_into_key_accomplishments`].
/// The command layer converts this into a user-facing receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeCompletedResult {
    /// New key_accomplishments text (may equal input verbatim if
    /// every candidate was deduped).
    pub new_key_accomplishments: String,
    /// Number of completed tasks actually added.
    pub imported: u32,
    /// Number of completed tasks skipped because a normalized match
    /// already existed in the body.
    pub skipped: u32,
}

/// Merge a list of completed task texts into a Key accomplishments
/// body. Handles two concerns at once:
///
/// **Dedupe.** Each candidate is normalized via [`normalize_task_text`]
/// and compared against every line already in the body (with any
/// leading `- ` bullet marker stripped before normalization). If the
/// normalized form already exists, the candidate is skipped — so
/// repeated imports don't stack duplicates and hand-typed prose that
/// happens to match a completed task also wins.
///
/// **Placement.** If the body already contains a
/// [`COMPLETED_TASKS_HEADING`] line, new bullets are inserted at the
/// END of the contiguous bullet block that follows that heading
/// (walking forward, accepting blank lines interspersed, stopping at
/// any new heading or non-blank prose line). If no such heading
/// exists, a fresh heading + bullets block is appended to the body,
/// separated by a blank line if the body was non-empty.
pub fn merge_completed_tasks_into_key_accomplishments(
    key_accomplishments: &str,
    completed_task_texts: &[String],
) -> MergeCompletedResult {
    // Build the dedupe set: every existing line's normalized form.
    let existing_norms: std::collections::HashSet<String> = key_accomplishments
        .split('\n')
        .map(|line| {
            let stripped = line.trim_start().trim_start_matches("- ").trim();
            normalize_task_text(stripped)
        })
        .filter(|n| !n.is_empty())
        .collect();

    // Walk candidates in order. Track our own additions in `seen` so
    // two identical completed tasks in the same batch don't both land.
    let mut seen = existing_norms;
    let mut fresh: Vec<&str> = Vec::new();
    let mut skipped: u32 = 0;
    for text in completed_task_texts {
        let norm = normalize_task_text(text);
        if norm.is_empty() {
            continue;
        }
        if seen.contains(&norm) {
            skipped += 1;
        } else {
            fresh.push(text.as_str());
            seen.insert(norm);
        }
    }

    if fresh.is_empty() {
        return MergeCompletedResult {
            new_key_accomplishments: key_accomplishments.to_string(),
            imported: 0,
            skipped,
        };
    }

    let bullets: Vec<String> = fresh.iter().map(|t| format!("- {}", t)).collect();
    let imported: u32 = fresh.len() as u32;

    // Look for an existing "#### Completed Tasks" heading. Tolerate
    // case + trailing-whitespace variants ("#### completed tasks  ").
    let lines: Vec<&str> = key_accomplishments.split('\n').collect();
    let heading_idx = lines.iter().position(|line| {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("#### ") {
            normalize_task_text(rest) == "completed tasks"
        } else {
            false
        }
    });

    let new_body = if let Some(hi) = heading_idx {
        // Walk forward to find the end of the contiguous bullet block
        // under this heading. Blank lines are tolerated (folks
        // separate bullet groups with blanks); a new heading OR a
        // non-blank prose line ends the block.
        let mut last_bullet_idx: Option<usize> = None;
        for j in (hi + 1)..lines.len() {
            let trimmed = lines[j].trim_start();
            if is_markdown_heading_line(trimmed) {
                break;
            } else if trimmed.starts_with("- ") {
                last_bullet_idx = Some(j);
            } else if trimmed.is_empty() {
                continue;
            } else {
                break;
            }
        }
        let insert_idx = last_bullet_idx.map(|i| i + 1).unwrap_or(hi + 1);
        let mut new_lines: Vec<String> = lines[..insert_idx]
            .iter()
            .map(|s| s.to_string())
            .collect();
        for b in &bullets {
            new_lines.push(b.clone());
        }
        for l in &lines[insert_idx..] {
            new_lines.push(l.to_string());
        }
        new_lines.join("\n")
    } else {
        let base = key_accomplishments.trim_end();
        let block = format!("{}\n{}", COMPLETED_TASKS_HEADING, bullets.join("\n"));
        if base.is_empty() {
            block
        } else {
            format!("{}\n\n{}", base, block)
        }
    };

    MergeCompletedResult {
        new_key_accomplishments: new_body,
        imported,
        skipped,
    }
}

/// True if the (trimmed-leading) line reads as a Markdown ATX heading:
/// 1-6 leading `#` characters followed by either a space or end-of-line.
/// A bare `#Foo` (no space) is NOT a heading.
fn is_markdown_heading_line(trimmed: &str) -> bool {
    if !trimmed.starts_with('#') {
        return false;
    }
    let hashes = trimmed.chars().take_while(|c| *c == '#').count();
    if hashes == 0 || hashes > 6 {
        return false;
    }
    let rest = &trimmed[hashes..];
    rest.is_empty() || rest.starts_with(' ')
}

// ---------------------------------------------------------------------------
// Tasks-body manipulation (Slice 6a — ### Tasks section)
// ---------------------------------------------------------------------------

/// Split-and-classify view of a `tasks_body` string. Both lists
/// preserve insertion order. Non-task lines (anchor comments,
/// blank lines, stray text) are discarded — the two lists ARE the
/// truth about what tasks exist and their state.
struct TasksBodyBreakdown {
    incomplete_lines: Vec<String>,
    completed_lines: Vec<String>,
}

/// Parse a `tasks_body` (the body of the `### Tasks` section from
/// `parse_weekly_summary`) into its incomplete + completed task
/// lines. State is derived from each line's checkbox character
/// (`[ ]` vs `[x]`/`[X]`) — anchor comments are not required for a
/// correct read. This is the tamper-robustness property: even if a
/// user deletes both anchors, we still know which tasks are done
/// and which aren't.
///
/// Preserves the raw line text (case, punctuation, indentation) so
/// a round-trip through [`render_tasks_body`] can emit exactly the
/// same content aside from anchor and ordering normalization.
fn parse_tasks_body(tasks_body: &str) -> TasksBodyBreakdown {
    let mut incomplete = Vec::new();
    let mut completed = Vec::new();
    for line in tasks_body.split('\n') {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed.strip_prefix("- [") else {
            continue;
        };
        let mut chars = rest.chars();
        let Some(marker) = chars.next() else { continue };
        if chars.next() != Some(']') {
            continue;
        }
        match marker {
            ' ' => incomplete.push(line.to_string()),
            'x' | 'X' => completed.push(line.to_string()),
            _ => {}
        }
    }
    TasksBodyBreakdown {
        incomplete_lines: incomplete,
        completed_lines: completed,
    }
}

/// Re-emit a `tasks_body` from a breakdown. Always includes both
/// anchor comments in the canonical position, so callers don't have
/// to worry about anchor reconstruction. Idempotent:
/// `render_tasks_body(parse_tasks_body(x))` yields a canonical
/// version of `x`.
fn render_tasks_body(bd: &TasksBodyBreakdown) -> String {
    let mut out = String::new();
    out.push_str(TASKS_ANCHOR_INCOMPLETE);
    for line in &bd.incomplete_lines {
        out.push('\n');
        out.push_str(line);
    }
    out.push('\n');
    out.push_str(TASKS_ANCHOR_COMPLETED);
    for line in &bd.completed_lines {
        out.push('\n');
        out.push_str(line);
    }
    out
}

/// Append a new open task to the end of the Incomplete subsection.
/// Validation: empty text / embedded newlines / pre-existing `- [`
/// prefix / text longer than [`MAX_TASK_TEXT_LEN`] all bounce with
/// user-facing error strings.
///
/// The Incomplete subsection is deterministically the "top" bucket
/// in the rendered body — no ordering ambiguity between callers.
pub fn append_task_to_tasks_body(
    tasks_body: &str,
    text: &str,
) -> Result<String, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("Task text can't be empty.".to_string());
    }
    if trimmed.contains('\n') || trimmed.contains('\r') {
        return Err("Task text can't span multiple lines.".to_string());
    }
    if trimmed.len() > MAX_TASK_TEXT_LEN {
        return Err(format!(
            "Task text is too long (max {MAX_TASK_TEXT_LEN} bytes)."
        ));
    }
    if trimmed.starts_with("- [") {
        return Err(
            "Don't include the `- [ ]` prefix — we add it for you.".to_string(),
        );
    }

    let mut bd = parse_tasks_body(tasks_body);
    bd.incomplete_lines.push(format!("- [ ] {trimmed}"));
    Ok(render_tasks_body(&bd))
}

/// Move a task between the Incomplete and Completed subsections of
/// a `tasks_body`, flipping its checkbox character in the process.
/// Identity model: composite `(text_hash, ordinal)` key. Slice 6a's
/// move-on-check semantics — a checked task belongs in Completed;
/// unchecking moves it back to Incomplete.
///
/// The moved task lands at the END of its target list (newest-at-
/// bottom convention matches how `append_task_to_tasks_body` inserts).
///
/// Returns `(new_tasks_body, new_is_completed)`. `new_is_completed`
/// is the state *after* the toggle — callers write it to the
/// completion sidecar. Note: because moving a task changes its
/// position, its `ordinal` after the move may differ (if there are
/// duplicates with the same normalized text). Callers that update
/// the sidecar with a per-task key should re-parse the returned
/// body to compute the new ordinal.
pub fn toggle_task_in_tasks_body(
    tasks_body: &str,
    text_hash: &str,
    ordinal: u32,
) -> Result<(String, bool), String> {
    // Locate the target task via the existing (hash, ordinal)
    // identity model. Parsing the full body once gives us the
    // task's current state (which determines which list it lives in
    // and how to flip it).
    let all_tasks = parse_plans_tasks(tasks_body);
    let target = all_tasks
        .iter()
        .find(|t| t.text_hash == text_hash && t.ordinal == ordinal)
        .ok_or_else(|| {
            eprintln!(
                "[toggle] task not found in tasks_body (hash={text_hash}, ordinal={ordinal})"
            );
            "That task couldn't be found in your weekly file — it may have been edited or removed since this list loaded."
                .to_string()
        })?;

    let mut bd = parse_tasks_body(tasks_body);
    let source_lines = if target.is_completed {
        &mut bd.completed_lines
    } else {
        &mut bd.incomplete_lines
    };

    // Find the source line index by scanning for the ONE that
    // matches this task's (hash, position-within-state) — we can
    // reuse parse_plans_tasks on the source list to compute
    // ordinal-within-state, then match on that.
    let source_joined = source_lines.join("\n");
    let source_tasks = parse_plans_tasks(&source_joined);
    let within_state_ordinal = source_tasks
        .iter()
        .enumerate()
        .find(|(_, t)| t.text_hash == text_hash)
        .map(|(idx_of_first, _)| {
            // `ordinal` in the OUTER parse is offset by however many
            // same-hash tasks came before this task's state bucket.
            // Compute: rank of this task among same-hash tasks in the
            // source-state list.
            all_tasks
                .iter()
                .take_while(|t| !(t.text_hash == text_hash && t.ordinal == ordinal))
                .filter(|t| t.text_hash == text_hash && t.is_completed == target.is_completed)
                .count()
                + idx_of_first * 0 // idx_of_first not used — keep for clarity
        });
    let rank_in_source = within_state_ordinal.ok_or_else(|| {
        "internal error: task not found in its own state bucket after parse".to_string()
    })?;
    // Walk source_lines and pick out the `rank_in_source`-th line
    // whose text hashes to `text_hash`.
    let mut match_count: usize = 0;
    let mut source_idx: Option<usize> = None;
    for (i, line) in source_lines.iter().enumerate() {
        let line_hash = task_line_text_hash(line);
        if line_hash.as_deref() == Some(text_hash) {
            if match_count == rank_in_source {
                source_idx = Some(i);
                break;
            }
            match_count += 1;
        }
    }
    let source_idx = source_idx.ok_or_else(|| {
        "internal error: source line index not resolvable from ordinal".to_string()
    })?;
    let moved_line = source_lines.remove(source_idx);

    // Flip the checkbox character in the extracted line. We know the
    // marker is at position "- [" + 3 (past the '['), and it's
    // ASCII, so the byte splice is UTF-8 safe.
    let flipped_line = flip_checkbox_in_line(&moved_line).ok_or_else(|| {
        "internal error: extracted line lost its checkbox marker".to_string()
    })?;

    // Append to the target list (opposite state).
    if target.is_completed {
        bd.incomplete_lines.push(flipped_line);
    } else {
        bd.completed_lines.push(flipped_line);
    }
    let new_is_completed = !target.is_completed;

    Ok((render_tasks_body(&bd), new_is_completed))
}

/// Outcome of a successful `edit_task_in_tasks_body` call. `new_ordinal`
/// is the task's per-hash file-position rank AFTER the rename — the
/// command layer needs it to re-key sidecar and provenance entries
/// when the normalized text (and thus the hash) actually changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditTaskOutcome {
    pub new_body: String,
    pub new_text_hash: String,
    pub new_ordinal: u32,
    pub is_completed: bool,
}

/// Rename a task in `tasks_body` in-place. Located via the existing
/// `(text_hash, ordinal)` identity, the task line's prefix
/// (leading whitespace + `- [ ]` / `- [x]`) is preserved verbatim,
/// only the text portion is swapped.
///
/// Rejects the same inputs as `append_task_to_tasks_body`: empty
/// text, embedded newlines, oversize, and text starting with `- [`.
/// Case-preserving: an uppercase-X marker stays uppercase.
pub fn edit_task_in_tasks_body(
    tasks_body: &str,
    text_hash: &str,
    ordinal: u32,
    new_text: &str,
) -> Result<EditTaskOutcome, String> {
    let trimmed = new_text.trim();
    if trimmed.is_empty() {
        return Err("Task text can't be empty.".to_string());
    }
    if trimmed.contains('\n') || trimmed.contains('\r') {
        return Err("Task text can't span multiple lines.".to_string());
    }
    if trimmed.len() > MAX_TASK_TEXT_LEN {
        return Err(format!(
            "Task text is too long (max {MAX_TASK_TEXT_LEN} bytes)."
        ));
    }
    if trimmed.starts_with("- [") {
        return Err(
            "Don't include the `- [ ]` prefix — we add it for you.".to_string(),
        );
    }

    let all_tasks = parse_plans_tasks(tasks_body);
    let target = all_tasks
        .iter()
        .find(|t| t.text_hash == text_hash && t.ordinal == ordinal)
        .ok_or_else(|| {
            eprintln!(
                "[edit] task not found in tasks_body (hash={text_hash}, ordinal={ordinal})"
            );
            "That task couldn't be found in your weekly file — it may have been edited or removed since this list loaded."
                .to_string()
        })?;

    let mut bd = parse_tasks_body(tasks_body);
    let source_lines = if target.is_completed {
        &mut bd.completed_lines
    } else {
        &mut bd.incomplete_lines
    };

    // Same rank-within-state math as toggle_task_in_tasks_body —
    // count how many same-hash tasks in the source state come before
    // the target in the outer parse.
    let rank_in_source: usize = all_tasks
        .iter()
        .take_while(|t| !(t.text_hash == text_hash && t.ordinal == ordinal))
        .filter(|t| t.text_hash == text_hash && t.is_completed == target.is_completed)
        .count();

    let mut match_count: usize = 0;
    let mut source_idx: Option<usize> = None;
    for (i, line) in source_lines.iter().enumerate() {
        let line_hash = task_line_text_hash(line);
        if line_hash.as_deref() == Some(text_hash) {
            if match_count == rank_in_source {
                source_idx = Some(i);
                break;
            }
            match_count += 1;
        }
    }
    let source_idx = source_idx.ok_or_else(|| {
        "internal error: source line index not resolvable from ordinal".to_string()
    })?;

    // Preserve the exact leading whitespace + "- [X]" + one-space
    // separator by locating where the text starts. Everything before
    // that offset stays byte-identical; only the text tail is swapped.
    let original_line = &source_lines[source_idx];
    let (_, text_start) = match_task_marker(original_line).ok_or_else(|| {
        "internal error: extracted line lost its checkbox marker".to_string()
    })?;
    let prefix = &original_line[..text_start];
    let new_line = format!("{prefix}{trimmed}");
    source_lines[source_idx] = new_line;

    let new_hash = hash_task_text(&normalize_task_text(trimmed));
    let new_body = render_tasks_body(&bd);

    // Compute the renamed task's new outer ordinal in the fresh body.
    // The rename doesn't reorder any lines, so we can identify our
    // renamed task by its position within its state sub-list
    // (source_idx) and pull its ordinal from the re-parsed outer view.
    let new_ordinal = parse_plans_tasks(&new_body)
        .into_iter()
        .filter(|t| t.is_completed == target.is_completed)
        .nth(source_idx)
        .map(|t| t.ordinal)
        .ok_or_else(|| {
            "internal error: renamed task not found in fresh parse".to_string()
        })?;

    Ok(EditTaskOutcome {
        new_body,
        new_text_hash: new_hash,
        new_ordinal,
        is_completed: target.is_completed,
    })
}

/// Remove a task line from `tasks_body`. Located via the existing
/// `(text_hash, ordinal)` identity. The line is dropped verbatim
/// (including its leading whitespace + checkbox marker); everything
/// else in the body — anchor comments, other tasks, prose between
/// anchors — stays byte-identical. Returns the new tasks_body.
///
/// Callers on the command layer must ALSO drop the deleted task's
/// entries from the completion sidecar + rollover-log provenance
/// (via a positional re-key pass) — see `delete_task_impl` for the
/// bookkeeping shape.
pub fn delete_task_from_tasks_body(
    tasks_body: &str,
    text_hash: &str,
    ordinal: u32,
) -> Result<String, String> {
    let all_tasks = parse_plans_tasks(tasks_body);
    let target = all_tasks
        .iter()
        .find(|t| t.text_hash == text_hash && t.ordinal == ordinal)
        .ok_or_else(|| {
            eprintln!(
                "[delete] task not found in tasks_body (hash={text_hash}, ordinal={ordinal})"
            );
            "That task couldn't be found in your weekly file — it may have been edited or removed since this list loaded."
                .to_string()
        })?;

    let mut bd = parse_tasks_body(tasks_body);
    let source_lines = if target.is_completed {
        &mut bd.completed_lines
    } else {
        &mut bd.incomplete_lines
    };

    let rank_in_source: usize = all_tasks
        .iter()
        .take_while(|t| !(t.text_hash == text_hash && t.ordinal == ordinal))
        .filter(|t| t.text_hash == text_hash && t.is_completed == target.is_completed)
        .count();

    let mut match_count: usize = 0;
    let mut source_idx: Option<usize> = None;
    for (i, line) in source_lines.iter().enumerate() {
        let line_hash = task_line_text_hash(line);
        if line_hash.as_deref() == Some(text_hash) {
            if match_count == rank_in_source {
                source_idx = Some(i);
                break;
            }
            match_count += 1;
        }
    }
    let source_idx = source_idx.ok_or_else(|| {
        "internal error: source line index not resolvable from ordinal".to_string()
    })?;
    source_lines.remove(source_idx);

    Ok(render_tasks_body(&bd))
}

/// Compute the same normalized text-hash the identity model uses,
/// for a raw task line. Returns None if the line isn't a valid
/// task line.
fn task_line_text_hash(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix("- [")?;
    let mut chars = rest.chars();
    let marker = chars.next()?;
    if !matches!(marker, ' ' | 'x' | 'X') {
        return None;
    }
    if chars.next()? != ']' {
        return None;
    }
    // Skip closing bracket + one whitespace.
    let after_bracket = &rest[2..];
    let text_start = after_bracket
        .chars()
        .next()
        .filter(|c| c.is_whitespace())
        .map(|_| after_bracket.chars().next().unwrap().len_utf8())
        .unwrap_or(0);
    let text = &after_bracket[text_start..];
    let normalized = normalize_task_text(text);
    if normalized.is_empty() {
        return None;
    }
    Some(hash_task_text(&normalized))
}

/// Flip `[ ]` → `[x]` or `[x]`/`[X]` → `[ ]` in a task line, leaving
/// everything else untouched.
fn flip_checkbox_in_line(line: &str) -> Option<String> {
    let leading_ws_len = line.len() - line.trim_start().len();
    let after_ws = &line[leading_ws_len..];
    let after_dash = after_ws.strip_prefix("- [")?;
    let mut chars = after_dash.chars();
    let marker = chars.next()?;
    let flipped = match marker {
        ' ' => 'x',
        'x' | 'X' => ' ',
        _ => return None,
    };
    // Splice the single ASCII byte cleanly.
    let marker_offset = leading_ws_len + 3;
    let mut out = String::with_capacity(line.len());
    out.push_str(&line[..marker_offset]);
    out.push(flipped);
    out.push_str(&line[marker_offset + 1..]);
    Some(out)
}

// ---------------------------------------------------------------------------
// Inline-only markdown rendering for task display
// ---------------------------------------------------------------------------

/// Render a task's text as inline-only HTML suitable for the read-only
/// task-list UI. Task lines like `- [ ] **Ship** the ~~old~~ new thing`
/// were previously shown verbatim (literal asterisks + tildes) because
/// the frontend was rendering `{t.text}`. This helper produces the
/// HTML the UI uses via `{@html …}` after sanitization.
///
/// Pipeline:
///
/// 1. pulldown-cmark with `ENABLE_STRIKETHROUGH` — bold, italic, strike,
///    inline code. `ENABLE_TABLES` and `ENABLE_SMART_PUNCTUATION` are
///    deliberately off (block markup and auto-quote-swap are both
///    wrong for a one-line context). pulldown-cmark passes raw HTML
///    through by default; step 2 is what neutralizes it.
/// 2. ammonia with a tight inline-only allowlist. `<script>`,
///    `<img>`, event handlers, and `javascript:` / `data:` URLs are
///    all stripped by ammonia's default posture — but we narrow the
///    tag list to `strong, em, del, code, br` so nothing block-level
///    (headings, lists, blockquotes, tables) survives even if the user
///    somehow embeds it.
/// 3. Peel the outer `<p>…</p>` wrapper. pulldown-cmark always emits
///    one for top-level prose; we're rendering into a `<span>` so a
///    block `<p>` would break layout.
///
/// Links (`[label](url)`) are intentionally NOT rendered in Slice 1 —
/// ammonia strips the `<a>` tag but keeps the label text. If users
/// ask for clickable links later, add `a` to the allowlist plus
/// `.link_rel(Some("noopener noreferrer"))` and `.url_schemes(…)`.
///
/// The output is safe to pass to Svelte's `{@html}`: everything is
/// sanitized before it crosses the IPC boundary.
pub fn render_task_text_inline(text: &str) -> String {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_STRIKETHROUGH);

    let mut raw = String::with_capacity(text.len() * 2);
    html::push_html(&mut raw, Parser::new_ext(text, opts));

    let cleaned = ammonia::Builder::default()
        .tags(std::collections::HashSet::from([
            "strong", "em", "del", "code", "br",
        ]))
        // No URL schemes needed — `<a>` isn't in the allowlist. Left
        // explicit so a future change adding `a` doesn't inherit
        // ammonia's default url_schemes surface by accident.
        .url_schemes(std::collections::HashSet::new())
        .clean(&raw)
        .to_string();

    strip_paragraph_wrapper(&cleaned)
}

/// pulldown-cmark wraps top-level prose in `<p>…</p>`. We render into
/// an inline `<span>`, so the wrapper has to go. Only strips a single
/// matched outer wrapper; multi-paragraph input (rare — task text is a
/// single line by construction) is returned as-is so paragraph breaks
/// don't silently disappear.
fn strip_paragraph_wrapper(html: &str) -> String {
    let trimmed = html.trim();
    if let Some(rest) = trimmed.strip_prefix("<p>") {
        if let Some(inner) = rest.strip_suffix("</p>") {
            // Guard against multi-paragraph input: if the inner still
            // contains `</p>`, we'd be joining two paragraphs. Leave
            // the string untouched in that case.
            if !inner.contains("</p>") {
                return inner.to_string();
            }
        }
    }
    trimmed.to_string()
}

// ---------------------------------------------------------------------------
// Sidecar
// ---------------------------------------------------------------------------

/// One completion record in the sidecar. Timestamps are ISO 8601 with
/// offset (e.g. `"2026-07-07T14:23:00-04:00"`); values may be
/// approximations backfilled from the source file's mtime when the
/// user checks tasks externally.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TaskCompletion {
    pub year: u32,
    pub week: u32,
    pub text_hash: String,
    pub ordinal: u32,
    pub completed_at: String,
}

/// Persistent index of task completions across all weeks. Read on
/// demand; not held in RwLock state (unlike labels) because it's
/// small and rarely contended.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskCompletions {
    pub version: u32,
    pub completions: Vec<TaskCompletion>,
}

impl Default for TaskCompletions {
    fn default() -> Self {
        Self {
            version: CURRENT_TASK_COMPLETIONS_VERSION,
            completions: Vec::new(),
        }
    }
}

impl TaskCompletions {
    /// Load the sidecar. Missing file → empty. Corrupt file → empty +
    /// stderr warning (same posture as `LabelIndex::load`; losing
    /// completion timestamps is preferable to blocking the UI).
    pub async fn load<B: StorageBackend + ?Sized>(backend: &B) -> StorageResult<Self> {
        match backend.read_metadata(TASK_COMPLETIONS_FILE).await? {
            Some(content) => match serde_json::from_str::<TaskCompletions>(&content) {
                Ok(idx) => Ok(idx),
                Err(e) => {
                    eprintln!(
                        "task-completions.json failed to parse ({}). Starting with an empty index.",
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
        backend.write_metadata(TASK_COMPLETIONS_FILE, &content).await
    }

    pub fn find(
        &self,
        year: u32,
        week: u32,
        text_hash: &str,
        ordinal: u32,
    ) -> Option<&TaskCompletion> {
        self.completions
            .iter()
            .find(|c| c.year == year && c.week == week && c.text_hash == text_hash && c.ordinal == ordinal)
    }
}

// ---------------------------------------------------------------------------
// Rollover
// ---------------------------------------------------------------------------

/// Compact `(year, week)` pair used as the idempotency key on
/// `RolloverLog`. Serialized in camelCase to match the frontend
/// `YearWeek` type.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct YearWeekKey {
    pub year: u32,
    pub week: u32,
}

/// Provenance record for a single task. One entry per (year, week,
/// text_hash, ordinal) tuple currently living in the journal. Two
/// entries with the same `original_*` fields but different
/// `(year, week)` describe the same task copied forward through
/// rollover.
///
/// `original_created_at` is a best-effort RFC 3339 timestamp — set
/// precisely for tasks born via `append_task_to_current_week`, and
/// backfilled from file mtime for tasks predating the provenance
/// system.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TaskProvenance {
    pub year: u32,
    pub week: u32,
    pub text_hash: String,
    pub ordinal: u32,
    pub original_year: u32,
    pub original_week: u32,
    pub original_created_at: String,
}

/// The `.metadata/rollover-log.json` sidecar. Tracks two things:
///
/// 1. **Idempotency** — `last_run_to_week` records the target week of
///    the most recent successful rollover. `check_and_apply_rollover`
///    no-ops when it matches the current week, so repeated triggers
///    from focus / visibility events never double-copy tasks.
/// 2. **Provenance** — one entry per live task-instance. Preserved on
///    each rollover copy so a task can trace back to its `original_*`
///    week even after being pulled forward through multiple weeks.
///    Chris's ask: paper trail for a future "time to resolution" stat.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RolloverLog {
    pub version: u32,
    #[serde(default)]
    pub last_run_to_week: Option<YearWeekKey>,
    #[serde(default)]
    pub last_run_at: Option<String>,
    #[serde(default)]
    pub provenance: Vec<TaskProvenance>,
}

impl Default for RolloverLog {
    fn default() -> Self {
        Self {
            version: CURRENT_ROLLOVER_LOG_VERSION,
            last_run_to_week: None,
            last_run_at: None,
            provenance: Vec::new(),
        }
    }
}

impl RolloverLog {
    /// Load the sidecar. Missing file → empty. Corrupt file → empty +
    /// stderr warning — matches `TaskCompletions::load` posture.
    /// Losing the rollover log means the next rollover treats the
    /// world as "never rolled over" and re-runs; provenance for
    /// existing tasks is lost, but the source-of-truth (markdown) is
    /// unchanged.
    pub async fn load<B: StorageBackend + ?Sized>(backend: &B) -> StorageResult<Self> {
        match backend.read_metadata(ROLLOVER_LOG_FILE).await? {
            Some(content) => match serde_json::from_str::<RolloverLog>(&content) {
                Ok(log) => Ok(log),
                Err(e) => {
                    eprintln!(
                        "rollover-log.json failed to parse ({}). Starting with an empty log.",
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
        backend.write_metadata(ROLLOVER_LOG_FILE, &content).await
    }

    /// O(n) lookup for the provenance entry matching a task's
    /// identity. Returns `None` for tasks predating the provenance
    /// system.
    pub fn find(
        &self,
        year: u32,
        week: u32,
        text_hash: &str,
        ordinal: u32,
    ) -> Option<&TaskProvenance> {
        self.provenance.iter().find(|p| {
            p.year == year
                && p.week == week
                && p.text_hash == text_hash
                && p.ordinal == ordinal
        })
    }

    /// Insert or overwrite a provenance entry keyed by
    /// `(year, week, text_hash, ordinal)`. Overwrite semantics mean a
    /// duplicate append (via retry) doesn't create ghost rows.
    pub fn upsert(&mut self, entry: TaskProvenance) {
        if let Some(existing) = self.provenance.iter_mut().find(|p| {
            p.year == entry.year
                && p.week == entry.week
                && p.text_hash == entry.text_hash
                && p.ordinal == entry.ordinal
        }) {
            *existing = entry;
        } else {
            self.provenance.push(entry);
        }
    }
}

// ---------------------------------------------------------------------------
// Auto-import log
// ---------------------------------------------------------------------------

/// Slice 6c-followup — one-line sidecar tracking when the automated
/// "import completed tasks" workflow last fired. The value is a
/// local YYYY-MM-DD string; the trigger-event handler compares
/// today's local date and no-ops if they match, so we run the
/// import exactly once per local day regardless of how many
/// mount/focus events the app sees.
///
/// Missing file → treated as "never run". Corrupt file → same, with
/// a stderr warning (matches the RolloverLog + TaskCompletions
/// posture; losing the log just means the next trigger re-runs).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AutoImportLog {
    #[serde(default = "AutoImportLog::current_version")]
    pub version: u32,
    /// Last local date the import successfully ran, as YYYY-MM-DD.
    /// `None` when never run.
    #[serde(default)]
    pub last_import_date: Option<String>,
    /// Wall-clock RFC-3339 of the last run. Diagnostic only —
    /// today-vs-yesterday gating uses `last_import_date`.
    #[serde(default)]
    pub last_import_at: Option<String>,
}

impl AutoImportLog {
    fn current_version() -> u32 {
        1
    }

    pub async fn load<B: StorageBackend + ?Sized>(backend: &B) -> StorageResult<Self> {
        match backend.read_metadata(AUTO_IMPORT_LOG_FILE).await? {
            Some(content) => match serde_json::from_str::<AutoImportLog>(&content) {
                Ok(log) => Ok(log),
                Err(e) => {
                    eprintln!(
                        "auto-import-log.json failed to parse ({}). Starting with an empty log.",
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
        backend.write_metadata(AUTO_IMPORT_LOG_FILE, &content).await
    }
}

// ---------------------------------------------------------------------------
// Task due dates (Phase 3e)
// ---------------------------------------------------------------------------

/// One task's optional due date. Composite identity
/// `(year, week, text_hash, ordinal)` matches every other task-
/// scoped sidecar in the app. `due_date` is a LOCAL YYYY-MM-DD
/// string — no time-of-day, no timezone, no ISO-8601 flavor. Users
/// pick calendar days; time-of-day for reminders lives on
/// `TaskReminderSettings`, not per task.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TaskDueDate {
    pub year: u32,
    pub week: u32,
    pub text_hash: String,
    pub ordinal: u32,
    /// Local calendar date, formatted `YYYY-MM-DD`.
    pub due_date: String,
}

/// Sidecar mapping tasks → due dates. Same load/save posture as
/// `TaskCompletions` + `RolloverLog`: missing file → empty, corrupt
/// file → empty + stderr warning, atomic write via `write_metadata`.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TaskDueDates {
    #[serde(default = "TaskDueDates::current_version")]
    pub version: u32,
    #[serde(default)]
    pub due_dates: Vec<TaskDueDate>,
}

impl TaskDueDates {
    fn current_version() -> u32 {
        CURRENT_TASK_DUE_DATES_VERSION
    }

    pub async fn load<B: StorageBackend + ?Sized>(backend: &B) -> StorageResult<Self> {
        match backend.read_metadata(TASK_DUE_DATES_FILE).await? {
            Some(content) => match serde_json::from_str::<TaskDueDates>(&content) {
                Ok(dd) => Ok(dd),
                Err(e) => {
                    eprintln!(
                        "task-due-dates.json failed to parse ({}). Starting with an empty log.",
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
        backend.write_metadata(TASK_DUE_DATES_FILE, &content).await
    }

    /// O(n) lookup for a task's due-date entry.
    pub fn find(
        &self,
        year: u32,
        week: u32,
        text_hash: &str,
        ordinal: u32,
    ) -> Option<&TaskDueDate> {
        self.due_dates.iter().find(|d| {
            d.year == year && d.week == week && d.text_hash == text_hash && d.ordinal == ordinal
        })
    }

    /// Insert or overwrite the entry keyed by
    /// `(year, week, text_hash, ordinal)`. Overwrite semantics let
    /// callers seed a fresh date without a preceding remove.
    pub fn upsert(&mut self, entry: TaskDueDate) {
        if let Some(existing) = self.due_dates.iter_mut().find(|d| {
            d.year == entry.year
                && d.week == entry.week
                && d.text_hash == entry.text_hash
                && d.ordinal == entry.ordinal
        }) {
            *existing = entry;
        } else {
            self.due_dates.push(entry);
        }
    }

    /// Remove the entry keyed by `(year, week, text_hash, ordinal)`.
    /// No-op when no matching entry exists.
    pub fn remove(&mut self, year: u32, week: u32, text_hash: &str, ordinal: u32) {
        self.due_dates.retain(|d| {
            !(d.year == year
                && d.week == week
                && d.text_hash == text_hash
                && d.ordinal == ordinal)
        });
    }
}

/// Validate a due-date string sent from the frontend. Accepts the
/// exact `YYYY-MM-DD` local-calendar shape and rejects anything else
/// (empty, whitespace, ISO-8601-with-time, wrong separator, out-of-
/// range month/day). Returns Ok(canonical) on success — the parsed
/// value re-serialized so callers write a normalized form.
pub fn validate_due_date_input(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("Due date can't be empty.".to_string());
    }
    // NaiveDate::parse_from_str with `%Y-%m-%d` accepts leading
    // zeros AND rejects malformed month/day values (e.g. 2026-13-01).
    match chrono::NaiveDate::parse_from_str(trimmed, "%Y-%m-%d") {
        Ok(nd) => Ok(nd.format("%Y-%m-%d").to_string()),
        Err(_) => Err(format!(
            "Due date must look like YYYY-MM-DD (got {trimmed:?})."
        )),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty() {
        assert!(parse_plans_tasks("").is_empty());
        assert!(parse_plans_tasks("   \n  \n").is_empty());
    }

    #[test]
    fn parse_ignores_non_task_bullets() {
        let input = "- just a bullet\n- another one\n";
        assert!(parse_plans_tasks(input).is_empty());
    }

    #[test]
    fn parse_single_open_task() {
        let tasks = parse_plans_tasks("- [ ] Write the doc\n");
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].text, "Write the doc");
        assert!(!tasks[0].is_completed);
        assert_eq!(tasks[0].ordinal, 0);
        assert_eq!(tasks[0].byte_offset_in_plans, 0);
    }

    #[test]
    fn parse_single_completed_task() {
        let tasks = parse_plans_tasks("- [x] Ship it\n");
        assert_eq!(tasks.len(), 1);
        assert!(tasks[0].is_completed);
        assert_eq!(tasks[0].text, "Ship it");
    }

    #[test]
    fn parse_uppercase_x_marks_completed() {
        let tasks = parse_plans_tasks("- [X] Also done\n");
        assert_eq!(tasks.len(), 1);
        assert!(tasks[0].is_completed);
    }

    #[test]
    fn parse_mixed_states() {
        let input = "\
- [ ] First
- [x] Second
- [ ] Third
";
        let tasks = parse_plans_tasks(input);
        assert_eq!(tasks.len(), 3);
        assert!(!tasks[0].is_completed);
        assert!(tasks[1].is_completed);
        assert!(!tasks[2].is_completed);
        assert_eq!(tasks[0].text, "First");
        assert_eq!(tasks[1].text, "Second");
        assert_eq!(tasks[2].text, "Third");
    }

    #[test]
    fn parse_nested_tasks_surface_flat() {
        let input = "\
- [ ] Top level
  - [ ] Nested one
  - [x] Nested done
";
        let tasks = parse_plans_tasks(input);
        assert_eq!(tasks.len(), 3);
        assert_eq!(tasks[0].text, "Top level");
        assert_eq!(tasks[1].text, "Nested one");
        assert_eq!(tasks[2].text, "Nested done");
        assert!(tasks[2].is_completed);
    }

    #[test]
    fn parse_duplicate_text_gets_ordinals() {
        let input = "\
- [ ] Same
- [ ] Same
- [x] Same
";
        let tasks = parse_plans_tasks(input);
        assert_eq!(tasks.len(), 3);
        assert_eq!(tasks[0].ordinal, 0);
        assert_eq!(tasks[1].ordinal, 1);
        assert_eq!(tasks[2].ordinal, 2);
        // All three share a hash — that's the whole point of the ordinal.
        assert_eq!(tasks[0].text_hash, tasks[1].text_hash);
        assert_eq!(tasks[1].text_hash, tasks[2].text_hash);
    }

    #[test]
    fn parse_normalization_treats_case_and_punctuation_as_same() {
        let input = "\
- [ ] Do the thing
- [ ] DO the THING!
- [ ] do the thing.
";
        let tasks = parse_plans_tasks(input);
        assert_eq!(tasks.len(), 3);
        // All three normalize to the same hash.
        assert_eq!(tasks[0].text_hash, tasks[1].text_hash);
        assert_eq!(tasks[1].text_hash, tasks[2].text_hash);
        assert_eq!(tasks[0].ordinal, 0);
        assert_eq!(tasks[1].ordinal, 1);
        assert_eq!(tasks[2].ordinal, 2);
        // Display text is preserved as written.
        assert_eq!(tasks[0].text, "Do the thing");
        assert_eq!(tasks[1].text, "DO the THING!");
        assert_eq!(tasks[2].text, "do the thing.");
    }

    #[test]
    fn parse_empty_task_text_is_skipped() {
        let tasks = parse_plans_tasks("- [ ]\n- [ ]   \n");
        assert!(tasks.is_empty());
    }

    #[test]
    fn parse_records_offsets_for_splice() {
        let input = "- [ ] one\n- [x] two\n";
        let tasks = parse_plans_tasks(input);
        assert_eq!(tasks[0].byte_offset_in_plans, 0);
        assert_eq!(tasks[0].line_length, "- [ ] one".len());
        assert_eq!(tasks[1].byte_offset_in_plans, "- [ ] one\n".len());
        assert_eq!(tasks[1].line_length, "- [x] two".len());
        // The bracket position should be reachable via the offsets.
        assert_eq!(&input[tasks[1].byte_offset_in_plans + 3..tasks[1].byte_offset_in_plans + 4], "x");
    }

    #[test]
    fn parse_rejects_malformed_markers() {
        assert!(parse_plans_tasks("- [] no marker char\n").is_empty());
        assert!(parse_plans_tasks("- [y] wrong marker\n").is_empty());
        assert!(parse_plans_tasks("* [ ] wrong bullet\n").is_empty());
        assert!(parse_plans_tasks("[ ] no bullet\n").is_empty());
    }

    #[test]
    fn normalize_collapses_whitespace() {
        assert_eq!(normalize_task_text("  hello   world  "), "hello world");
        assert_eq!(normalize_task_text("hello\tworld"), "hello world");
    }

    #[test]
    fn normalize_lowercases() {
        assert_eq!(normalize_task_text("Hello World"), "hello world");
    }

    #[test]
    fn normalize_strips_trailing_punctuation() {
        assert_eq!(normalize_task_text("done!"), "done");
        assert_eq!(normalize_task_text("done!!!"), "done");
        assert_eq!(normalize_task_text("done.?!,:;"), "done");
    }

    #[test]
    fn normalize_preserves_internal_punctuation() {
        assert_eq!(normalize_task_text("Well, do it."), "well, do it");
    }

    #[test]
    fn normalize_empty_stays_empty() {
        assert_eq!(normalize_task_text(""), "");
        assert_eq!(normalize_task_text("   "), "");
        assert_eq!(normalize_task_text("!!!"), "");
    }

    #[test]
    fn hash_is_deterministic_and_hex() {
        let a = hash_task_text("ship the thing");
        let b = hash_task_text("ship the thing");
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn hash_differs_by_input() {
        assert_ne!(hash_task_text("a"), hash_task_text("b"));
    }

    #[test]
    fn task_completions_default_is_current_version() {
        let empty = TaskCompletions::default();
        assert_eq!(empty.version, CURRENT_TASK_COMPLETIONS_VERSION);
        assert!(empty.completions.is_empty());
    }

    #[test]
    fn task_completion_serializes_camel_case() {
        let entry = TaskCompletion {
            year: 2026,
            week: 27,
            text_hash: "abc".to_string(),
            ordinal: 0,
            completed_at: "2026-07-07T14:00:00-04:00".to_string(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"textHash\""));
        assert!(json.contains("\"completedAt\""));
        assert!(!json.contains("\"text_hash\""));
    }

    fn hash_of(text: &str) -> String {
        hash_task_text(&normalize_task_text(text))
    }

    // ---------------------------------------------------------------
    // Slice 6a — tasks_body manipulation
    // ---------------------------------------------------------------

    #[test]
    fn parse_tasks_body_classifies_by_checkbox_state_not_anchors() {
        // Anchors present + tasks in "wrong" positions relative to
        // them. State comes from the checkbox character, not the
        // anchor. Tamper-robust behavior locked here.
        let body = format!(
            "{TASKS_ANCHOR_INCOMPLETE}\n\
             - [x] Actually done despite living under the incomplete anchor\n\
             - [ ] Actually open\n\
             {TASKS_ANCHOR_COMPLETED}\n\
             - [ ] Actually open despite living under the completed anchor\n\
             - [x] Actually done"
        );
        let bd = parse_tasks_body(&body);
        assert_eq!(bd.incomplete_lines.len(), 2);
        assert_eq!(bd.completed_lines.len(), 2);
        assert!(bd.incomplete_lines.iter().any(|l| l.contains("Actually open")));
        assert!(bd.incomplete_lines.iter().any(|l| l.contains("Actually open despite")));
        assert!(bd.completed_lines.iter().any(|l| l.contains("Actually done")));
    }

    #[test]
    fn parse_tasks_body_survives_missing_anchors() {
        // Robustness: user hand-edited the file and nuked the
        // anchors. Reads still work — anchors are write-side
        // infrastructure only.
        let body = "- [ ] First\n- [x] Second\n";
        let bd = parse_tasks_body(body);
        assert_eq!(bd.incomplete_lines, vec!["- [ ] First".to_string()]);
        assert_eq!(bd.completed_lines, vec!["- [x] Second".to_string()]);
    }

    #[test]
    fn render_tasks_body_always_emits_both_anchors_in_canonical_order() {
        // Even for an empty breakdown, both anchors appear. That's
        // the fresh-scaffold shape: writes can find their insertion
        // points on the first task action.
        let bd = TasksBodyBreakdown {
            incomplete_lines: Vec::new(),
            completed_lines: Vec::new(),
        };
        let out = render_tasks_body(&bd);
        assert!(out.contains(TASKS_ANCHOR_INCOMPLETE));
        assert!(out.contains(TASKS_ANCHOR_COMPLETED));
        // Incomplete comes before Completed.
        assert!(
            out.find(TASKS_ANCHOR_INCOMPLETE)
                < out.find(TASKS_ANCHOR_COMPLETED)
        );
    }

    #[test]
    fn render_tasks_body_roundtrip_is_stable_under_parse() {
        // After one render + parse cycle, we get back the same
        // breakdown. Prevents drift on repeated no-op writes.
        let bd1 = TasksBodyBreakdown {
            incomplete_lines: vec!["- [ ] one".to_string(), "- [ ] two".to_string()],
            completed_lines: vec!["- [x] done".to_string()],
        };
        let rendered = render_tasks_body(&bd1);
        let bd2 = parse_tasks_body(&rendered);
        assert_eq!(bd1.incomplete_lines, bd2.incomplete_lines);
        assert_eq!(bd1.completed_lines, bd2.completed_lines);
    }

    #[test]
    fn append_task_to_tasks_body_lands_in_incomplete_at_end() {
        let body = format!(
            "{TASKS_ANCHOR_INCOMPLETE}\n\
             - [ ] existing\n\
             {TASKS_ANCHOR_COMPLETED}\n\
             - [x] done"
        );
        let out = append_task_to_tasks_body(&body, "fresh task").unwrap();
        let bd = parse_tasks_body(&out);
        // Newest at the bottom of Incomplete.
        assert_eq!(
            bd.incomplete_lines,
            vec![
                "- [ ] existing".to_string(),
                "- [ ] fresh task".to_string()
            ]
        );
        // Completed untouched.
        assert_eq!(bd.completed_lines, vec!["- [x] done".to_string()]);
    }

    #[test]
    fn append_task_to_tasks_body_scaffolds_anchors_when_missing() {
        // Empty tasks_body → append still succeeds and produces a
        // well-formed body with both anchors.
        let out = append_task_to_tasks_body("", "first task").unwrap();
        assert!(out.contains(TASKS_ANCHOR_INCOMPLETE));
        assert!(out.contains(TASKS_ANCHOR_COMPLETED));
        assert!(out.contains("- [ ] first task"));
    }

    #[test]
    fn append_task_to_tasks_body_validation_shape() {
        // Rejects the same inputs users are surfaced errors for:
        // empty / all-whitespace / embedded newline / pre-prefixed.
        assert!(append_task_to_tasks_body("", "").is_err());
        assert!(append_task_to_tasks_body("", "  \n  ").is_err());
        assert!(append_task_to_tasks_body("", "line one\nline two").is_err());
        assert!(append_task_to_tasks_body("", "- [ ] pre-prefixed").is_err());
    }

    // ---------------------------------------------------------------
    // edit_task_in_tasks_body tests
    // ---------------------------------------------------------------

    #[test]
    fn edit_task_renames_incomplete_task_and_returns_new_hash() {
        let body = format!(
            "{TASKS_ANCHOR_INCOMPLETE}\n\
             - [ ] Original text\n\
             {TASKS_ANCHOR_COMPLETED}"
        );
        let old_hash = hash_task_text(&normalize_task_text("Original text"));
        let out = edit_task_in_tasks_body(&body, &old_hash, 0, "Renamed text").unwrap();
        assert!(!out.is_completed);
        let expected_new_hash = hash_task_text(&normalize_task_text("Renamed text"));
        assert_eq!(out.new_text_hash, expected_new_hash);
        assert_ne!(out.new_text_hash, old_hash);
        assert_eq!(out.new_ordinal, 0);
        let bd = parse_tasks_body(&out.new_body);
        assert_eq!(bd.incomplete_lines, vec!["- [ ] Renamed text".to_string()]);
        assert!(bd.completed_lines.is_empty());
    }

    #[test]
    fn edit_task_preserves_completed_marker_case() {
        // A file the user hand-typed with `[X]` should keep the
        // uppercase X after an edit — we're renaming, not re-checking.
        let body = format!(
            "{TASKS_ANCHOR_INCOMPLETE}\n\
             {TASKS_ANCHOR_COMPLETED}\n\
             - [X] Old"
        );
        let hash = hash_task_text(&normalize_task_text("Old"));
        let out = edit_task_in_tasks_body(&body, &hash, 0, "New").unwrap();
        assert!(out.is_completed);
        assert!(
            out.new_body.contains("- [X] New"),
            "must preserve uppercase X marker: {}",
            out.new_body
        );
    }

    #[test]
    fn edit_task_preserves_leading_whitespace_on_indented_line() {
        // Users can hand-edit `### Tasks` and drop in an indented
        // task line — an edit must not re-flow their formatting.
        // Raw string here so the Rust line-continuation quirk doesn't
        // strip the two leading spaces on the task line.
        let body = format!(
            "{}\n  - [ ] Indented\n{}",
            TASKS_ANCHOR_INCOMPLETE, TASKS_ANCHOR_COMPLETED,
        );
        let hash = hash_task_text(&normalize_task_text("Indented"));
        let out = edit_task_in_tasks_body(&body, &hash, 0, "Renamed").unwrap();
        assert!(
            out.new_body.contains("  - [ ] Renamed"),
            "leading whitespace must be preserved: {}",
            out.new_body
        );
    }

    #[test]
    fn edit_task_normalizes_new_text_for_hash_but_stores_visible_verbatim() {
        // Punctuation + case affect the visible line but not the
        // normalized hash — same rules as append/toggle. Verifies the
        // stored line uses the trimmed user input.
        let body = format!(
            "{TASKS_ANCHOR_INCOMPLETE}\n\
             - [ ] before\n\
             {TASKS_ANCHOR_COMPLETED}"
        );
        let old_hash = hash_task_text(&normalize_task_text("before"));
        let out = edit_task_in_tasks_body(&body, &old_hash, 0, "  AFTER!! ").unwrap();
        // Stored verbatim after `.trim()`.
        assert!(out.new_body.contains("- [ ] AFTER!!"), "visible: {}", out.new_body);
        // Hash uses normalized form ("after").
        assert_eq!(out.new_text_hash, hash_task_text(&normalize_task_text("after")));
    }

    #[test]
    fn edit_task_targets_correct_duplicate_via_ordinal() {
        // Three identical tasks. Editing ordinal=1 (middle) must
        // rename ONLY the middle one; ord=0 and ord=2 stay literal.
        let body = format!(
            "{TASKS_ANCHOR_INCOMPLETE}\n\
             - [ ] Standup\n\
             - [ ] Standup\n\
             - [ ] Standup\n\
             {TASKS_ANCHOR_COMPLETED}"
        );
        let hash = hash_task_text(&normalize_task_text("Standup"));
        let out = edit_task_in_tasks_body(&body, &hash, 1, "Middle standup").unwrap();
        let bd = parse_tasks_body(&out.new_body);
        assert_eq!(
            bd.incomplete_lines,
            vec![
                "- [ ] Standup".to_string(),
                "- [ ] Middle standup".to_string(),
                "- [ ] Standup".to_string(),
            ]
        );
        // Renamed task's new hash has no duplicate before it → ord=0.
        assert_eq!(out.new_ordinal, 0);
    }

    #[test]
    fn edit_task_errors_on_missing_hash() {
        let body = format!(
            "{TASKS_ANCHOR_INCOMPLETE}\n\
             - [ ] Only\n\
             {TASKS_ANCHOR_COMPLETED}"
        );
        let err =
            edit_task_in_tasks_body(&body, "not-a-real-hash", 0, "Renamed").unwrap_err();
        assert!(err.contains("couldn't be found"), "err: {err}");
    }

    #[test]
    fn edit_task_shares_validation_with_append() {
        let body = format!(
            "{TASKS_ANCHOR_INCOMPLETE}\n\
             - [ ] Task\n\
             {TASKS_ANCHOR_COMPLETED}"
        );
        let hash = hash_task_text(&normalize_task_text("Task"));
        assert!(edit_task_in_tasks_body(&body, &hash, 0, "").is_err());
        assert!(edit_task_in_tasks_body(&body, &hash, 0, "   ").is_err());
        assert!(edit_task_in_tasks_body(&body, &hash, 0, "one\ntwo").is_err());
        assert!(edit_task_in_tasks_body(&body, &hash, 0, "- [ ] prefixed").is_err());
    }

    // ---------------------------------------------------------------
    // merge_completed_tasks_into_key_accomplishments tests
    // ---------------------------------------------------------------

    #[test]
    fn merge_completed_empty_body_gets_heading_and_bullets() {
        let out = merge_completed_tasks_into_key_accomplishments(
            "",
            &vec!["Ship the widget".to_string(), "Fix the bug".to_string()],
        );
        assert_eq!(out.imported, 2);
        assert_eq!(out.skipped, 0);
        assert_eq!(
            out.new_key_accomplishments,
            "#### Completed Tasks\n- Ship the widget\n- Fix the bug"
        );
    }

    #[test]
    fn merge_completed_prose_only_appends_block_with_blank_separator() {
        let out = merge_completed_tasks_into_key_accomplishments(
            "Had a great sprint.",
            &vec!["Ship the widget".to_string()],
        );
        assert_eq!(out.imported, 1);
        assert_eq!(
            out.new_key_accomplishments,
            "Had a great sprint.\n\n#### Completed Tasks\n- Ship the widget"
        );
    }

    #[test]
    fn merge_completed_existing_heading_appends_under_existing_bullets() {
        // KEY behavior — the reason we're rewriting this. Second
        // import must not add a duplicate heading. New bullets go at
        // the END of the contiguous bullet block under the heading.
        let body = "#### Completed Tasks\n- Ship the widget\n- Fix the bug";
        let out = merge_completed_tasks_into_key_accomplishments(
            body,
            &vec!["Write the docs".to_string()],
        );
        assert_eq!(out.imported, 1);
        assert_eq!(
            out.new_key_accomplishments,
            "#### Completed Tasks\n- Ship the widget\n- Fix the bug\n- Write the docs"
        );
    }

    #[test]
    fn merge_completed_existing_heading_with_trailing_prose_inserts_before_prose() {
        let body = "#### Completed Tasks\n- A\n- B\n\nAlso had a great meeting.";
        let out = merge_completed_tasks_into_key_accomplishments(
            body,
            &vec!["C".to_string()],
        );
        assert_eq!(
            out.new_key_accomplishments,
            "#### Completed Tasks\n- A\n- B\n- C\n\nAlso had a great meeting."
        );
    }

    #[test]
    fn merge_completed_dedupes_against_bullets_via_normalization() {
        // Existing bullet "- ship it" should match a candidate
        // "Ship It!" via normalized form (case + trailing punctuation).
        let body = "#### Completed Tasks\n- ship it";
        let out = merge_completed_tasks_into_key_accomplishments(
            body,
            &vec!["Ship It!".to_string(), "Fix the bug".to_string()],
        );
        assert_eq!(out.imported, 1);
        assert_eq!(out.skipped, 1);
        assert_eq!(
            out.new_key_accomplishments,
            "#### Completed Tasks\n- ship it\n- Fix the bug"
        );
    }

    #[test]
    fn merge_completed_dedupes_against_plain_prose_lines() {
        // Prose line "Ship the widget" (no bullet marker) should
        // still block a duplicate import of a "Ship the widget" task.
        let body = "Highlights this week:\nShip the widget went perfectly.";
        let out = merge_completed_tasks_into_key_accomplishments(
            body,
            &vec!["Ship the widget went perfectly.".to_string()],
        );
        assert_eq!(out.imported, 0);
        assert_eq!(out.skipped, 1);
        // Body unchanged, no heading added.
        assert_eq!(out.new_key_accomplishments, body);
    }

    #[test]
    fn merge_completed_all_duplicates_returns_body_verbatim() {
        let body = "#### Completed Tasks\n- Foo\n- Bar";
        let out = merge_completed_tasks_into_key_accomplishments(
            body,
            &vec!["foo".to_string(), "BAR!".to_string()],
        );
        assert_eq!(out.imported, 0);
        assert_eq!(out.skipped, 2);
        assert_eq!(out.new_key_accomplishments, body);
    }

    #[test]
    fn merge_completed_duplicates_within_batch_only_add_once() {
        let out = merge_completed_tasks_into_key_accomplishments(
            "",
            &vec![
                "Standup".to_string(),
                "Standup".to_string(),
                "Standup".to_string(),
            ],
        );
        assert_eq!(out.imported, 1);
        assert_eq!(out.skipped, 2);
        assert_eq!(
            out.new_key_accomplishments,
            "#### Completed Tasks\n- Standup"
        );
    }

    #[test]
    fn merge_completed_stops_bullet_walk_at_next_heading() {
        // Second heading (of any level) breaks the bullet-block scan.
        // Insert must land BEFORE the second heading.
        let body = "#### Completed Tasks\n- A\n\n### Another Section\n- prose bullet";
        let out = merge_completed_tasks_into_key_accomplishments(
            body,
            &vec!["B".to_string()],
        );
        assert_eq!(
            out.new_key_accomplishments,
            "#### Completed Tasks\n- A\n- B\n\n### Another Section\n- prose bullet"
        );
    }

    #[test]
    fn merge_completed_heading_case_variation_still_matches() {
        // User hand-typed "#### completed tasks" (lowercase) before
        // clicking Import — we still recognize it and append under it,
        // not stack a new heading.
        let body = "#### completed tasks\n- foo";
        let out = merge_completed_tasks_into_key_accomplishments(
            body,
            &vec!["bar".to_string()],
        );
        assert_eq!(
            out.new_key_accomplishments,
            "#### completed tasks\n- foo\n- bar"
        );
    }

    #[test]
    fn merge_completed_heading_present_but_no_bullets_inserts_immediately_below() {
        let body = "#### Completed Tasks\nSome intro prose.";
        let out = merge_completed_tasks_into_key_accomplishments(
            body,
            &vec!["A".to_string()],
        );
        assert_eq!(
            out.new_key_accomplishments,
            "#### Completed Tasks\n- A\nSome intro prose."
        );
    }

    #[test]
    fn merge_completed_heading_is_last_line_appends_bullets_after() {
        let body = "Great sprint.\n\n#### Completed Tasks";
        let out = merge_completed_tasks_into_key_accomplishments(
            body,
            &vec!["A".to_string(), "B".to_string()],
        );
        assert_eq!(
            out.new_key_accomplishments,
            "Great sprint.\n\n#### Completed Tasks\n- A\n- B"
        );
    }

    #[test]
    fn merge_completed_empty_candidates_is_noop() {
        let body = "#### Completed Tasks\n- A";
        let out = merge_completed_tasks_into_key_accomplishments(body, &vec![]);
        assert_eq!(out.imported, 0);
        assert_eq!(out.skipped, 0);
        assert_eq!(out.new_key_accomplishments, body);
    }

    #[test]
    fn merge_completed_first_of_multiple_headings_wins() {
        // If two "#### Completed Tasks" headings exist (from pre-fix
        // imports), we append under the FIRST one. User can
        // consolidate the second block manually.
        let body = "#### Completed Tasks\n- A\n\n#### Completed Tasks\n- B";
        let out = merge_completed_tasks_into_key_accomplishments(
            body,
            &vec!["C".to_string()],
        );
        assert_eq!(
            out.new_key_accomplishments,
            "#### Completed Tasks\n- A\n- C\n\n#### Completed Tasks\n- B"
        );
    }

    // ---------------------------------------------------------------
    // delete_task_from_tasks_body tests
    // ---------------------------------------------------------------

    #[test]
    fn delete_task_removes_only_targeted_line() {
        let body = format!(
            "{TASKS_ANCHOR_INCOMPLETE}\n\
             - [ ] Keep A\n\
             - [ ] Drop me\n\
             - [ ] Keep B\n\
             {TASKS_ANCHOR_COMPLETED}"
        );
        let hash = hash_task_text(&normalize_task_text("Drop me"));
        let new_body = delete_task_from_tasks_body(&body, &hash, 0).unwrap();
        let bd = parse_tasks_body(&new_body);
        assert_eq!(
            bd.incomplete_lines,
            vec!["- [ ] Keep A".to_string(), "- [ ] Keep B".to_string()]
        );
        assert!(bd.completed_lines.is_empty());
    }

    #[test]
    fn delete_task_targets_correct_duplicate_via_ordinal() {
        // Three identical tasks. Deleting ordinal=1 (middle) must
        // remove ONLY the middle one; ord=0 and ord=2 survive.
        let body = format!(
            "{TASKS_ANCHOR_INCOMPLETE}\n\
             - [ ] Standup\n\
             - [ ] Standup\n\
             - [ ] Standup\n\
             {TASKS_ANCHOR_COMPLETED}"
        );
        let hash = hash_task_text(&normalize_task_text("Standup"));
        let new_body = delete_task_from_tasks_body(&body, &hash, 1).unwrap();
        let bd = parse_tasks_body(&new_body);
        assert_eq!(
            bd.incomplete_lines,
            vec!["- [ ] Standup".to_string(), "- [ ] Standup".to_string()]
        );
    }

    #[test]
    fn delete_task_removes_completed_task_and_leaves_anchors_intact() {
        let body = format!(
            "{TASKS_ANCHOR_INCOMPLETE}\n\
             - [ ] Still open\n\
             {TASKS_ANCHOR_COMPLETED}\n\
             - [x] Nuke me"
        );
        let hash = hash_task_text(&normalize_task_text("Nuke me"));
        let new_body = delete_task_from_tasks_body(&body, &hash, 0).unwrap();
        assert!(new_body.contains(TASKS_ANCHOR_INCOMPLETE));
        assert!(new_body.contains(TASKS_ANCHOR_COMPLETED));
        assert!(new_body.contains("- [ ] Still open"));
        assert!(!new_body.contains("Nuke me"));
    }

    #[test]
    fn delete_task_errors_on_missing_hash() {
        let body = format!(
            "{TASKS_ANCHOR_INCOMPLETE}\n\
             - [ ] Only\n\
             {TASKS_ANCHOR_COMPLETED}"
        );
        let err = delete_task_from_tasks_body(&body, "not-a-real-hash", 0).unwrap_err();
        assert!(err.contains("couldn't be found"), "err: {err}");
    }

    #[test]
    fn delete_task_errors_on_wrong_ordinal_for_existing_hash() {
        let body = format!(
            "{TASKS_ANCHOR_INCOMPLETE}\n\
             - [ ] Only one\n\
             {TASKS_ANCHOR_COMPLETED}"
        );
        let hash = hash_task_text(&normalize_task_text("Only one"));
        // Task exists at ord=0, but caller asked for ord=1.
        let err = delete_task_from_tasks_body(&body, &hash, 1).unwrap_err();
        assert!(err.contains("couldn't be found"), "err: {err}");
    }

    #[test]
    fn delete_task_last_task_leaves_empty_body_with_anchors() {
        let body = format!(
            "{TASKS_ANCHOR_INCOMPLETE}\n\
             - [ ] Last one\n\
             {TASKS_ANCHOR_COMPLETED}"
        );
        let hash = hash_task_text(&normalize_task_text("Last one"));
        let new_body = delete_task_from_tasks_body(&body, &hash, 0).unwrap();
        let bd = parse_tasks_body(&new_body);
        assert!(bd.incomplete_lines.is_empty());
        assert!(bd.completed_lines.is_empty());
        // Anchors survive so subsequent appends land in the right place.
        assert!(new_body.contains(TASKS_ANCHOR_INCOMPLETE));
        assert!(new_body.contains(TASKS_ANCHOR_COMPLETED));
    }

    #[test]
    fn toggle_task_move_from_incomplete_to_completed() {
        let body = format!(
            "{TASKS_ANCHOR_INCOMPLETE}\n\
             - [ ] Ship the thing\n\
             {TASKS_ANCHOR_COMPLETED}"
        );
        let hash = hash_task_text(&normalize_task_text("Ship the thing"));
        let (new_body, new_state) =
            toggle_task_in_tasks_body(&body, &hash, 0).unwrap();
        assert!(new_state);
        let bd = parse_tasks_body(&new_body);
        assert!(bd.incomplete_lines.is_empty());
        assert_eq!(bd.completed_lines, vec!["- [x] Ship the thing".to_string()]);
    }

    #[test]
    fn toggle_task_move_from_completed_to_incomplete() {
        let body = format!(
            "{TASKS_ANCHOR_INCOMPLETE}\n\
             {TASKS_ANCHOR_COMPLETED}\n\
             - [x] Shipped"
        );
        let hash = hash_task_text(&normalize_task_text("Shipped"));
        // In the outer parse of body, this task's ordinal is 0
        // (only task in the body).
        let (new_body, new_state) =
            toggle_task_in_tasks_body(&body, &hash, 0).unwrap();
        assert!(!new_state);
        let bd = parse_tasks_body(&new_body);
        assert_eq!(bd.incomplete_lines, vec!["- [ ] Shipped".to_string()]);
        assert!(bd.completed_lines.is_empty());
    }

    #[test]
    fn toggle_task_preserves_other_tasks() {
        let body = format!(
            "{TASKS_ANCHOR_INCOMPLETE}\n\
             - [ ] Alpha\n\
             - [ ] Beta\n\
             - [ ] Gamma\n\
             {TASKS_ANCHOR_COMPLETED}\n\
             - [x] Delta"
        );
        // Ordinal is per-unique-hash, not per-position. Beta is
        // the sole occurrence of its normalized text, so its
        // ordinal is 0 (same as Alpha's, Gamma's, and Delta's
        // ordinals under their respective hashes).
        let hash = hash_task_text(&normalize_task_text("Beta"));
        let (new_body, _) = toggle_task_in_tasks_body(&body, &hash, 0).unwrap();
        let bd = parse_tasks_body(&new_body);
        assert_eq!(
            bd.incomplete_lines,
            vec!["- [ ] Alpha".to_string(), "- [ ] Gamma".to_string()]
        );
        assert_eq!(
            bd.completed_lines,
            vec!["- [x] Delta".to_string(), "- [x] Beta".to_string()]
        );
    }

    #[test]
    fn toggle_task_errors_on_missing_hash() {
        let body = format!(
            "{TASKS_ANCHOR_INCOMPLETE}\n- [ ] One\n{TASKS_ANCHOR_COMPLETED}"
        );
        let err = toggle_task_in_tasks_body(&body, "deadbeef", 0).unwrap_err();
        assert!(err.contains("couldn't be found"));
    }

    #[test]
    fn flip_checkbox_in_line_handles_both_directions() {
        assert_eq!(
            flip_checkbox_in_line("- [ ] Ship it"),
            Some("- [x] Ship it".to_string())
        );
        assert_eq!(
            flip_checkbox_in_line("- [x] Done"),
            Some("- [ ] Done".to_string())
        );
        assert_eq!(
            flip_checkbox_in_line("- [X] Done"),
            Some("- [ ] Done".to_string())
        );
        // Preserves indentation.
        assert_eq!(
            flip_checkbox_in_line("  - [ ] Nested"),
            Some("  - [x] Nested".to_string())
        );
    }

    #[test]
    fn flip_checkbox_in_line_rejects_non_task_lines() {
        assert_eq!(flip_checkbox_in_line("Some prose"), None);
        assert_eq!(flip_checkbox_in_line("- Regular bullet"), None);
        assert_eq!(flip_checkbox_in_line("- [y] Wrong marker"), None);
    }

    // ---------------------------------------------------------------
    // render_task_text_inline
    // ---------------------------------------------------------------

    #[test]
    fn render_plain_text_passes_through() {
        assert_eq!(render_task_text_inline("Just a task"), "Just a task");
    }

    #[test]
    fn render_empty_stays_empty() {
        assert_eq!(render_task_text_inline(""), "");
    }

    #[test]
    fn render_bold_via_asterisks() {
        assert_eq!(
            render_task_text_inline("Ship **the** thing"),
            "Ship <strong>the</strong> thing"
        );
    }

    #[test]
    fn render_bold_via_underscores() {
        assert_eq!(
            render_task_text_inline("Ship __the__ thing"),
            "Ship <strong>the</strong> thing"
        );
    }

    #[test]
    fn render_italic_via_asterisks() {
        assert_eq!(
            render_task_text_inline("Ship *the* thing"),
            "Ship <em>the</em> thing"
        );
    }

    #[test]
    fn render_italic_via_underscores() {
        assert_eq!(
            render_task_text_inline("Ship _the_ thing"),
            "Ship <em>the</em> thing"
        );
    }

    #[test]
    fn render_strikethrough() {
        assert_eq!(
            render_task_text_inline("~~drop this~~"),
            "<del>drop this</del>"
        );
    }

    #[test]
    fn render_inline_code() {
        assert_eq!(
            render_task_text_inline("Check `play.prodigygame.org`"),
            "Check <code>play.prodigygame.org</code>"
        );
    }

    #[test]
    fn render_mixed_formatting_from_bug_report() {
        // Chris's actual smoke-test line that surfaced the bug.
        assert_eq!(
            render_task_text_inline("Task 6, with **text** *formatting* ~~for~~ *fun*"),
            "Task 6, with <strong>text</strong> <em>formatting</em> <del>for</del> <em>fun</em>"
        );
    }

    #[test]
    fn render_unclosed_bold_stays_literal() {
        // CommonMark says unmatched delimiters render as literal text.
        // Confirm nothing dangles into the DOM.
        let out = render_task_text_inline("**oops");
        assert!(!out.contains("<strong>"), "no dangling strong tag");
        assert!(out.contains("**oops"), "literal ** preserved");
    }

    #[test]
    fn render_escaped_asterisks_stay_literal() {
        assert_eq!(
            render_task_text_inline("\\*not bold\\*"),
            "*not bold*"
        );
    }

    #[test]
    fn render_link_is_stripped_but_text_preserved() {
        // Slice 1 policy: <a> not in allowlist, so ammonia strips the
        // tag but ammonia's default is to KEEP the inner text.
        let out = render_task_text_inline("See [MAGE-1041](https://foo.com/x)");
        assert!(!out.contains("<a "), "anchor tag stripped");
        assert!(!out.contains("href="), "no href leaks");
        assert!(out.contains("MAGE-1041"), "link label preserved as text");
        assert!(
            !out.contains("https://foo.com"),
            "URL not leaked to the DOM"
        );
    }

    // XSS / injection defense — vectors from the research phase.
    // Every one of these MUST render as inert text or be stripped.

    #[test]
    fn xss_raw_script_tag_is_stripped() {
        let out = render_task_text_inline("<script>alert(1)</script>");
        assert!(!out.contains("<script"), "no script tag in output");
        assert!(!out.contains("alert(1)"), "no script body in output");
    }

    #[test]
    fn xss_img_onerror_is_stripped() {
        let out = render_task_text_inline("<img src=x onerror=alert(1)>");
        assert!(!out.contains("<img"), "no img tag in output");
        assert!(!out.contains("onerror"), "no event handler in output");
    }

    #[test]
    fn xss_iframe_srcdoc_is_stripped() {
        let out = render_task_text_inline(
            "<iframe srcdoc='<script>alert(1)</script>'></iframe>",
        );
        assert!(!out.contains("<iframe"), "no iframe in output");
        assert!(!out.contains("srcdoc"), "no srcdoc attr in output");
    }

    #[test]
    fn xss_javascript_url_in_markdown_link_is_dropped() {
        let out = render_task_text_inline("[click](javascript:alert(1))");
        assert!(!out.contains("<a "), "no anchor tag");
        assert!(!out.contains("javascript:"), "no js url leaks");
        assert!(!out.contains("alert(1)"), "no payload leaks");
    }

    #[test]
    fn xss_data_url_in_markdown_link_is_dropped() {
        let out = render_task_text_inline("[click](data:text/html,<script>alert(1)</script>)");
        assert!(!out.contains("data:"), "no data url leaks");
        assert!(!out.contains("<script"), "no script leaks");
    }

    #[test]
    fn xss_markdown_image_is_neutralized() {
        // pulldown-cmark renders ![alt](url) as <img>; ammonia strips
        // <img> from the allowlist. The alt text is kept by ammonia as
        // text between the stripped tags — verify no src / onerror
        // leaks into the DOM.
        let out = render_task_text_inline("![alt](javascript:alert(1))");
        assert!(!out.contains("<img"), "no img tag in output");
        assert!(!out.contains("javascript:"), "no js url leaks");
        assert!(!out.contains("onerror"), "no event handler leaks");
    }

    #[test]
    fn xss_html_entities_render_as_text_not_tags() {
        // &lt;script&gt; is a literal string, not markup. It must render
        // as visible < and > characters.
        let out = render_task_text_inline("&lt;script&gt;alert(1)&lt;/script&gt;");
        assert!(!out.contains("<script"), "no live script tag");
        // The entity-decoded text still shows the literal <script> markers
        // as visible text — that's fine, they're just characters now.
    }

    #[test]
    fn xss_style_tag_is_stripped() {
        let out = render_task_text_inline("<style>body{display:none}</style>Task");
        assert!(!out.contains("<style"), "no style tag in output");
        assert!(out.contains("Task"), "trailing text preserved");
    }

    #[test]
    fn xss_svg_onload_is_stripped() {
        let out = render_task_text_inline("<svg onload=alert(1)>");
        assert!(!out.contains("<svg"), "no svg tag");
        assert!(!out.contains("onload"), "no event handler");
    }

    #[test]
    fn xss_html_comment_with_conditional_is_stripped() {
        // IE conditional comment style — ancient but still worth testing.
        let out = render_task_text_inline("<!--[if IE]><script>x()</script><![endif]-->");
        assert!(!out.contains("<script"), "no script leaked from comment");
    }

    #[test]
    fn render_no_paragraph_wrapper_leaks_into_output() {
        // The peel step must run. Otherwise task-list items break out
        // of their <span> and look like block content.
        let out = render_task_text_inline("hello");
        assert!(!out.starts_with("<p>"), "no <p> wrapper: got {out:?}");
        assert!(!out.contains("</p>"), "no </p> in output: got {out:?}");
    }

    #[test]
    fn render_preserves_ampersand_as_entity() {
        // Bare `&` must become `&amp;` so downstream `{@html}` doesn't
        // mistake it for the start of an entity.
        let out = render_task_text_inline("A & B");
        assert!(
            out.contains("&amp;") || out.contains("&#38;"),
            "expected escaped ampersand, got {out:?}"
        );
    }

    #[test]
    fn render_preserves_less_than_as_entity() {
        let out = render_task_text_inline("2 < 3");
        assert!(
            out.contains("&lt;"),
            "expected escaped &lt; got {out:?}"
        );
    }

    #[test]
    fn render_nested_formatting() {
        // **bold *italic* bold**
        let out = render_task_text_inline("**bold *inner* bold**");
        assert!(out.contains("<strong>"), "outer strong present");
        assert!(out.contains("<em>"), "inner em present");
    }

    #[test]
    fn render_code_span_hides_markdown_metacharacters() {
        // Inside `code`, ** and * are literal.
        let out = render_task_text_inline("`**not bold**`");
        assert!(out.contains("<code>"), "code tag present");
        assert!(!out.contains("<strong>"), "no strong inside code");
        assert!(
            out.contains("**not bold**") || out.contains("**not bold**"),
            "literal asterisks preserved inside code: got {out:?}"
        );
    }

    #[test]
    fn render_strips_disallowed_inline_tags() {
        // pulldown-cmark passes raw HTML through by default; ammonia is
        // what enforces the allowlist. `<b>` is disallowed (we use
        // `<strong>` instead), so ammonia strips the tag but keeps the
        // inner text.
        let out = render_task_text_inline("<b>hi</b>");
        assert!(!out.contains("<b>"), "no <b> in output: got {out:?}");
        assert!(out.contains("hi"), "text preserved: got {out:?}");
    }

    #[test]
    fn xss_event_handlers_stripped_from_allowed_tags() {
        // Attribute-stripping regression guard on tags we DO allow.
        // ammonia's builder-default posture removes all unlisted
        // attributes; we don't allow any, so every event handler,
        // style, and data-* must be gone.
        for input in [
            "<strong onclick=\"alert(1)\">bold</strong>",
            "<em onmouseover=\"alert(1)\">it</em>",
            "<code onerror=\"alert(1)\">c</code>",
            "<del onload=\"alert(1)\">d</del>",
        ] {
            let out = render_task_text_inline(input);
            assert!(
                !out.contains("onclick") && !out.contains("onmouseover")
                    && !out.contains("onerror") && !out.contains("onload"),
                "event handler leaked for input {input:?}: got {out:?}"
            );
        }
    }

    #[test]
    fn xss_style_attribute_stripped_from_allowed_tags() {
        let out = render_task_text_inline(
            "<strong style=\"background:url(javascript:alert(1))\">x</strong>",
        );
        assert!(!out.contains("style="), "style attr must be stripped: {out:?}");
        assert!(
            !out.contains("javascript:"),
            "no js url in output: {out:?}"
        );
    }

    #[test]
    fn xss_data_and_id_attributes_stripped_from_allowed_tags() {
        let out = render_task_text_inline(
            "<strong id=\"pwn\" class=\"pwn\" data-payload=\"x\">x</strong>",
        );
        // We don't allowlist ANY attributes on inline tags. Everything
        // decorative (id/class/data-*) must disappear too.
        assert!(!out.contains(" id="), "id attr survived: {out:?}");
        assert!(!out.contains(" class="), "class attr survived: {out:?}");
        assert!(!out.contains("data-"), "data-* attr survived: {out:?}");
    }

    #[test]
    fn render_triple_asterisk_produces_bold_and_italic() {
        // Common CommonMark case: ***text*** should yield both <strong>
        // and <em>. The two tag orderings pulldown-cmark might emit
        // (strong-outside-em vs em-outside-strong) are both fine.
        let out = render_task_text_inline("***both***");
        assert!(out.contains("<strong>"), "expected <strong>: {out:?}");
        assert!(out.contains("<em>"), "expected <em>: {out:?}");
        assert!(out.contains("both"), "expected text: {out:?}");
    }

    #[test]
    fn xss_html5_semantic_tags_are_stripped() {
        // ammonia default doesn't allowlist these, but the inline-only
        // policy above explicitly excludes them. Regression guard so a
        // future allowlist expansion doesn't silently admit block markup.
        for tag in ["section", "article", "aside", "nav", "header", "footer"] {
            let input = format!("<{tag}>x</{tag}>");
            let out = render_task_text_inline(&input);
            assert!(
                !out.contains(&format!("<{tag}")),
                "{tag} tag survived: {out:?}"
            );
        }
    }

    #[test]
    fn xss_html5_media_tags_are_stripped() {
        // The vectors that carry the highest XSS punch: <video>,
        // <audio>, <source> can trigger network fetches + JS via event
        // handlers. All must be stripped.
        for input in [
            "<video src=x onerror=alert(1)></video>",
            "<audio src=x onerror=alert(1)></audio>",
            "<source src=x>",
            "<track src=x>",
        ] {
            let out = render_task_text_inline(input);
            assert!(
                !out.contains("<video") && !out.contains("<audio")
                    && !out.contains("<source") && !out.contains("<track"),
                "media tag survived: {input:?} -> {out:?}"
            );
            assert!(!out.contains("onerror"), "handler survived: {out:?}");
        }
    }

    #[test]
    fn render_table_syntax_stays_inert_because_tables_are_off() {
        // ENABLE_TABLES is deliberately unset. `| col | col |` should
        // render as literal text (with escaped pipes), never as a
        // <table>.
        let out = render_task_text_inline("Compare | col1 | col2 | in the sheet");
        assert!(!out.contains("<table"), "no table tag: {out:?}");
        assert!(!out.contains("<td"), "no td tag: {out:?}");
        assert!(out.contains("col1"), "text preserved: {out:?}");
    }

    #[test]
    fn strip_paragraph_wrapper_leaves_multi_paragraph_alone() {
        // Guard: a caller feeding multi-paragraph HTML shouldn't have
        // paragraphs concatenated silently.
        let input = "<p>one</p><p>two</p>";
        assert_eq!(strip_paragraph_wrapper(input), input);
    }

    #[test]
    fn strip_paragraph_wrapper_peels_single_p() {
        assert_eq!(strip_paragraph_wrapper("<p>hello</p>"), "hello");
        assert_eq!(
            strip_paragraph_wrapper("<p>hi <strong>there</strong></p>"),
            "hi <strong>there</strong>"
        );
    }

    #[test]
    fn task_completions_find_by_key() {
        let idx = TaskCompletions {
            version: 1,
            completions: vec![
                TaskCompletion {
                    year: 2026,
                    week: 27,
                    text_hash: "aaa".to_string(),
                    ordinal: 0,
                    completed_at: "2026-07-07T14:00:00-04:00".to_string(),
                },
                TaskCompletion {
                    year: 2026,
                    week: 27,
                    text_hash: "aaa".to_string(),
                    ordinal: 1,
                    completed_at: "2026-07-07T15:00:00-04:00".to_string(),
                },
            ],
        };
        assert!(idx.find(2026, 27, "aaa", 0).is_some());
        assert_eq!(
            idx.find(2026, 27, "aaa", 1).unwrap().completed_at,
            "2026-07-07T15:00:00-04:00"
        );
        assert!(idx.find(2026, 27, "aaa", 2).is_none());
        assert!(idx.find(2025, 27, "aaa", 0).is_none());
    }

    // ---------------------------------------------------------------
    // TaskDueDates (Phase 3e) — sidecar + validator tests
    // ---------------------------------------------------------------

    #[test]
    fn validate_due_date_accepts_canonical_shape() {
        assert_eq!(validate_due_date_input("2026-07-15").unwrap(), "2026-07-15");
        // Whitespace tolerated at edges, output is canonical.
        assert_eq!(validate_due_date_input("  2026-07-15  ").unwrap(), "2026-07-15");
    }

    #[test]
    fn validate_due_date_rejects_empty() {
        assert!(validate_due_date_input("").is_err());
        assert!(validate_due_date_input("   ").is_err());
    }

    #[test]
    fn validate_due_date_accepts_non_padded_month_and_canonicalizes() {
        // chrono's %Y-%m-%d accepts single-digit months/days and
        // parses them correctly. We canonicalize on output so the
        // sidecar always stores the zero-padded form.
        assert_eq!(validate_due_date_input("2026-7-15").unwrap(), "2026-07-15");
        assert_eq!(validate_due_date_input("2026-7-5").unwrap(), "2026-07-05");
    }

    #[test]
    fn validate_due_date_rejects_bad_shape() {
        // Wrong separator, embedded time, wrong order, non-date input.
        assert!(validate_due_date_input("2026/07/15").is_err());
        assert!(validate_due_date_input("2026-07-15T09:00:00").is_err());
        assert!(validate_due_date_input("07-15-2026").is_err());
        assert!(validate_due_date_input("today").is_err());
    }

    #[test]
    fn validate_due_date_rejects_out_of_range() {
        // Chrono's NaiveDate::parse_from_str rejects invalid month/day.
        assert!(validate_due_date_input("2026-13-01").is_err()); // no month 13
        assert!(validate_due_date_input("2026-02-30").is_err()); // no Feb 30
        assert!(validate_due_date_input("2026-04-31").is_err()); // April has 30 days
    }

    #[test]
    fn task_due_dates_default_is_empty_current_version() {
        let dd = TaskDueDates::default();
        assert_eq!(dd.due_dates.len(), 0);
        // Default derives version=0 (Rust default for u32); the load
        // path uses serde(default) fallback to current_version() only
        // when the field is absent from JSON. Verify current_version
        // returns the constant so the fallback stays in lockstep with
        // the type version.
        assert_eq!(TaskDueDates::current_version(), CURRENT_TASK_DUE_DATES_VERSION);
    }

    #[test]
    fn task_due_dates_find_by_composite_key() {
        let dd = TaskDueDates {
            version: 1,
            due_dates: vec![
                TaskDueDate {
                    year: 2026,
                    week: 28,
                    text_hash: "aaa".to_string(),
                    ordinal: 0,
                    due_date: "2026-07-15".to_string(),
                },
                TaskDueDate {
                    year: 2026,
                    week: 28,
                    text_hash: "aaa".to_string(),
                    ordinal: 1,
                    due_date: "2026-07-20".to_string(),
                },
            ],
        };
        assert_eq!(
            dd.find(2026, 28, "aaa", 0).unwrap().due_date,
            "2026-07-15"
        );
        assert_eq!(
            dd.find(2026, 28, "aaa", 1).unwrap().due_date,
            "2026-07-20"
        );
        // Wrong ordinal / hash / week / year → miss.
        assert!(dd.find(2026, 28, "aaa", 2).is_none());
        assert!(dd.find(2026, 28, "bbb", 0).is_none());
        assert!(dd.find(2026, 27, "aaa", 0).is_none());
        assert!(dd.find(2025, 28, "aaa", 0).is_none());
    }

    #[test]
    fn task_due_dates_upsert_replaces_existing_entry() {
        let mut dd = TaskDueDates::default();
        dd.upsert(TaskDueDate {
            year: 2026,
            week: 28,
            text_hash: "aaa".to_string(),
            ordinal: 0,
            due_date: "2026-07-15".to_string(),
        });
        assert_eq!(dd.due_dates.len(), 1);
        // Same identity, different date → replace, not append.
        dd.upsert(TaskDueDate {
            year: 2026,
            week: 28,
            text_hash: "aaa".to_string(),
            ordinal: 0,
            due_date: "2026-07-20".to_string(),
        });
        assert_eq!(dd.due_dates.len(), 1, "upsert must overwrite same-key entry");
        assert_eq!(dd.due_dates[0].due_date, "2026-07-20");
        // Different identity → append.
        dd.upsert(TaskDueDate {
            year: 2026,
            week: 28,
            text_hash: "bbb".to_string(),
            ordinal: 0,
            due_date: "2026-07-15".to_string(),
        });
        assert_eq!(dd.due_dates.len(), 2);
    }

    #[test]
    fn task_due_dates_remove_drops_matching_entry_only() {
        let mut dd = TaskDueDates::default();
        for hash in ["aaa", "bbb", "aaa"] {
            let ordinal: u32 = if hash == "aaa" { dd.due_dates.iter().filter(|d| d.text_hash == hash).count() as u32 } else { 0 };
            dd.upsert(TaskDueDate {
                year: 2026,
                week: 28,
                text_hash: hash.to_string(),
                ordinal,
                due_date: "2026-07-15".to_string(),
            });
        }
        assert_eq!(dd.due_dates.len(), 3);
        // Drop aaa/1 only. aaa/0 + bbb/0 stay.
        dd.remove(2026, 28, "aaa", 1);
        assert_eq!(dd.due_dates.len(), 2);
        assert!(dd.find(2026, 28, "aaa", 0).is_some());
        assert!(dd.find(2026, 28, "aaa", 1).is_none());
        assert!(dd.find(2026, 28, "bbb", 0).is_some());
        // Removing something not in the set is a no-op.
        dd.remove(2026, 28, "zzz", 0);
        assert_eq!(dd.due_dates.len(), 2);
    }

    #[tokio::test]
    async fn task_due_dates_load_missing_returns_default() {
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        let dd = TaskDueDates::load(&backend).await.unwrap();
        assert_eq!(dd.due_dates.len(), 0);
    }

    #[tokio::test]
    async fn task_due_dates_load_corrupt_returns_default_with_warning() {
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        backend
            .write_metadata(TASK_DUE_DATES_FILE, "{ not: valid json")
            .await
            .unwrap();
        let dd = TaskDueDates::load(&backend).await.unwrap();
        assert_eq!(
            dd.due_dates.len(),
            0,
            "corrupt sidecar should recover to empty, not error out"
        );
    }

    #[tokio::test]
    async fn task_due_dates_save_load_round_trips() {
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        let mut dd = TaskDueDates::default();
        dd.upsert(TaskDueDate {
            year: 2026,
            week: 28,
            text_hash: "aaa".to_string(),
            ordinal: 0,
            due_date: "2026-07-15".to_string(),
        });
        dd.upsert(TaskDueDate {
            year: 2026,
            week: 29,
            text_hash: "bbb".to_string(),
            ordinal: 2,
            due_date: "2026-07-22".to_string(),
        });
        dd.save(&backend).await.unwrap();

        let loaded = TaskDueDates::load(&backend).await.unwrap();
        assert_eq!(loaded.due_dates.len(), 2);
        assert_eq!(loaded.find(2026, 28, "aaa", 0).unwrap().due_date, "2026-07-15");
        assert_eq!(loaded.find(2026, 29, "bbb", 2).unwrap().due_date, "2026-07-22");
    }

    #[tokio::test]
    async fn task_due_dates_load_partial_json_backfills_version() {
        // A pre-versioning save (or hand-edited file) with just
        // { "dueDates": [...] } must load cleanly via serde(default).
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        backend
            .write_metadata(
                TASK_DUE_DATES_FILE,
                r#"{"dueDates":[{"year":2026,"week":28,"textHash":"aaa","ordinal":0,"dueDate":"2026-07-15"}]}"#,
            )
            .await
            .unwrap();
        let loaded = TaskDueDates::load(&backend).await.unwrap();
        assert_eq!(loaded.version, CURRENT_TASK_DUE_DATES_VERSION);
        assert_eq!(loaded.due_dates.len(), 1);
    }
}

