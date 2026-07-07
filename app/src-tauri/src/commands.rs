//! Tauri commands exposed to the frontend.
//!
//! Currently:
//!   - [`create_note`] — append a Note to the current week's file
//!   - [`read_week`] — return the raw markdown of a given (year, week)
//!   - [`get_settings`] — snapshot of app + journal settings; signals first-run
//!   - [`complete_first_run`] — writes both settings files; restarts if root changed
//!
//! State: the `LocalFilesystem` storage backend is registered as managed
//! Tauri state in `lib::run()`. Its root is determined at startup from
//! `app-settings.json` (or the default if first run).

use std::path::PathBuf;

use chrono::{Local, NaiveDate};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::email::{compose_weekly_email as compose, ComposeResult, MailSend};
use crate::labels::{
    is_iso_date_prefix, record_note_labels, scan_label_sites, LabelEntry, LabelIndex, LabelSite,
    LabelSiteKind,
};
use crate::notes::{
    append_note, iso_week_start, iso_year_week, parse_weekly_summary,
    replace_weekly_summary_in_file, weekly_file_scaffold, CaptureDraft, Note, WeeklySummary,
};
use crate::reminders::{
    request_notification_authorization, restart_reminder_task, ReminderHandle,
};
use crate::sent_log::{
    get_sent_record as load_sent_record, hash_weekly_summary, upsert_sent_record, SentRecord,
};
use crate::{DirtyEntry, DirtyRegistry};
use crate::settings::{
    default_journal_root, AppSettings, CustomTheme, JournalSettings, MailBodyDelivery,
    MailBodyFormat, MailSendMode, OutlookFlavor, ReminderSettings, Theme, CURRENT_VERSION,
};
use crate::storage::{LocalFilesystem, StorageBackend};
use crate::SharedStorage;

// ---------------------------------------------------------------------------
// create_note / read_week
// ---------------------------------------------------------------------------

/// Input payload for [`create_note`]. The frontend sends these fields as a
/// single object argument.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateNoteInput {
    pub title: Option<String>,
    pub body: String,
    #[serde(default)]
    pub labels: Vec<String>,
}

/// Append a Note to the current ISO week's file.
#[tauri::command]
pub async fn create_note(
    app: AppHandle,
    storage_state: State<'_, SharedStorage>,
    input: CreateNoteInput,
) -> Result<(), String> {
    let now = Local::now().fixed_offset();
    let (year, week) = iso_year_week(now.date_naive());

    let note = Note {
        timestamp: now,
        title: input.title.and_then(|t| {
            let trimmed = t.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        }),
        labels: input
            .labels
            .into_iter()
            .map(|l| l.trim().trim_start_matches('#').to_string())
            .filter(|l| !l.is_empty())
            .collect(),
        body: input.body,
    };

    // Hold the write lock across both the weekly-file write AND the
    // labels.json update so concurrent create_note / set_label_color calls
    // can't interleave their load → mutate → save against the label index.
    // Without serialization, two parallel set_label_color invocations could
    // both read the same on-disk snapshot, each mutate their own copy, and
    // the second save would clobber the first's color. (The atomic rename
    // in write_metadata only guarantees per-write durability — it doesn't
    // serialize read-modify-write.)
    let storage = storage_state.write().await;

    append_note(&*storage, year, week, &note)
        .await
        .map_err(|e| e.to_string())?;

    if let Err(e) = record_note_labels(&*storage, &note, now.date_naive()).await {
        eprintln!("warning: label index update failed: {e}");
    }

    emit_weekly_file_changed(&app, year, week);

    Ok(())
}

/// Read the raw markdown of a weekly file. Returns `None` if absent.
#[tauri::command]
pub async fn read_week(
    storage_state: State<'_, SharedStorage>,
    year: u32,
    week: u32,
) -> Result<Option<String>, String> {
    let storage = storage_state.read().await;
    storage
        .read_week(year, week)
        .await
        .map_err(|e| e.to_string())
}

/// Overwrite the entire weekly file with the given markdown. Used by the
/// journal browser's raw-markdown editor (`/journal` route) to save edits
/// to past weeks. The structured Weekly Summary editor at `/summary` uses
/// `update_weekly_summary` instead, which splices changes into the summary
/// section while preserving Weekly Notes below.
#[tauri::command]
pub async fn write_week(
    app: AppHandle,
    storage_state: State<'_, SharedStorage>,
    year: u32,
    week: u32,
    content: String,
) -> Result<(), String> {
    let storage = storage_state.read().await;
    storage
        .write_week(year, week, &content)
        .await
        .map_err(|e| e.to_string())?;

    emit_weekly_file_changed(&app, year, week);

    Ok(())
}

/// List ISO years that have any weekly files, sorted ascending. Empty if
/// the journal root has nothing yet.
#[tauri::command]
pub async fn list_years(storage_state: State<'_, SharedStorage>) -> Result<Vec<u32>, String> {
    let storage = storage_state.read().await;
    storage.list_years().await.map_err(|e| e.to_string())
}

/// List ISO week numbers present for the given year, sorted ascending.
/// Empty if the year folder doesn't exist.
#[tauri::command]
pub async fn list_weeks(
    storage_state: State<'_, SharedStorage>,
    year: u32,
) -> Result<Vec<u32>, String> {
    let storage = storage_state.read().await;
    storage.list_weeks(year).await.map_err(|e| e.to_string())
}

/// Return all known labels with their usage stats, sorted by recent-then-frequent
/// (the autocomplete ranking from `docs/label-system.md`).
#[tauri::command]
pub async fn get_labels(
    storage_state: State<'_, SharedStorage>,
) -> Result<Vec<LabelEntry>, String> {
    let storage = storage_state.read().await;
    let index = LabelIndex::load(&*storage)
        .await
        .map_err(|e| e.to_string())?;
    Ok(index.labels)
}

/// Set or clear the persisted color override for a label.
///
/// Phase 2.8+ "Colorful Labels": the user can pin a specific hex color to
/// a label (Settings > Theme > Label colors, or right-click on a chip);
/// the lazy-assignment path also calls this from the frontend after the
/// first hash-derived color is computed for a brand-new label so that
/// re-hashing on a future release that changes the seed doesn't suddenly
/// recolor every existing chip.
///
/// - `name`: the label's canonical name (no leading `#`, already trimmed).
///   Missing entries return Err — the caller is expected to have a real
///   label in mind, not a typo'd one.
/// - `color`: `Some("#rrggbb")` to set, `None` to clear. Validation reuses
///   `settings::deserialize_hex6_option` so the same rules apply as for
///   CustomTheme primaries; malformed input surfaces as a clear error
///   rather than a silently dropped override.
#[tauri::command]
pub async fn set_label_color(
    storage_state: State<'_, SharedStorage>,
    name: String,
    color: Option<String>,
) -> Result<(), String> {
    // Write lock — set_label_color does a load → mutate → save on
    // labels.json. A read lock would let two concurrent set_label_color
    // calls (or one set_label_color + one create_note that touches the
    // index) lose updates by racing on the in-memory copy. The atomic
    // rename in write_metadata makes individual writes durable; the
    // serialization here is what keeps the read-modify-write sequence
    // consistent.
    let storage = storage_state.write().await;
    set_label_color_impl(&*storage, &name, color).await
}

/// Implementation backbone of [`set_label_color`] — split out so unit
/// tests can drive it against a `LocalFilesystem` without standing up the
/// full Tauri `State` machinery. Loads the label index, validates and
/// normalizes the incoming hex, applies the update (or clears it), and
/// saves. `color: None` clears any existing override; `color: Some(...)`
/// runs through the same validator the serde deserializer uses so an
/// in-process caller can't sneak malformed input past the wire boundary.
pub(crate) async fn set_label_color_impl<B: StorageBackend + ?Sized>(
    backend: &B,
    name: &str,
    color: Option<String>,
) -> Result<(), String> {
    let normalized = match color {
        None => None,
        Some(s) => {
            let lower = s.to_ascii_lowercase();
            if is_hex6_color(&lower) {
                Some(lower)
            } else {
                return Err(format!(
                    "expected 6-digit hex color like #rrggbb, got {s:?}"
                ));
            }
        }
    };

    let mut index = LabelIndex::load(backend).await.map_err(|e| e.to_string())?;

    let entry = index
        .labels
        .iter_mut()
        .find(|e| e.name == name)
        .ok_or_else(|| format!("label {name:?} not found in index"))?;
    entry.color = normalized;

    index.save(backend).await.map_err(|e| e.to_string())
}

/// Trim + coerce empty → None for a settings string field. Used by
/// `complete_first_run` and `update_settings` on user_email /
/// manager_email / manager_name / bamboo_title so an empty box in the
/// UI persists as `None` (which the Send button's "is this set?"
/// gate then reads cleanly). Prior to extraction each field expanded
/// the same 4-line `.map(...).filter(...)` chain inline.
fn normalize_optional_string(opt: Option<&String>) -> Option<String> {
    opt.map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Callback return value for `walk_all_weeks_descending`. Lets a caller
/// stop the walk early (e.g., `search_journal_impl` hitting its
/// MAX_RESULTS cap) without needing labelled-break plumbing at every
/// site.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum WalkControl {
    Continue,
    Stop,
}

/// Walk every weekly file newest-first (years descending, weeks
/// descending within each year), invoking `per_file` for each file's
/// full markdown content. Return `WalkControl::Stop` from the callback
/// to end the walk early.
///
/// Per-file read/list errors are logged via `eprintln!` with the given
/// `tag` prefix and skipped — matching the "don't abort on partial
/// failure" posture from Phase 2.8b's atomic-write work (locked
/// decision #7). Only a hard failure at `list_years` bubbles up as
/// `Err`.
///
/// Consolidates the walk skeleton that used to live inline in
/// `rebuild_label_index_impl`, `get_label_stats`, `get_notes_for_label`,
/// and `search_journal_impl`. Callers now supply just the per-file
/// body.
pub(crate) async fn walk_all_weeks_descending<B, F>(
    backend: &B,
    tag: &str,
    mut per_file: F,
) -> Result<(), String>
where
    B: StorageBackend + ?Sized,
    F: FnMut(u32, u32, String) -> WalkControl,
{
    let mut years = backend.list_years().await.map_err(|e| e.to_string())?;
    years.sort_unstable();
    years.reverse();

    for year in years {
        let mut weeks = match backend.list_weeks(year).await {
            Ok(w) => w,
            Err(e) => {
                eprintln!("[{tag}] list_weeks({year}) failed: {e}");
                continue;
            }
        };
        weeks.sort_unstable();
        weeks.reverse();

        for week in weeks {
            let content = match backend.read_week(year, week).await {
                Ok(Some(c)) => c,
                Ok(None) => continue,
                Err(e) => {
                    eprintln!("[{tag}] read_week({year},{week}) failed: {e}");
                    continue;
                }
            };
            if matches!(per_file(year, week, content), WalkControl::Stop) {
                return Ok(());
            }
        }
    }

    Ok(())
}

/// Local mirror of `settings::is_hex6` for the `set_label_color` argument
/// validator. Kept inline here (rather than re-exported from settings) to
/// avoid widening the settings module's public surface for a single
/// command-side check.
fn is_hex6_color(s: &str) -> bool {
    let bytes = s.as_bytes();
    bytes.len() == 7
        && bytes[0] == b'#'
        && bytes[1..].iter().all(|b| b.is_ascii_hexdigit())
}

// ---------------------------------------------------------------------------
// rebuild_label_index
// ---------------------------------------------------------------------------

/// What [`rebuild_label_index`] reports back to the Labels Settings tab so
/// the loading overlay can show "scanned N files, found M labels" once the
/// rebuild settles. `failed_files` lists weekly markdown paths that errored
/// during the scan — the rebuild still completed (the entry just wasn't
/// counted from that file).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RebuildResult {
    pub labels_found: u32,
    pub files_scanned: u32,
    pub duration_ms: u64,
    /// Weekly file paths (rendered as `"YYYY/YYYY-Www.md"`) that errored
    /// during the scan. Per Chris's locked decision #7, we don't roll back
    /// on partial failure — we just surface what couldn't be read.
    pub failed_files: Vec<String>,
}

/// Walk every weekly file under the journal root, rebuild `labels.json`
/// from the explicit-labels sites found there, and report what we scanned.
///
/// Invoked by the Labels Settings tab on first open
/// per Settings session so the per-label color editor can render against
/// fresh data even if the live index has drifted from disk (e.g. user
/// hand-edited a weekly file). Color overrides survive the rebuild: we
/// read the current `labels.json` BEFORE regenerating and carry forward
/// any `color` value already on a given label name.
///
/// Per-file read errors are logged via `eprintln` and pushed into
/// `failed_files`; they never abort the rebuild. The only hard failure
/// path is the final `labels.json` save itself, which returns `Err` so
/// the UI can surface that the rebuild didn't land.
///
/// Locked-decision references:
///   - #2: scan reads explicit-label sites only (Notes `**Labels:**` line
///     + Summary `### Labels` subsection). Inline `#hashtag` text in prose
///     deliberately stays uncounted in the rebuilt index, matching the
///     delete-cascade scope.
///   - #7: no rollback on partial failure — `failed_files` is the contract.
#[tauri::command]
pub async fn rebuild_label_index(
    storage_state: State<'_, SharedStorage>,
) -> Result<RebuildResult, String> {
    let storage = storage_state.write().await;
    rebuild_label_index_impl(&*storage).await
}

/// Backend of [`rebuild_label_index`], factored out so unit tests can drive
/// it against a `LocalFilesystem` without standing up the Tauri `State`
/// machinery.
pub(crate) async fn rebuild_label_index_impl<B: StorageBackend + ?Sized>(
    backend: &B,
) -> Result<RebuildResult, String> {
    let start = std::time::Instant::now();

    // Step 1: preserve color overrides from the existing index, keyed by
    // label name. Read BEFORE we generate the replacement so a user-set
    // color survives the rebuild — that's the whole reason we layer
    // overrides onto a fresh count, instead of just running record_note_labels
    // over every file.
    let existing = LabelIndex::load(backend).await.map_err(|e| e.to_string())?;
    let mut color_overrides: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    for entry in &existing.labels {
        if let Some(color) = entry.color.as_ref() {
            color_overrides.insert(entry.name.clone(), color.clone());
        }
    }

    // Step 2: walk year -> week -> read_week and collect, per label, the
    // file count and the set of week-start dates it appeared in. We dedup
    // within a single file (mirrors `record_note_labels`'s "1 increment
    // per Note's combined label set"), so a label that shows up in both
    // the Summary subsection AND a Note labels line in the same weekly
    // file still counts as one for that file.
    struct Acc {
        count: u32,
        first_used: NaiveDate,
        last_used: NaiveDate,
    }
    let mut acc: std::collections::HashMap<String, Acc> = std::collections::HashMap::new();
    let mut files_scanned: u32 = 0;
    let mut failed_files: Vec<String> = Vec::new();

    let years = backend.list_years().await.map_err(|e| e.to_string())?;
    for year in years {
        let weeks = match backend.list_weeks(year).await {
            Ok(w) => w,
            Err(e) => {
                // Year directory listing failed — log and skip the whole
                // year rather than aborting. Per-file failures below have
                // the same posture.
                eprintln!("[rebuild_label_index] list_weeks({year}) failed: {e}");
                continue;
            }
        };
        for week in weeks {
            let pretty_path = format!("{year:04}/{year:04}-W{week:02}.md");
            let content = match backend.read_week(year, week).await {
                Ok(Some(c)) => c,
                Ok(None) => {
                    // list_weeks said it was there but read_week returns
                    // None — treat as scanned-but-empty rather than an
                    // error. (Unlikely race; mostly defensive.)
                    files_scanned = files_scanned.saturating_add(1);
                    continue;
                }
                Err(e) => {
                    eprintln!("[rebuild_label_index] read_week({year},{week}) failed: {e}");
                    failed_files.push(pretty_path);
                    continue;
                }
            };
            files_scanned = files_scanned.saturating_add(1);

            // The week-start date is what we record for first_used /
            // last_used. The label might live in a Note dated mid-week,
            // but we don't carry per-Note dates here — Phase 3a's stat
            // model uses ISO-week granularity, matching what
            // `Note::weekly_file_path` already does.
            let date = iso_week_start(year, week);

            // Dedup names within this single file.
            let sites = scan_label_sites(&content);
            let mut names_in_file: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            for site in sites {
                for name in site.names {
                    names_in_file.insert(name);
                }
            }

            for name in names_in_file {
                match acc.get_mut(&name) {
                    Some(entry) => {
                        entry.count = entry.count.saturating_add(1);
                        if date < entry.first_used {
                            entry.first_used = date;
                        }
                        if date > entry.last_used {
                            entry.last_used = date;
                        }
                    }
                    None => {
                        acc.insert(
                            name,
                            Acc {
                                count: 1,
                                first_used: date,
                                last_used: date,
                            },
                        );
                    }
                }
            }
        }
    }

    // Step 3: assemble a fresh LabelIndex. Color overrides graft onto
    // matching names; entries with no override stay `color: None` so the
    // chip renderer's hash-derived path keeps working unchanged.
    let mut new_index = LabelIndex::default();
    let labels_found = acc.len() as u32;
    for (name, a) in acc.into_iter() {
        let color = color_overrides.get(&name).cloned();
        new_index.labels.push(LabelEntry {
            name,
            count: a.count,
            first_used: a.first_used,
            last_used: a.last_used,
            color,
        });
    }
    // Mirror `touch`'s sort so the rebuilt index reads back in the same
    // order the autocomplete consumer (and the Labels tab) expects:
    // most-recent last_used first, then count desc, then alphabetical.
    new_index.labels.sort_by(|a, b| {
        b.last_used
            .cmp(&a.last_used)
            .then(b.count.cmp(&a.count))
            .then(a.name.cmp(&b.name))
    });

    // Step 4: atomic save. Failure here IS fatal — the caller's UI needs
    // to know the rebuild didn't actually land on disk.
    new_index.save(backend).await.map_err(|e| e.to_string())?;

    let duration_ms = start.elapsed().as_millis() as u64;
    Ok(RebuildResult {
        labels_found,
        files_scanned,
        duration_ms,
        failed_files,
    })
}

// ---------------------------------------------------------------------------
// get_label_stats
// ---------------------------------------------------------------------------

/// On-demand usage breakdown for a single label, surfaced in the Labels
/// Settings tab's per-label details popup. Per locked-decision #1 we do NOT
/// extend `labels.json` with per-site counts — every call re-walks weekly
/// files so the numbers always reflect what's actually on disk.
///
/// `index_count` is the cached file-occurrence count from `labels.json` so
/// the caller (and our own eprintln drift warning) can compare against
/// `total` and notice when the index has fallen out of sync. Per locked
/// decision #8, drift is reported but not auto-repaired here — the dedicated
/// `rebuild_label_index` command is the user's repair seam.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelStats {
    pub total: u32,
    pub in_notes: u32,
    pub in_summaries: u32,
    /// `labels.json`'s cached file-occurrence count for the same name.
    /// Surfaced for drift detection; `total` is the authoritative number
    /// per locked-decision #8.
    pub index_count: u32,
}

/// Walk every weekly file and count how often `name` appears in each kind of
/// explicit-labels site (Note `**Labels:**` line vs Summary `### Labels`
/// subsection). The counts are PER SITE, not per file — a label that
/// appears in both the Summary subsection AND a Note's labels line in the
/// same weekly file shows up once in `in_summaries` AND once in `in_notes`.
/// (That intentionally differs from `rebuild_label_index_impl`'s per-file
/// dedup, because the details popup wants to show the user how their label
/// usage actually breaks down across the two surfaces.)
///
/// Per-file read errors are logged via `eprintln` and skipped — they never
/// abort the scan, matching `rebuild_label_index_impl`'s posture (locked
/// decision #7). The only hard failures come from the journal-root level
/// listing or the labels.json load.
#[tauri::command]
pub async fn get_label_stats(
    storage_state: State<'_, SharedStorage>,
    name: String,
) -> Result<LabelStats, String> {
    // Read-only operation — no labels.json mutation.
    let storage = storage_state.read().await;

    let mut in_notes: u32 = 0;
    let mut in_summaries: u32 = 0;

    walk_all_weeks_descending(&*storage, "label-stats", |_year, _week, content| {
        for site in scan_label_sites(&content) {
            if site.names.iter().any(|n| n == &name) {
                match site.kind {
                    LabelSiteKind::NoteLabelsLine => {
                        in_notes = in_notes.saturating_add(1);
                    }
                    LabelSiteKind::SummaryLabelsSubsection => {
                        in_summaries = in_summaries.saturating_add(1);
                    }
                }
            }
        }
        WalkControl::Continue
    })
    .await?;

    let total = in_notes.saturating_add(in_summaries);

    // Compare against labels.json's cached count. Per locked-decision #8,
    // scanned total wins — we log but don't try to "fix" the index from
    // inside a read-only stats call. The Labels Settings tab has a dedicated
    // Rebuild button for that.
    let index = LabelIndex::load(&*storage)
        .await
        .map_err(|e| e.to_string())?;
    let index_count = index
        .labels
        .iter()
        .find(|e| e.name == name)
        .map(|e| e.count)
        .unwrap_or(0);

    if total != index_count {
        eprintln!(
            "[label-stats] drift detected for {name:?}: scanned={total} index={index_count}"
        );
    }

    Ok(LabelStats {
        total,
        in_notes,
        in_summaries,
        index_count,
    })
}

// ---------------------------------------------------------------------------
// get_notes_for_label (Label Library drill-down)
// ---------------------------------------------------------------------------

/// Which surface a single label reference lives on. Serialized as bare
/// lowercase strings so the frontend can `switch` on the raw value with
/// no mapping layer — `"note"` for a Note's `**Labels:**` line, `"summary"`
/// for a Weekly Summary `### Labels` subsection.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LabelReferenceKind {
    Note,
    Summary,
}

/// A single site where a label appears, enriched with the metadata the
/// Label Library drill-down needs to render a "jump to this note" list
/// entry. For Note references, `note_timestamp` and `note_title` come
/// from the enclosing `### YYYY-MM-DD HH:MM — Title` heading so the user
/// can disambiguate multiple Notes in the same week. For Summary
/// references both fields are `None` (there's only one Weekly Summary
/// per week; the year/week combo is enough).
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LabelReference {
    pub year: u32,
    pub week: u32,
    pub kind: LabelReferenceKind,
    pub note_timestamp: Option<String>,
    pub note_title: Option<String>,
}

/// Walk every weekly file, return one `LabelReference` per site where
/// `name` appears. Ordered newest-first: years descending, weeks
/// descending within a year, and source-document order preserved
/// within a single file (so multiple Note references in the same week
/// list top-to-bottom as written).
///
/// Read-only — never mutates `labels.json`. Per-file read errors are
/// logged via `eprintln` and skipped (locked-decision #7 from Phase
/// 2.8b's rename/delete work).
#[tauri::command]
pub async fn get_notes_for_label(
    storage_state: State<'_, SharedStorage>,
    name: String,
) -> Result<Vec<LabelReference>, String> {
    let storage = storage_state.read().await;
    let mut refs: Vec<LabelReference> = Vec::new();

    walk_all_weeks_descending(&*storage, "label-refs", |year, week, content| {
        for site in scan_label_sites(&content) {
            if !site.names.iter().any(|n| n == &name) {
                continue;
            }
            let reference = match site.kind {
                LabelSiteKind::SummaryLabelsSubsection => LabelReference {
                    year,
                    week,
                    kind: LabelReferenceKind::Summary,
                    note_timestamp: None,
                    note_title: None,
                },
                LabelSiteKind::NoteLabelsLine => {
                    let (ts, title) =
                        extract_note_heading_before(&content, site.byte_range.start)
                            .unwrap_or((String::new(), None));
                    LabelReference {
                        year,
                        week,
                        kind: LabelReferenceKind::Note,
                        note_timestamp: if ts.is_empty() { None } else { Some(ts) },
                        note_title: title,
                    }
                }
            };
            refs.push(reference);
        }
        WalkControl::Continue
    })
    .await?;

    Ok(refs)
}

/// Walk backward from `byte_offset` in `content` and extract the nearest
/// Note heading (`### YYYY-MM-DD HH:MM[ — Title]`). Returns `(timestamp,
/// optional_title)`; the timestamp is whatever text appears between the
/// `### ` prefix and the ` — ` title separator (or end-of-line), verbatim,
/// so it round-trips as-written and the frontend just displays the string.
///
/// Returns `None` when no `### ` heading precedes the offset OR when the
/// nearest one is a Summary subsection (`### Key accomplishments`, etc.)
/// rather than a Note — filtered via `is_iso_date_prefix` on the first
/// ten bytes after the `### ` marker, mirroring how `scan_label_sites`
/// discriminates in the forward direction.
fn extract_note_heading_before(
    content: &str,
    byte_offset: usize,
) -> Option<(String, Option<String>)> {
    if byte_offset > content.len() {
        return None;
    }
    let preceding = &content[..byte_offset];
    // Match `\n### ` so we don't false-match a hash mid-line. Special-case:
    // if the file happens to start with `### ` at byte 0 (no leading
    // newline), we accept that too — rare but valid markdown.
    let heading_line_start = if let Some(idx) = preceding.rfind("\n### ") {
        idx + 1
    } else if preceding.starts_with("### ") {
        0
    } else {
        return None;
    };
    let rest_start = heading_line_start + 4; // skip "### "
    let line_end = content[rest_start..]
        .find('\n')
        .map(|i| rest_start + i)
        .unwrap_or(content.len());
    let heading_rest = &content[rest_start..line_end];

    // Confirm this is a Note heading, not a Summary subsection heading
    // like "### Key accomplishments" or "### Labels" or "### Plans and
    // priorities for next week". Note headings start with an ISO date.
    let iso_check = heading_rest.as_bytes().get(..10)?;
    if !is_iso_date_prefix(iso_check) {
        return None;
    }

    // Parse "YYYY-MM-DD HH:MM[ — Title]". Timestamp = text before ` — `,
    // title = text after (if any). Both trimmed.
    let (ts, title) = if let Some(sep_idx) = heading_rest.find(" — ") {
        let ts = heading_rest[..sep_idx].trim().to_string();
        let title_str = heading_rest[sep_idx + " — ".len()..].trim();
        (
            ts,
            if title_str.is_empty() {
                None
            } else {
                Some(title_str.to_string())
            },
        )
    } else {
        (heading_rest.trim().to_string(), None)
    };
    Some((ts, title))
}

// ---------------------------------------------------------------------------
// search_journal (full-text search)
// ---------------------------------------------------------------------------

/// Discriminates which surface a search result sits on. Serialized as
/// bare lowercase strings so the frontend can switch on the raw value
/// with no mapping layer — matches the `LabelReferenceKind` pattern
/// established by the Label Library drill-down.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SearchResultKind {
    Summary,
    Note,
}

/// A single search hit's context. The frontend re-locates the query
/// within `snippet` for highlighting, so we don't ship match offsets —
/// keeps the payload lean and avoids re-computing positions after the
/// whitespace-collapse step.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SearchSnippet {
    /// ~120 chars of context around the match with the match somewhere
    /// inside. Case-preserved from source; whitespace collapsed so it
    /// renders on one line.
    pub snippet: String,
}

/// One result per surface (Weekly Summary OR individual Note) that
/// contains ≥ 1 matches. `snippets` is capped at `MAX_SNIPPETS_PER_RESULT`
/// for display; the UI shows "and N more matches" when total_matches
/// exceeds the cap.
///
/// `scroll_offset` is the byte offset in the source weekly file the
/// frontend uses to scroll MarkdownEditor to the right location after
/// deep-linking. For Summary results it's 0 (Summary is always at the
/// top of the file); for Note results it's the byte offset of the
/// `### YYYY-MM-DD HH:MM` heading so the user lands at the top of the
/// matching note.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub year: u32,
    pub week: u32,
    pub kind: SearchResultKind,
    /// For Summary results: the `### Labels` subsection labels.
    /// For Note results: the note's own `**Labels:**` line.
    /// Shipped on every result so the frontend can render chips
    /// without a second Tauri round-trip.
    pub labels: Vec<String>,
    /// For Note results: the enclosing heading's timestamp
    /// (e.g., "2026-06-25 14:23"). `None` for Summary results.
    pub note_timestamp: Option<String>,
    /// For Note results: the optional title after " — " in the
    /// heading. `None` for Summary results OR untitled notes.
    pub note_title: Option<String>,
    /// Byte offset in the source weekly file for scroll-to-position
    /// deep-linking. Summary = 0, Note = heading offset.
    pub scroll_offset: u32,
    pub snippets: Vec<SearchSnippet>,
    pub total_matches: u32,
}

/// Substring cap on snippets shipped per result. Set low because the
/// user only needs enough hits to decide "is this the week I meant?"
/// — deeper exploration happens on the /journal page after they click.
const MAX_SNIPPETS_PER_RESULT: usize = 3;

/// Hard cap on per-result match counting. If a Summary contains more
/// than this, we stop counting and cap `total_matches` at MAX — avoids
/// pathological O(n·file_size) scans on a Summary that happens to
/// contain thousands of matches for a 2-char query.
const MAX_MATCHES_PER_RESULT: u32 = 100;

/// Minimum query length. A single-char search on a busy corpus returns
/// noise and stresses the frontend. Two chars is the smallest useful
/// query for names, project keys, etc.
const MIN_QUERY_LENGTH: usize = 2;

/// Hard cap on total results across all weeks. A 2-char query on a
/// dense journal (e.g., "qa" searched by a QA analyst) can produce
/// thousands of matches; each result card renders three snippets +
/// highlighted <mark> elements, and reconciling that DOM tree on the
/// Svelte side can visibly stall the UI. We stop scanning further
/// weeks once we hit this cap; the frontend surfaces a "narrow your
/// query" tip when results.len() equals MAX_RESULTS.
///
/// 200 was chosen empirically — comfortably renders in <1s on the
/// Tauri WebView, still gives users enough hits to feel the shape of
/// their matches without landing them in an unusable wall of text.
const MAX_RESULTS: usize = 200;

/// Full-text search across every weekly file. Scans BOTH the Weekly
/// Summary block (four content fields joined) AND every individual
/// Note's labels-line + body. Case-insensitive substring match.
///
/// Optional label filter narrows to surfaces whose labels contain ≥ 1
/// of the requested labels (OR semantics — any match counts). For
/// Summary results the filter checks the `### Labels` subsection; for
/// Note results it checks the note's `**Labels:**` line. This means a
/// week whose Summary carries a label but whose Notes don't will
/// surface the Summary result but skip its Notes, and vice versa.
///
/// Results are ordered newest-first: years desc, weeks desc within a
/// year; within a week, the Summary (if it matched) comes first, then
/// Notes in document order. Read-only — never mutates.
#[tauri::command]
pub async fn search_journal(
    storage_state: State<'_, SharedStorage>,
    query: String,
    label_filter: Vec<String>,
) -> Result<Vec<SearchResult>, String> {
    let storage = storage_state.read().await;
    search_journal_impl(&*storage, &query, &label_filter).await
}

/// Trait-object-friendly impl seam for `search_journal`. Tests drive
/// this directly against a `LocalFilesystem` — matches the pattern
/// established by `rename_label_impl` / `delete_label_cascade_impl` so
/// the command body doesn't have to be duplicated in the test suite.
pub(crate) async fn search_journal_impl<B: StorageBackend + ?Sized>(
    backend: &B,
    query: &str,
    label_filter: &[String],
) -> Result<Vec<SearchResult>, String> {
    let trimmed = query.trim();
    if trimmed.len() < MIN_QUERY_LENGTH {
        // Empty result on too-short query — the frontend renders an
        // "enter a search term" hint rather than treating this as an error.
        return Ok(Vec::new());
    }
    let needle = trimmed.to_lowercase();

    let label_filter_set: std::collections::HashSet<&str> =
        label_filter.iter().map(String::as_str).collect();
    let use_label_filter = !label_filter_set.is_empty();

    let mut results: Vec<SearchResult> = Vec::new();

    walk_all_weeks_descending(backend, "search", |year, week, content| {
        // ---- Summary surface ------------------------------------
        let summary = parse_weekly_summary(&content);
        let summary_passes_label = !use_label_filter
            || summary
                .labels
                .iter()
                .any(|l| label_filter_set.contains(l.as_str()));

        if summary_passes_label {
            // Concatenate the four content fields with a separator
            // that never appears in the source (double newline).
            let joined = [
                summary.key_accomplishments.as_str(),
                summary.plans_and_priorities.as_str(),
                summary.challenges_or_roadblocks.as_str(),
                summary.anything_else.as_str(),
            ]
            .join("\n\n");

            if let Some((snippets, total)) = scan_matches(&joined, &needle) {
                results.push(SearchResult {
                    year,
                    week,
                    kind: SearchResultKind::Summary,
                    labels: summary.labels.clone(),
                    note_timestamp: None,
                    note_title: None,
                    // Summary sits at the top of every weekly file.
                    // 0 scrolls the editor to origin, which lands
                    // the user on the Summary.
                    scroll_offset: 0,
                    snippets,
                    total_matches: total,
                });
                if results.len() >= MAX_RESULTS {
                    return WalkControl::Stop;
                }
            }
        }

        // ---- Note surfaces --------------------------------------
        // Extract each Note (heading offset + timestamp + title +
        // labels + body-text-haystack) and scan them independently.
        // Notes are returned in document order (top-to-bottom of
        // the "## Weekly Notes" section).
        for note in extract_notes_for_search(&content) {
            let note_passes_label = !use_label_filter
                || note
                    .labels
                    .iter()
                    .any(|l| label_filter_set.contains(l.as_str()));
            if !note_passes_label {
                continue;
            }
            if let Some((snippets, total)) = scan_matches(&note.haystack, &needle) {
                results.push(SearchResult {
                    year,
                    week,
                    kind: SearchResultKind::Note,
                    labels: note.labels,
                    note_timestamp: Some(note.timestamp),
                    note_title: note.title,
                    scroll_offset: note.heading_offset as u32,
                    snippets,
                    total_matches: total,
                });
                if results.len() >= MAX_RESULTS {
                    return WalkControl::Stop;
                }
            }
        }

        WalkControl::Continue
    })
    .await?;

    Ok(results)
}

/// Run the substring scan over a single haystack. Returns
/// `(snippets, total_matches)` when at least one match landed, or
/// `None` when the haystack contains nothing. Shared between the
/// Summary and Note code paths so the cursor-advance logic can't
/// drift between them.
fn scan_matches(haystack: &str, needle_lower: &str) -> Option<(Vec<SearchSnippet>, u32)> {
    let haystack_lower = haystack.to_lowercase();
    let mut snippets: Vec<SearchSnippet> = Vec::new();
    let mut total: u32 = 0;
    let mut cursor: usize = 0;
    while let Some(rel_idx) = haystack_lower[cursor..].find(needle_lower) {
        let match_start = cursor + rel_idx;
        let match_end = match_start + needle_lower.len();
        total = total.saturating_add(1);
        if snippets.len() < MAX_SNIPPETS_PER_RESULT {
            snippets.push(SearchSnippet {
                snippet: build_snippet(haystack, match_start, match_end),
            });
        }
        cursor = match_end.max(cursor + 1);
        if total >= MAX_MATCHES_PER_RESULT {
            break;
        }
    }
    if total == 0 {
        None
    } else {
        Some((snippets, total))
    }
}

/// A single Note extracted from a weekly file, in the shape the search
/// walk needs: heading offset for scroll-to, metadata for the result
/// card, labels for filter-checking, and a plain-text haystack for the
/// substring scan.
///
/// The haystack includes the labels line (if any) and the free-text
/// body — but NOT the heading itself. Excluding the heading avoids
/// noisy matches where a user searches for a date like "2026-06-25"
/// and hits every Note's timestamp; date-based navigation is a
/// separate concern (the `/journal` sidebar tree).
struct ParsedNoteForSearch {
    /// Byte offset of the `### ` prefix in the source file. Passed to
    /// the frontend as `scroll_offset` so MarkdownEditor scrolls the
    /// user to the top of the matching note.
    heading_offset: usize,
    /// "YYYY-MM-DD HH:MM" as written on the heading line.
    timestamp: String,
    /// Optional " — Title" tail from the heading line.
    title: Option<String>,
    /// Labels from the note's `**Labels:**` line, if present.
    labels: Vec<String>,
    /// Text to substring-scan: labels line + body, concatenated.
    haystack: String,
}

/// Extract every Note from a weekly file for search purposes. Walks
/// the raw markdown from the `## Weekly Notes` header (or the start
/// of the file if the Summary is absent) forward, treating each
/// `### YYYY-MM-DD HH:MM` heading as the start of a new note.
///
/// Note boundaries: heading through (next-heading OR end-of-file).
/// Distinguishes Note headings from Summary subsection headings via
/// the ISO-date-prefix check that `scan_label_sites` uses forward-
/// direction. Malformed / unparseable headings are skipped silently
/// (never panic on user content).
fn extract_notes_for_search(content: &str) -> Vec<ParsedNoteForSearch> {
    let mut notes = Vec::new();
    // Anchor the walk at the "## Weekly Notes" section start so a
    // Summary subsection heading like "### Key accomplishments" can't
    // be mistaken for a Note. If the marker is missing (empty week
    // file, hand-authored variant), fall back to scanning the whole
    // file — the ISO-date-prefix guard filters non-Note headings.
    let mut search_from = content.find("\n## Weekly Notes").map(|i| i + 1).unwrap_or(0);

    while let Some(rel) = content[search_from..].find("\n### ") {
        let heading_line_start = search_from + rel + 1; // skip the '\n'
        let rest_start = heading_line_start + 4; // skip "### "
        let line_end = content[rest_start..]
            .find('\n')
            .map(|i| rest_start + i)
            .unwrap_or(content.len());
        let heading_rest = &content[rest_start..line_end];

        // Advance the outer cursor past this heading regardless of
        // whether it was a real Note — matches scan_label_sites'
        // posture on non-Note `###` headings.
        search_from = line_end;

        // Only ISO-date-prefixed headings are Notes.
        let iso_check = match heading_rest.as_bytes().get(..10) {
            Some(bytes) => bytes,
            None => continue,
        };
        if !is_iso_date_prefix(iso_check) {
            continue;
        }

        // Parse "YYYY-MM-DD HH:MM[ — Title]".
        let (timestamp, title) = if let Some(sep_idx) = heading_rest.find(" — ") {
            let ts = heading_rest[..sep_idx].trim().to_string();
            let title_str = heading_rest[sep_idx + " — ".len()..].trim();
            (
                ts,
                if title_str.is_empty() {
                    None
                } else {
                    Some(title_str.to_string())
                },
            )
        } else {
            (heading_rest.trim().to_string(), None)
        };

        // Body window: from line_end to the next `\n### ` OR EOF.
        // Includes the trailing newline of the heading and everything
        // up to (not including) the next heading's leading newline.
        let body_end = content[line_end..]
            .find("\n### ")
            .map(|i| line_end + i)
            .unwrap_or(content.len());
        let body_window = &content[line_end..body_end];

        // Pull the labels line if present. The extractor scans the
        // first few non-empty lines of the body_window for a line
        // starting with `**Labels:**`.
        let mut labels: Vec<String> = Vec::new();
        for line in body_window.lines().take(4) {
            let trimmed = line.trim_start();
            if let Some(rest) = trimmed.strip_prefix("**Labels:**") {
                for token in rest.split_whitespace() {
                    if let Some(stripped) = token.strip_prefix('#') {
                        if !stripped.is_empty() {
                            labels.push(stripped.to_string());
                        }
                    }
                }
                break;
            }
            if trimmed.is_empty() {
                continue;
            }
            // Hit a non-empty non-labels line — no labels for this note.
            break;
        }

        // Haystack: body_window verbatim. Includes labels-line + body
        // text so a search for "release" finds both a "#release" tag
        // AND the word "release" in prose. Excludes the heading (see
        // the ParsedNoteForSearch doc comment for the rationale).
        let haystack = body_window.to_string();

        notes.push(ParsedNoteForSearch {
            heading_offset: heading_line_start,
            timestamp,
            title,
            labels,
            haystack,
        });
    }

    notes
}

/// Build a ~120-char snippet centered on the match. Whitespace inside
/// the snippet is collapsed to single spaces so the whole thing fits
/// on one row in the UI. Ellipses are added when the snippet doesn't
/// cover the start / end of the source respectively.
///
/// Snippet slicing walks to a char boundary before slicing to avoid
/// panicking on multi-byte UTF-8 (emoji, accented chars). The frontend
/// re-finds the match position within the returned snippet — we don't
/// ship offsets because whitespace collapse would have invalidated them.
fn build_snippet(source: &str, match_start: usize, match_end: usize) -> String {
    const HALF_WIDTH: usize = 60;

    let raw_start = match_start.saturating_sub(HALF_WIDTH);
    let raw_end = (match_end + HALF_WIDTH).min(source.len());

    // Walk forward to a char boundary at the start, backward at the end.
    let mut safe_start = raw_start;
    while safe_start < source.len() && !source.is_char_boundary(safe_start) {
        safe_start += 1;
    }
    let mut safe_end = raw_end;
    while safe_end > 0 && !source.is_char_boundary(safe_end) {
        safe_end -= 1;
    }
    if safe_end < match_end {
        // Extremely defensive — a bad boundary walk could have pulled
        // safe_end back past the match itself. Prefer over-inclusion.
        safe_end = source.len();
    }

    let slice = &source[safe_start..safe_end];

    // Collapse whitespace (single/multi-line) to single spaces for
    // display. split_whitespace already handles \n, \t, and multiple
    // spaces uniformly.
    let collapsed: String = slice.split_whitespace().collect::<Vec<_>>().join(" ");

    // Ellipsis prefixes/suffixes indicate the source extended past our
    // window on that side.
    let mut out = String::with_capacity(collapsed.len() + 2);
    if safe_start > 0 {
        out.push('…');
    }
    out.push_str(&collapsed);
    if safe_end < source.len() {
        out.push('…');
    }
    out
}

// ---------------------------------------------------------------------------
// rename_label
// ---------------------------------------------------------------------------

/// Result of a [`rename_label`] pass. `files_modified` counts weekly files we
/// successfully spliced at least one replacement into; `occurrences_replaced`
/// counts individual `#oldname` tokens turned into `#newname` across all
/// sites; `failed_files` lists weekly files that errored while we tried to
/// read or write them — per locked-decision #7, we don't roll back the rest
/// of the pass on partial failure, we just surface what couldn't be touched.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameResult {
    pub files_modified: u32,
    pub occurrences_replaced: u32,
    pub failed_files: Vec<String>,
}

/// Rename a label across every weekly file's explicit-labels sites AND
/// `labels.json`. Surface for the Labels Settings tab's per-row rename
/// affordance.
///
/// Validation:
///   - `old_name == new_name` after `#` strip → `Err("rename is a no-op")`.
///   - Empty `new_name` (after `#` strip + trim) → `Err`.
///   - Any byte in `new_name` outside `is_label_char` (alphanumeric + `_` +
///     `-`) → `Err` naming the offending value, mirroring how
///     `set_label_color`'s hex validator names the bad input.
///
/// Workflow per locked-decision #2 + #9:
///   1. Walk every weekly file. For each, scan explicit-labels sites
///      (`scan_label_sites` → both Note `**Labels:**` lines and Summary
///      `### Labels` subsections — inline `#hashtag` text in prose stays
///      put). Splice in a rebuilt chunk with `old_name` swapped for
///      `new_name`. Process sites in REVERSE byte order so earlier ranges
///      stay valid as we splice.
///   2. Dedup within a single chunk: if `new_name` already appears on the
///      same labels line / subsection, drop the renamed copy. The pre-
///      existing destination wins (locked-decision #5 mirrors this for color
///      overrides; same intent here for token position).
///   3. Update `labels.json`: if `new_name` is absent, rename the entry in
///      place (preserve count + first_used + last_used + color). If it
///      exists, MERGE — sum counts, take min(first_used) + max(last_used),
///      and keep the destination's existing color override (locked
///      decision #5).
///
/// Per-file read/write errors are logged via `eprintln` and pushed into
/// `failed_files` — they never abort the rename pass. The labels.json save
/// at the end IS fatal; the caller's UI needs to know the index didn't land.
#[tauri::command]
pub async fn rename_label(
    storage_state: State<'_, SharedStorage>,
    old_name: String,
    new_name: String,
) -> Result<RenameResult, String> {
    let storage = storage_state.write().await;
    rename_label_impl(&*storage, &old_name, &new_name).await
}

/// Backend of [`rename_label`], factored out so the unit tests can drive it
/// against a `LocalFilesystem` directly without standing up the Tauri
/// `State` machinery (matches the pattern used by `set_label_color_impl`
/// and `rebuild_label_index_impl`).
pub(crate) async fn rename_label_impl<B: StorageBackend + ?Sized>(
    backend: &B,
    old_name: &str,
    new_name: &str,
) -> Result<RenameResult, String> {
    // Strip a leading `#` from either side so callers can pass either form
    // (`release` or `#release`) — the rest of the impl works with bare
    // names, matching what's stored in `LabelEntry::name` and what
    // `LabelSite::names` carries.
    let old_clean = old_name.trim().trim_start_matches('#');
    let new_clean = new_name.trim().trim_start_matches('#');

    if new_clean.is_empty() {
        return Err("new label name must not be empty".to_string());
    }
    if old_clean == new_clean {
        return Err("rename is a no-op".to_string());
    }
    // Reuse the `is_label_char` rule (alphanumeric + `_` + `-`) — locked
    // decision #9. Inline here rather than re-exporting from labels.rs for
    // a single command-side check, mirroring `is_hex6_color` above.
    if let Some(bad) = new_clean.bytes().find(|b| !is_label_char_byte(*b)) {
        return Err(format!(
            "new label name {new_clean:?} contains invalid character {:?}; \
             allowed: alphanumeric, '_', '-'",
            bad as char
        ));
    }

    let mut files_modified: u32 = 0;
    let mut occurrences_replaced: u32 = 0;
    let mut failed_files: Vec<String> = Vec::new();

    // Walk every weekly file and rewrite explicit-labels sites in place.
    let years = backend.list_years().await.map_err(|e| e.to_string())?;
    for year in years {
        let weeks = match backend.list_weeks(year).await {
            Ok(w) => w,
            Err(e) => {
                eprintln!("[rename_label] list_weeks({year}) failed: {e}");
                continue;
            }
        };
        for week in weeks {
            let pretty_path = format!("{year:04}/{year:04}-W{week:02}.md");
            let mut content = match backend.read_week(year, week).await {
                Ok(Some(c)) => c,
                Ok(None) => continue,
                Err(e) => {
                    eprintln!("[rename_label] read_week({year},{week}) failed: {e}");
                    failed_files.push(pretty_path);
                    continue;
                }
            };

            // Pre-scan: how many sites in this file actually mention
            // old_clean? Consume `sites` with .into_iter() so we end
            // up with owned LabelSite values — no borrow on content
            // persists, and we can splice content in place without
            // a full-file .clone() (previously we cloned content into
            // `rewritten` just to sidestep the borrow checker).
            let sites = scan_label_sites(&content);
            let mut ordered: Vec<LabelSite> = sites
                .into_iter()
                .filter(|s| s.names.iter().any(|n| n == old_clean))
                .collect();
            if ordered.is_empty() {
                continue;
            }

            // Sort touched sites by descending start so splices high-to-low
            // don't invalidate the indices of the ones still to come.
            ordered.sort_by_key(|s| std::cmp::Reverse(s.byte_range.start));

            let mut per_file_replacements: u32 = 0;
            for site in ordered {
                let (new_chunk, replaced) = rebuild_chunk_for_rename(
                    &content[site.byte_range.clone()],
                    &site.names,
                    old_clean,
                    new_clean,
                    site.kind,
                );
                per_file_replacements = per_file_replacements.saturating_add(replaced);
                content.replace_range(site.byte_range, &new_chunk);
            }

            if per_file_replacements == 0 {
                // Defensive: a touched site that produced no replacement
                // means rebuild_chunk_for_rename failed to find the token —
                // log and skip rather than writing an identical file.
                continue;
            }

            match backend.write_week(year, week, &content).await {
                Ok(()) => {
                    files_modified = files_modified.saturating_add(1);
                    occurrences_replaced =
                        occurrences_replaced.saturating_add(per_file_replacements);
                }
                Err(e) => {
                    eprintln!("[rename_label] write_week({year},{week}) failed: {e}");
                    failed_files.push(pretty_path);
                }
            }
        }
    }

    // Update labels.json. Three branches:
    //   - old missing entirely → no-op on the index (still report the file
    //     work we did; this can legitimately happen if the user typed a
    //     name that only lives in inline-prose hashtags, which we don't
    //     touch).
    //   - new missing → rename in place; count + dates + color preserved.
    //   - both present → merge into destination; sum counts, min/max
    //     dates, keep destination's color (locked-decision #5).
    let mut index = LabelIndex::load(backend).await.map_err(|e| e.to_string())?;
    let old_pos = index.labels.iter().position(|e| e.name == old_clean);
    let new_pos = index.labels.iter().position(|e| e.name == new_clean);
    match (old_pos, new_pos) {
        (None, _) => {
            // Nothing to update on the index side — the file rewrites may
            // still have happened (e.g. if labels.json drifted behind disk)
            // but there's no entry to rename here.
        }
        (Some(o), None) => {
            index.labels[o].name = new_clean.to_string();
        }
        (Some(o), Some(n)) if o == n => {
            // Shouldn't be reachable — old_clean != new_clean is enforced
            // up top — but treat as no-op rather than panic if it ever is.
        }
        (Some(o), Some(n)) => {
            // Merge `o` into `n`. Read both before mutating so we don't
            // hold overlapping borrows.
            let merged_count = index.labels[n]
                .count
                .saturating_add(index.labels[o].count);
            let merged_first = index.labels[n].first_used.min(index.labels[o].first_used);
            let merged_last = index.labels[n].last_used.max(index.labels[o].last_used);
            index.labels[n].count = merged_count;
            index.labels[n].first_used = merged_first;
            index.labels[n].last_used = merged_last;
            // Color: locked-decision #5 — destination's existing override
            // wins. If destination had no color, we still don't promote the
            // source's color (the merge target is the canonical name now).
            // Remove the old entry. The index's `iter().position` returned
            // `o` against the pre-mutation order, so re-resolve by name to
            // avoid index-shift bugs if the two entries happened to be
            // adjacent and Vec::remove shifted things underneath us.
            if let Some(pos) = index.labels.iter().position(|e| e.name == old_clean) {
                index.labels.remove(pos);
            }
        }
    }
    index.save(backend).await.map_err(|e| e.to_string())?;

    Ok(RenameResult {
        files_modified,
        occurrences_replaced,
        failed_files,
    })
}

/// Local mirror of `labels::is_label_char` so the rename validator can stay
/// inside commands.rs without widening the labels module's public surface.
/// Kept in lockstep with the version in labels.rs — alphanumeric + `_` +
/// `-` per locked-decision #9.
fn is_label_char_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
}

/// Rewrite a single label site's chunk so that every `#old` token becomes
/// `#new`, dedup-ing against any pre-existing `#new` token already on the
/// same line/subsection. Returns the new chunk plus a count of how many
/// `#old` tokens were rewritten (or dropped, in the dedup case — both count
/// as "occurrences replaced" for the reporting struct).
///
/// We rebuild the chunk from the parsed `names` list rather than doing a
/// byte-level `replace("#old", "#new")`, which would mis-handle prefix
/// overlaps (`#release` inside `#release-train`) and structural whitespace.
/// The site's trailing whitespace — the part of the byte range past the
/// last token — is preserved verbatim so a Summary subsection's blank-line
/// gap before the next heading survives the splice.
fn rebuild_chunk_for_rename(
    original: &str,
    names: &[String],
    old: &str,
    new: &str,
    kind: LabelSiteKind,
) -> (String, u32) {
    // Apply the rename + dedup to the names list. Track how many `#old`
    // tokens we touched (replaced into `#new`, or dropped when `#new`
    // already existed earlier in the same chunk).
    let mut new_names: Vec<String> = Vec::with_capacity(names.len());
    let mut replaced: u32 = 0;
    for name in names {
        if name == old {
            if new_names.iter().any(|n| n == new) {
                // `#new` already on this chunk before the rename hit `#old`.
                // Drop the renamed token — destination's original position
                // wins.
                replaced = replaced.saturating_add(1);
            } else {
                new_names.push(new.to_string());
                replaced = replaced.saturating_add(1);
            }
        } else if name == new {
            // The destination was already here. Keep it; subsequent
            // `#old` occurrences will dedup against it.
            if !new_names.iter().any(|n| n == new) {
                new_names.push(new.to_string());
            }
        } else {
            new_names.push(name.clone());
        }
    }

    match kind {
        LabelSiteKind::NoteLabelsLine => {
            // Shape per `Note::to_markdown`: `**Labels:** #a #b\n`. Preserve
            // the trailing-newline-or-EOF state of the original chunk so a
            // labels line that sat at the very end of a file (no trailing
            // `\n` per `scan_label_sites`) doesn't gain one.
            let trailing = if original.ends_with('\n') { "\n" } else { "" };
            let mut s = String::from("**Labels:**");
            for n in &new_names {
                s.push_str(" #");
                s.push_str(n);
            }
            s.push_str(trailing);
            (s, replaced)
        }
        LabelSiteKind::SummaryLabelsSubsection => {
            // Shape per `render_weekly_summary`: `### Labels\n#a #b\n` then
            // any trailing whitespace up to the next heading. Slice the
            // heading line and the trailing-whitespace region out of the
            // original chunk so the rebuilt body slots cleanly back in.
            let heading_end = original
                .find('\n')
                .map(|i| i + 1)
                .unwrap_or(original.len());
            let heading = &original[..heading_end]; // "### Labels\n"
            let rest = &original[heading_end..];
            // Body line ends at the next `\n` (or EOF for an exotic file
            // that ends mid-subsection). Everything past that newline is
            // trailing whitespace we want to preserve verbatim.
            let body_end = rest.find('\n').map(|i| i + 1).unwrap_or(rest.len());
            let trailing = &rest[body_end..];

            let new_body = new_names
                .iter()
                .map(|n| format!("#{n}"))
                .collect::<Vec<_>>()
                .join(" ");
            // Always terminate the body line with `\n` so the trailing
            // whitespace region (which may be empty, or may be a blank line
            // before the next heading) keeps its structural meaning.
            let mut out = String::with_capacity(original.len());
            out.push_str(heading);
            out.push_str(&new_body);
            out.push('\n');
            out.push_str(trailing);
            (out, replaced)
        }
    }
}

// ---------------------------------------------------------------------------
// delete_label_cascade
// ---------------------------------------------------------------------------

/// Result of a [`delete_label_cascade`] pass. `files_modified` counts weekly
/// files we successfully rewrote (i.e. at least one labels-array entry was
/// stripped); `occurrences_removed` counts individual `#name` tokens dropped
/// across all explicit-labels sites; `failed_files` lists weekly files that
/// errored during read/write — per locked-decision #7, partial failure does
/// not roll back the rest of the pass, we just surface what couldn't be
/// touched.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteResult {
    pub files_modified: u32,
    pub occurrences_removed: u32,
    pub failed_files: Vec<String>,
}

/// Strip a label from every weekly file's explicit-labels sites AND remove
/// the entry from `labels.json`. Surface for the Labels Settings tab's
/// per-row delete affordance.
///
/// Per locked-decision #2, the cascade is SCOPE-LIMITED to explicit
/// `labels:[...]` arrays — meaning the Note `**Labels:**` line and the
/// Weekly Summary `### Labels` subsection. Inline `#hashtag` text in note
/// bodies or summary prose is left byte-for-byte untouched; the details
/// popup surfaces this with a "deletes from labels arrays only" tip.
///
/// Empty-after-delete behaviour (locked-decision #6):
///   - Note `**Labels:**` line goes empty → drop the whole line (so the
///     post-delete file doesn't carry a bare `**Labels:**` with no chips).
///   - Summary `### Labels` subsection goes empty → keep the `### Labels`
///     header and emit an empty body line, matching the empty-summary
///     scaffold (`### Labels\n\n`). The header anchors the section so the
///     user can immediately add labels again without re-typing the heading.
///
/// Per-file read/write errors are logged via `eprintln` and pushed into
/// `failed_files`; they never abort the cascade. The labels.json save at the
/// end IS fatal — the caller's UI needs to know the index didn't land.
#[tauri::command]
pub async fn delete_label_cascade(
    storage_state: State<'_, SharedStorage>,
    name: String,
) -> Result<DeleteResult, String> {
    let storage = storage_state.write().await;
    delete_label_cascade_impl(&*storage, &name).await
}

/// Backend of [`delete_label_cascade`], factored out so unit tests can drive
/// it against a `LocalFilesystem` directly (matches `rename_label_impl` and
/// `rebuild_label_index_impl`).
pub(crate) async fn delete_label_cascade_impl<B: StorageBackend + ?Sized>(
    backend: &B,
    name: &str,
) -> Result<DeleteResult, String> {
    // Accept either `name` or `#name` form so callers can pass whichever they
    // had in hand — the rest of the impl works on the bare name (matching
    // `LabelEntry::name` / `LabelSite::names`).
    let clean = name.trim().trim_start_matches('#').to_string();
    if clean.is_empty() {
        return Err("label name must not be empty".to_string());
    }

    let mut files_modified: u32 = 0;
    let mut occurrences_removed: u32 = 0;
    let mut failed_files: Vec<String> = Vec::new();

    // Walk every weekly file and rewrite explicit-labels sites in place.
    let years = backend.list_years().await.map_err(|e| e.to_string())?;
    for year in years {
        let weeks = match backend.list_weeks(year).await {
            Ok(w) => w,
            Err(e) => {
                eprintln!("[delete_label_cascade] list_weeks({year}) failed: {e}");
                continue;
            }
        };
        for week in weeks {
            let pretty_path = format!("{year:04}/{year:04}-W{week:02}.md");
            let mut content = match backend.read_week(year, week).await {
                Ok(Some(c)) => c,
                Ok(None) => continue,
                Err(e) => {
                    eprintln!("[delete_label_cascade] read_week({year},{week}) failed: {e}");
                    failed_files.push(pretty_path);
                    continue;
                }
            };

            // Pre-scan: which sites in this file actually carry `clean`?
            // Consume `sites` with .into_iter() so we end up with owned
            // LabelSite values — no borrow on content persists, and we
            // can splice content in place without a full-file .clone().
            let sites = scan_label_sites(&content);
            let mut ordered: Vec<LabelSite> = sites
                .into_iter()
                .filter(|s| s.names.iter().any(|n| n == &clean))
                .collect();
            if ordered.is_empty() {
                continue;
            }

            // Sort touched sites by descending start so splices high-to-low
            // don't invalidate the indices of the ones still to come.
            ordered.sort_by_key(|s| std::cmp::Reverse(s.byte_range.start));

            let mut per_file_removed: u32 = 0;
            for site in ordered {
                let (new_chunk, removed) = rebuild_chunk_for_delete(
                    &content[site.byte_range.clone()],
                    &site.names,
                    &clean,
                    site.kind,
                );
                per_file_removed = per_file_removed.saturating_add(removed);
                content.replace_range(site.byte_range, &new_chunk);
            }

            if per_file_removed == 0 {
                // Defensive: scan_label_sites said this file held `clean` but
                // rebuild_chunk_for_delete reported no removals. Skip the
                // write rather than touching an identical file.
                continue;
            }

            match backend.write_week(year, week, &content).await {
                Ok(()) => {
                    files_modified = files_modified.saturating_add(1);
                    occurrences_removed =
                        occurrences_removed.saturating_add(per_file_removed);
                }
                Err(e) => {
                    eprintln!(
                        "[delete_label_cascade] write_week({year},{week}) failed: {e}"
                    );
                    failed_files.push(pretty_path);
                }
            }
        }
    }

    // Remove the entry from labels.json. Idempotent on missing — the user
    // may have hit Delete on an entry that only existed as inline-prose
    // hashtags (which we don't touch), or the index may have drifted behind
    // disk. Either way the file rewrites above already did the work; the
    // index update is a no-op for missing entries.
    let mut index = LabelIndex::load(backend).await.map_err(|e| e.to_string())?;
    if let Some(pos) = index.labels.iter().position(|e| e.name == clean) {
        index.labels.remove(pos);
    }
    index.save(backend).await.map_err(|e| e.to_string())?;

    Ok(DeleteResult {
        files_modified,
        occurrences_removed,
        failed_files,
    })
}

/// Rewrite a single label site's chunk with `target` removed. Returns the
/// new chunk plus the count of `#target` tokens dropped.
///
/// Shape rules per locked-decision #6:
///   - `NoteLabelsLine`: rebuild as `**Labels:** #a #b\n`. If the names list
///     goes empty, return `""` so the splice DROPS the whole line.
///   - `SummaryLabelsSubsection`: rebuild as `### Labels\n<#tokens or empty>\n<trailing>`.
///     If names go empty, keep the `### Labels\n` header and emit an empty
///     body line (`### Labels\n\n` shape), matching the empty-summary
///     scaffold so the Settings details popup's "stays empty" note holds.
fn rebuild_chunk_for_delete(
    original: &str,
    names: &[String],
    target: &str,
    kind: LabelSiteKind,
) -> (String, u32) {
    let mut new_names: Vec<String> = Vec::with_capacity(names.len());
    let mut removed: u32 = 0;
    for name in names {
        if name == target {
            removed = removed.saturating_add(1);
        } else {
            new_names.push(name.clone());
        }
    }

    match kind {
        LabelSiteKind::NoteLabelsLine => {
            // Empty after delete: per locked-decision #6, drop the entire
            // `**Labels:**` line — returning "" splices the byte range out,
            // which includes the trailing newline (or EOF) captured by
            // scan_label_sites.
            if new_names.is_empty() {
                return (String::new(), removed);
            }
            // Preserve the trailing-newline-or-EOF state of the original
            // chunk so a labels line sitting at the very end of a file
            // doesn't gain one.
            let trailing = if original.ends_with('\n') { "\n" } else { "" };
            let mut s = String::from("**Labels:**");
            for n in &new_names {
                s.push_str(" #");
                s.push_str(n);
            }
            s.push_str(trailing);
            (s, removed)
        }
        LabelSiteKind::SummaryLabelsSubsection => {
            // Shape per `render_weekly_summary`: `### Labels\n<body>\n<trailing>`.
            // We slice out the heading line and the trailing whitespace
            // region so the rebuilt body slots cleanly back in. Per
            // locked-decision #6, the heading STAYS even when the body
            // empties out — the section anchors the user's next add.
            let heading_end = original
                .find('\n')
                .map(|i| i + 1)
                .unwrap_or(original.len());
            let heading = &original[..heading_end]; // "### Labels\n"
            let rest = &original[heading_end..];
            let body_end = rest.find('\n').map(|i| i + 1).unwrap_or(rest.len());
            let trailing = &rest[body_end..];

            let new_body = new_names
                .iter()
                .map(|n| format!("#{n}"))
                .collect::<Vec<_>>()
                .join(" ");
            let mut out = String::with_capacity(original.len());
            out.push_str(heading);
            out.push_str(&new_body);
            // Always terminate the body line with `\n` so the trailing
            // whitespace region (which may be empty, or may be a blank line
            // before the next heading) keeps its structural meaning. When
            // new_body is empty this collapses to `### Labels\n\n<trailing>`,
            // which matches the empty-summary scaffold.
            out.push('\n');
            out.push_str(trailing);
            (out, removed)
        }
    }
}

// ---------------------------------------------------------------------------
// Weekly Summary
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YearWeek {
    pub year: u32,
    pub week: u32,
}

/// Return the current ISO year + week as a single struct. Used by the
/// frontend to know which weekly file to load.
#[tauri::command]
pub fn get_current_year_week() -> YearWeek {
    let (year, week) = iso_year_week(Local::now().date_naive());
    YearWeek { year, week }
}

/// Read and parse the Weekly Summary for a given (year, week). Returns
/// an empty/default summary when the weekly file doesn't exist yet — the
/// frontend can render the empty form without distinguishing first-write
/// from existing-file-with-no-summary.
#[tauri::command]
pub async fn get_weekly_summary(
    storage_state: State<'_, SharedStorage>,
    year: u32,
    week: u32,
) -> Result<WeeklySummary, String> {
    let storage = storage_state.read().await;
    let content = storage
        .read_week(year, week)
        .await
        .map_err(|e| e.to_string())?;
    Ok(match content {
        Some(c) => parse_weekly_summary(&c),
        None => WeeklySummary::default(),
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateWeeklySummaryInput {
    pub year: u32,
    pub week: u32,
    pub key_accomplishments: String,
    pub plans_and_priorities: String,
    pub challenges_or_roadblocks: String,
    pub anything_else: String,
    #[serde(default)]
    pub labels: Vec<String>,
}

/// Splice the user's edits back into the weekly file, preserving everything
/// outside the Weekly Summary section (frontmatter, week heading, Weekly Notes
/// with their captured notes). If the file doesn't exist yet, creates the
/// scaffold first and then splices.
///
/// `last_updated` is stamped server-side with the local clock — the frontend
/// doesn't send it.
#[tauri::command]
pub async fn update_weekly_summary(
    app: AppHandle,
    storage_state: State<'_, SharedStorage>,
    input: UpdateWeeklySummaryInput,
) -> Result<(), String> {
    let now = Local::now().fixed_offset();
    let new_summary = WeeklySummary {
        key_accomplishments: input.key_accomplishments,
        plans_and_priorities: input.plans_and_priorities,
        challenges_or_roadblocks: input.challenges_or_roadblocks,
        anything_else: input.anything_else,
        labels: input
            .labels
            .into_iter()
            .map(|l| l.trim().trim_start_matches('#').to_string())
            .filter(|l| !l.is_empty())
            .collect(),
        last_updated: Some(now.format("%Y-%m-%d %H:%M").to_string()),
    };

    let storage = storage_state.read().await;

    let existing = storage
        .read_week(input.year, input.week)
        .await
        .map_err(|e| e.to_string())?;

    let updated = match existing {
        Some(content) => replace_weekly_summary_in_file(&content, &new_summary),
        None => {
            let scaffold = weekly_file_scaffold(input.year, input.week, now);
            replace_weekly_summary_in_file(&scaffold, &new_summary)
        }
    };

    storage
        .write_week(input.year, input.week, &updated)
        .await
        .map_err(|e| e.to_string())?;

    emit_weekly_file_changed(&app, input.year, input.week);

    Ok(())
}

// ---------------------------------------------------------------------------
// Weekly file change event
// ---------------------------------------------------------------------------

/// Payload broadcast on the `weekly-file-changed` event. Frontend routes
/// listen for this and reload their in-memory copy when the (year, week)
/// they have open matches.
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct WeeklyFileChanged {
    year: u32,
    week: u32,
}

/// Broadcast a "this weekly file just changed" notification to all
/// frontend windows. Listeners on /journal and /summary use it to
/// reconcile their in-memory copy when a sibling route (or the menu-bar
/// /capture popup) writes to the same week.
///
/// Errors from `emit` are swallowed — failing to notify shouldn't fail
/// the underlying write that already succeeded on disk.
fn emit_weekly_file_changed(app: &AppHandle, year: u32, week: u32) {
    let _ = app.emit("weekly-file-changed", WeeklyFileChanged { year, week });
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

/// What the frontend sees when querying settings state.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsBundle {
    /// `true` when app-settings.json doesn't exist yet — the wizard should render.
    pub first_run: bool,
    /// The currently-active journal root (default on first run; configured otherwise).
    pub journal_root: PathBuf,
    /// The recommended default location for the first-run picker.
    pub default_journal_root: PathBuf,
    /// The user's display name, if set.
    pub user_name: Option<String>,
    /// The user's own email — pins Gmail account in Gmail mode, sets
    /// outgoing sender in Native Mac Mail mode. Optional.
    pub user_email: Option<String>,
    /// Reminder preferences.
    pub reminder: ReminderSettings,
    /// Active theme — defaults to Dark, persisted in app-settings.json.
    pub theme: Theme,
    /// User's Custom theme primaries (Phase 2.8). `None` until the user
    /// saves a Custom theme for the first time. Surfaced even when
    /// `theme != Custom` so the Settings panel can pre-populate the
    /// Custom editor with the user's last-saved palette.
    pub custom_theme: Option<CustomTheme>,
    /// Manager's email — used by the "Send weekly summary to manager" flow.
    pub manager_email: Option<String>,
    /// Manager's display name — used to personalize the email greeting.
    pub manager_name: Option<String>,
    /// Job title (as it appears in BambooHR). Optional.
    pub bamboo_title: Option<String>,
    /// Jira project keys the user is associated with. Empty vec = none set.
    pub jira_project_keys: Vec<String>,
    /// Which Send-to-manager path the user has chosen (defaults to Gmail).
    pub mail_send_mode: MailSendMode,
    /// Plaintext flavor for Gmail/Outlook and non-HTML Native Mac Mail.
    pub mail_body_format: MailBodyFormat,
    /// Native Mac Mail HTML toggle (ignored outside NativeMail mode).
    pub mail_native_html: bool,
    /// Outlook host flavor (Business vs Personal).
    pub mail_outlook_flavor: OutlookFlavor,
    /// How the body reaches the compose window (Prefilled in the URL vs
    /// ClipboardPaste — empty compose, rich HTML on the clipboard).
    pub mail_body_delivery: MailBodyDelivery,
    /// Phase 2.8+ "Colorful Labels": render label chips with their
    /// per-label color instead of the flat theme accent. Defaults to
    /// false on a fresh install; the user opts in from Settings > Theme.
    pub colorful_labels: bool,
}

#[tauri::command]
pub async fn get_settings(
    app: AppHandle,
    storage_state: State<'_, SharedStorage>,
) -> Result<SettingsBundle, String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;

    let app_settings = AppSettings::load(&app_data_dir)
        .await
        .map_err(|e| e.to_string())?;

    let storage = storage_state.read().await;

    let journal_settings = JournalSettings::load(&*storage)
        .await
        .map_err(|e| e.to_string())?;

    let journal_root = app_settings
        .as_ref()
        .map(|s| s.journal_root.clone())
        .unwrap_or_else(|| storage.root().to_path_buf());
    let theme = app_settings.as_ref().map(|s| s.theme).unwrap_or_default();
    let custom_theme = app_settings.as_ref().and_then(|s| s.custom_theme.clone());

    Ok(SettingsBundle {
        first_run: app_settings.is_none(),
        journal_root,
        default_journal_root: default_journal_root(),
        user_name: journal_settings.user_name,
        user_email: journal_settings.user_email,
        reminder: journal_settings.reminder,
        theme,
        custom_theme,
        manager_email: journal_settings.manager_email,
        manager_name: journal_settings.manager_name,
        bamboo_title: journal_settings.bamboo_title,
        jira_project_keys: journal_settings.jira_project_keys,
        mail_send_mode: journal_settings.mail_send_mode,
        mail_body_format: journal_settings.mail_body_format,
        mail_native_html: journal_settings.mail_native_html,
        mail_outlook_flavor: journal_settings.mail_outlook_flavor,
        mail_body_delivery: journal_settings.mail_body_delivery,
        colorful_labels: journal_settings.colorful_labels,
    })
}

/// Normalize an incoming list of Jira project keys: uppercase, strip
/// whitespace and stray punctuation, drop empties + duplicates. Accepts
/// the comma-separated form the wizard sends OR a pre-split Vec from the
/// settings panel. No format gating — the user can save "FOO bar 1!"
/// and we'll store "FOOBAR1"; chastising them for shape choices isn't
/// the wizard's job.
fn normalize_jira_keys(raw: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    raw.into_iter()
        .flat_map(|s| {
            s.split(|c: char| c == ',' || c.is_whitespace())
                .map(|t| {
                    t.chars()
                        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
                        .collect::<String>()
                        .to_ascii_uppercase()
                })
                .collect::<Vec<_>>()
        })
        .filter(|s| !s.is_empty() && seen.insert(s.clone()))
        .collect()
}

/// Payload sent by the first-run wizard on completion.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompleteFirstRunInput {
    pub user_name: Option<String>,
    /// The user's own email (optional — pre-fills userEmail in settings).
    #[serde(default)]
    pub user_email: Option<String>,
    pub journal_root: PathBuf,
    pub reminder: ReminderSettings,
    /// Manager email (optional — the user may skip the manager step).
    #[serde(default)]
    pub manager_email: Option<String>,
    /// Manager display name (optional).
    #[serde(default)]
    pub manager_name: Option<String>,
    /// BambooHR job title (optional).
    #[serde(default)]
    pub bamboo_title: Option<String>,
    /// Jira project keys as the user typed them (comma + whitespace
    /// separated). Backend normalizes to uppercase tokens. Optional.
    #[serde(default)]
    pub jira_project_keys: Vec<String>,
}

/// Payload sent by the post-first-run settings panel.
///
/// All fields are present (not optional) because the settings panel always
/// renders a full form — partial updates aren't a thing yet.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSettingsInput {
    pub user_name: Option<String>,
    /// The user's own email — pins Gmail account / sets Mac Mail sender.
    /// Optional; trimmed and coerced to None when empty, matching userName.
    #[serde(default)]
    pub user_email: Option<String>,
    pub journal_root: PathBuf,
    pub reminder: ReminderSettings,
    pub theme: Theme,
    /// User's Custom theme primaries (Phase 2.8). When `theme == Custom`,
    /// this MUST be `Some(...)` — `update_settings` rejects the call
    /// otherwise. When `theme != Custom`, callers can still send a payload
    /// (the settings panel always submits the current editor state); the
    /// payload is preserved so toggling back to Custom restores it.
    #[serde(default)]
    pub custom_theme: Option<CustomTheme>,
    /// Manager email — `None` (or empty after trim) disables the Send button.
    /// `#[serde(default)]` lets older frontends omit the field without erroring.
    #[serde(default)]
    pub manager_email: Option<String>,
    /// Manager display name — purely cosmetic (greeting in the email).
    #[serde(default)]
    pub manager_name: Option<String>,
    /// BambooHR job title (optional).
    #[serde(default)]
    pub bamboo_title: Option<String>,
    /// Jira project keys (uppercased + deduped server-side). Optional.
    #[serde(default)]
    pub jira_project_keys: Vec<String>,
    /// Mail tab — chosen send path. Defaults to Gmail when omitted.
    #[serde(default)]
    pub mail_send_mode: MailSendMode,
    /// Mail tab — plaintext flavor for Gmail/Outlook + non-HTML Mac Mail.
    #[serde(default)]
    pub mail_body_format: MailBodyFormat,
    /// Mail tab — Native Mac Mail HTML toggle.
    #[serde(default)]
    pub mail_native_html: bool,
    /// Mail tab — Outlook host flavor.
    #[serde(default)]
    pub mail_outlook_flavor: OutlookFlavor,
    /// Mail tab — body delivery (Prefilled vs ClipboardPaste).
    #[serde(default)]
    pub mail_body_delivery: MailBodyDelivery,
    /// Theme tab — Colorful Labels toggle. Defaults to false so older
    /// frontends that omit the field don't accidentally turn it on.
    #[serde(default)]
    pub colorful_labels: bool,
}

/// Writes both settings files. If the user picked a journal root different
/// from the running storage's root, returns `true` so the frontend can prompt
/// for an app restart. (`app.restart()` is unreliable across Tauri 2 minor
/// versions, so we surface the need to the UI instead of triggering it.)
#[tauri::command]
pub async fn complete_first_run(
    app: AppHandle,
    storage_state: State<'_, SharedStorage>,
    reminder_handle: State<'_, ReminderHandle>,
    input: CompleteFirstRunInput,
) -> Result<(), String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;

    // 1. Save app-level settings (journal_root + theme — theme defaults to Dark on first run).
    //    First-run wizard never collects a Custom theme; the user opts into
    //    Custom later through the Settings > Theme tab.
    let app_settings = AppSettings {
        version: CURRENT_VERSION,
        journal_root: input.journal_root.clone(),
        theme: Theme::default(),
        custom_theme: None,
    };
    app_settings
        .save(&app_data_dir)
        .await
        .map_err(|e| e.to_string())?;

    // 2. Save journal-level settings into the CHOSEN root (which may differ
    //    from the storage instance's root if the user picked a non-default).
    //    Phase 2.7 wizard collects manager email/name, Bamboo title, and
    //    Jira keys alongside the original name + reminder. Empty-after-trim
    //    strings persist as None so downstream "is this set?" checks stay
    //    simple.
    let chosen_storage = LocalFilesystem::new(input.journal_root.clone());
    let user_email = normalize_optional_string(input.user_email.as_ref());
    let manager_email = normalize_optional_string(input.manager_email.as_ref());
    let manager_name = normalize_optional_string(input.manager_name.as_ref());
    let bamboo_title = normalize_optional_string(input.bamboo_title.as_ref());
    let jira_project_keys = normalize_jira_keys(input.jira_project_keys.clone());
    let journal_settings = JournalSettings {
        version: CURRENT_VERSION,
        user_name: input.user_name.clone(),
        user_email,
        reminder: input.reminder.clone(),
        manager_email,
        manager_name,
        bamboo_title,
        jira_project_keys,
        // First-run wizard doesn't ask about mail prefs yet — every new
        // install starts on Gmail + clean text, which is the recommended
        // path. The Settings > Mail tab lets the user change it later.
        ..JournalSettings::default()
    };
    journal_settings
        .save(&chosen_storage)
        .await
        .map_err(|e| e.to_string())?;

    // 3. Hot-swap the running LocalFilesystem if the user picked a non-default
    //    root. After this, subsequent commands write to the chosen location
    //    without an app restart.
    {
        let mut fs = storage_state.write().await;
        if fs.root() != input.journal_root.as_path() {
            *fs = LocalFilesystem::new(input.journal_root.clone());
        }
    }

    // 4. Request notification permission if the user just enabled reminders.
    //    This is the highest-acceptance moment for the prompt — the user
    //    explicitly opted in. Idempotent: subsequent calls return the
    //    remembered decision. No-op on non-macOS platforms.
    if input.reminder.enabled {
        request_notification_authorization().await;
    }

    // 5. Restart the reminder scheduler in-process with the new config.
    //    The wizard's reminder takes effect immediately — no relaunch needed.
    restart_reminder_task(
        app.clone(),
        &reminder_handle,
        input.reminder,
        input.user_name,
    );

    // 6. Broadcast so any open window (main, capture) can re-fetch and apply
    //    the new settings immediately — theme, reminder position, etc.
    let _ = app.emit("settings-changed", ());

    // No restart needed — storage and reminder both hot-swap.
    Ok(())
}

/// Save edits from the post-first-run settings panel.
///
/// Like `complete_first_run`, but also handles `theme` (which the wizard
/// doesn't expose) and is meant for use after the user has already onboarded.
/// Everything applies in-process — no app restart needed, even when
/// journal_root changes (the running `LocalFilesystem` is swapped).
#[tauri::command]
pub async fn update_settings(
    app: AppHandle,
    storage_state: State<'_, SharedStorage>,
    reminder_handle: State<'_, ReminderHandle>,
    input: UpdateSettingsInput,
) -> Result<(), String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;

    // 1. App-level (journal_root + theme + custom theme payload).
    //
    // Custom theme rules (locked decision #1, see slice plan):
    //   - theme == Custom requires a 12-token payload — without it the
    //     frontend has nothing to derive from and the user would land on
    //     a blank stylesheet. Reject the call.
    //   - theme == Light/Dark with a payload preserves it. The user can
    //     toggle to Light briefly, come back to Custom, and find their
    //     palette intact.
    //   - theme == Light/Dark without a payload falls back to whatever
    //     was on disk so the same "switch away and back" flow works for
    //     users who haven't touched the Custom editor this session.
    let previous_custom_theme = AppSettings::load(&app_data_dir)
        .await
        .map_err(|e| e.to_string())?
        .and_then(|s| s.custom_theme);
    let custom_theme = match (input.theme, input.custom_theme.clone()) {
        (Theme::Custom, None) => {
            return Err(
                "custom theme requires a payload — none was provided".to_string(),
            );
        }
        (_, Some(payload)) => Some(payload),
        (_, None) => previous_custom_theme,
    };
    let app_settings = AppSettings {
        version: CURRENT_VERSION,
        journal_root: input.journal_root.clone(),
        theme: input.theme,
        custom_theme,
    };
    app_settings
        .save(&app_data_dir)
        .await
        .map_err(|e| e.to_string())?;

    // 2. Journal-level (write to the chosen root so a root change still lands
    //    the new prefs at the right place). Manager email is trimmed; an
    //    empty string after trimming persists as None so the Send button's
    //    "is this set?" check stays simple.
    let chosen_storage = LocalFilesystem::new(input.journal_root.clone());
    let user_email = normalize_optional_string(input.user_email.as_ref());
    let manager_email = normalize_optional_string(input.manager_email.as_ref());
    let manager_name = normalize_optional_string(input.manager_name.as_ref());
    let bamboo_title = normalize_optional_string(input.bamboo_title.as_ref());
    let jira_project_keys = normalize_jira_keys(input.jira_project_keys.clone());
    let journal_settings = JournalSettings {
        version: CURRENT_VERSION,
        user_name: input.user_name.clone(),
        user_email,
        reminder: input.reminder.clone(),
        manager_email,
        manager_name,
        bamboo_title,
        jira_project_keys,
        mail_send_mode: input.mail_send_mode,
        mail_body_format: input.mail_body_format,
        mail_native_html: input.mail_native_html,
        mail_outlook_flavor: input.mail_outlook_flavor,
        mail_body_delivery: input.mail_body_delivery,
        colorful_labels: input.colorful_labels,
    };
    journal_settings
        .save(&chosen_storage)
        .await
        .map_err(|e| e.to_string())?;

    // 3. Hot-swap the running LocalFilesystem if root changed.
    {
        let mut fs = storage_state.write().await;
        if fs.root() != input.journal_root.as_path() {
            *fs = LocalFilesystem::new(input.journal_root.clone());
        }
    }

    // 4. Request notification permission if the user has reminders enabled.
    //    macOS only — the system prompt fires once, subsequent calls return
    //    the remembered decision.
    if input.reminder.enabled {
        request_notification_authorization().await;
    }

    // 5. Restart the reminder scheduler with the new config (no-op if disabled).
    restart_reminder_task(
        app.clone(),
        &reminder_handle,
        input.reminder,
        input.user_name,
    );

    // 6. Broadcast so all windows refresh (theme on capture popup, Noot
    //    appears/disappears on the week stripe, etc.) without waiting for
    //    the next 60-second tick.
    let _ = app.emit("settings-changed", ());

    Ok(())
}

// ---------------------------------------------------------------------------
// Dirty registry
// ---------------------------------------------------------------------------

/// Publish the dirty state of a frontend surface into the backend's
/// DirtyRegistry. Called by `app/src/lib/dirty.ts` from /summary and the
/// capture popup whenever their form state diverges from the last-saved
/// snapshot. Read at quit time by `try_quit` (in lib.rs).
///
/// `key` is a stable namespace string ("summary", "capture"). Adding more
/// dirty surfaces later doesn't require Rust changes — just call with a
/// new key from the frontend.
#[tauri::command]
pub fn set_window_dirty(
    registry: State<'_, DirtyRegistry>,
    key: String,
    entry: DirtyEntry,
) {
    // Recover the guard from a poisoned mutex rather than panicking the
    // Tauri thread. Poisoning means an earlier holder panicked mid-update;
    // the dirty-registry data may be inconsistent but continuing beats
    // crashing the whole app (which would lose any actual unsaved work
    // that IS still recoverable via the auto-save sidecar).
    let mut guard = registry
        .0
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.insert(key, entry);
}

// ---------------------------------------------------------------------------
// Capture draft (auto-save Phase 2)
// ---------------------------------------------------------------------------
//
// The quick-capture popup auto-saves its in-flight contents to
// `<journal>/.metadata/capture-draft.json` on a 1.5s debounce. This lets the
// user close the popup, quit the app, or crash without losing what they were
// typing — the draft reloads on next launch. The draft is cleared on a
// successful Submit (when it becomes a real Note in the weekly file).

const CAPTURE_DRAFT_FILE: &str = "capture-draft.json";

/// Load the saved draft, if any. Returns `None` when the file is missing,
/// when it parses but is empty (all fields blank — semantically nothing to
/// restore), or when the file is corrupt (treated as "no draft" rather than
/// surfacing a parse error — a corrupt file simply means the user starts
/// with an empty popup, which is the same as no draft).
#[tauri::command]
pub async fn load_capture_draft(
    storage_state: State<'_, SharedStorage>,
) -> Result<Option<CaptureDraft>, String> {
    let storage = storage_state.read().await;
    let raw = match storage.read_metadata(CAPTURE_DRAFT_FILE).await {
        Ok(Some(c)) => c,
        Ok(None) => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    let draft: CaptureDraft = match serde_json::from_str(&raw) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("[capture-draft] failed to parse {CAPTURE_DRAFT_FILE}: {e}");
            return Ok(None);
        }
    };
    if draft.is_empty() {
        Ok(None)
    } else {
        Ok(Some(draft))
    }
}

/// Persist the current draft. If the draft is empty (after normalization)
/// we delete the file instead of writing empty bytes — keeps the
/// `.metadata/` folder clean for the no-draft case.
#[tauri::command]
pub async fn save_capture_draft(
    storage_state: State<'_, SharedStorage>,
    draft: CaptureDraft,
) -> Result<(), String> {
    let storage = storage_state.read().await;
    if draft.is_empty() {
        return storage
            .delete_metadata(CAPTURE_DRAFT_FILE)
            .await
            .map_err(|e| e.to_string());
    }
    let serialized = serde_json::to_string_pretty(&draft).map_err(|e| e.to_string())?;
    storage
        .write_metadata(CAPTURE_DRAFT_FILE, &serialized)
        .await
        .map_err(|e| e.to_string())
}

/// Delete the draft file. Called after a successful Submit (the draft
/// became a real Note). Idempotent — "file already absent" is fine.
#[tauri::command]
pub async fn clear_capture_draft(
    storage_state: State<'_, SharedStorage>,
) -> Result<(), String> {
    let storage = storage_state.read().await;
    storage
        .delete_metadata(CAPTURE_DRAFT_FILE)
        .await
        .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Send weekly summary to manager (Phase 2.6)
// ---------------------------------------------------------------------------
//
// The frontend's "Send to manager" button drives three commands in sequence:
//
//   1. get_sent_record(year, week) on page load — feeds the disabled/enabled
//      decision (already sent for this week + same content hash → disabled).
//   2. compose_weekly_email(year, week) on click + confirm — returns either
//      a mailto: URL or an .eml file path; frontend hands it to opener.
//   3. mark_weekly_summary_sent(year, week, contentHash, sentTo) after the
//      open returns Ok — stamps sent-log.json so the next load knows.
//
// No live link to lib::run is needed; everything reads/writes through the
// same storage backend the rest of the app already uses.

/// Return the sent-log entry for (year, week), or `None` if this week has
/// never been sent. Cheap — re-reads `sent-log.json` each call (the file is
/// tiny; in-memory caching would just add invalidation bugs).
#[tauri::command]
pub async fn get_sent_record(
    storage_state: State<'_, SharedStorage>,
    year: u32,
    week: u32,
) -> Result<Option<SentRecord>, String> {
    let storage = storage_state.read().await;
    load_sent_record(&*storage, year, week)
        .await
        .map_err(|e| e.to_string())
}

/// Compose the email for (year, week) into either a `mailto:` URL or an
/// `.eml` file (length-based fallback). Reads the current Weekly Summary
/// off disk every time so we never compose stale text — frontend gates the
/// button on `isDirty` to prevent the user from sending unsaved edits.
///
/// Errors:
///   - `"no manager email set"` if the journal settings have no manager
///     email (or it's empty after trim). UI gates on this too; backend
///     check is defense-in-depth.
///   - I/O / serde errors as strings.
#[tauri::command]
pub async fn compose_weekly_email(
    storage_state: State<'_, SharedStorage>,
    year: u32,
    week: u32,
    format: Option<String>,
    mail_send_mode: Option<MailSendMode>,
    user_email: Option<String>,
) -> Result<ComposeResult, String> {
    let storage = storage_state.read().await;
    // /summary and /journal both default to HTML (the styled multipart
    // .eml). Unknown/omitted values fall through to HTML so the seam is
    // forward-compatible — explicit "text" callers (none today) get the
    // legacy mailto/single-part path.
    let body_format = match format.as_deref() {
        Some("text") => crate::email::BodyFormat::Text,
        _ => crate::email::BodyFormat::Html,
    };

    let journal_settings = JournalSettings::load(&*storage)
        .await
        .map_err(|e| e.to_string())?;
    // Manager email is optional — when missing we still compose a draft
    // and let the user fill the To: line in their mail app. Empty string
    // signals "no known recipient" to the email layer.
    let recipient = journal_settings
        .manager_email
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_default();
    let manager_name = journal_settings
        .manager_name
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let raw = storage
        .read_week(year, week)
        .await
        .map_err(|e| e.to_string())?;
    let summary = match raw {
        Some(c) => parse_weekly_summary(&c),
        None => return Err(format!("no weekly summary saved for {year}-W{week:02}")),
    };

    // Check the sent-log: if a record for this week already exists, this is
    // a resend (gating ensures the content hash differs from the recorded one,
    // so we wouldn't even be here unless the user edited and saved). Resends
    // use a different subject line so the manager's mail thread shows it's
    // an updated version of an earlier message.
    let is_resend = load_sent_record(&*storage, year, week)
        .await
        .map_err(|e| e.to_string())?
        .is_some();

    let week_label = format_week_label(year, week);
    let now = Local::now().fixed_offset();

    // Phase 2.9b Slice 4: route through the mode-specific builder. Frontend
    // passes the active mail_send_mode + user_email (read from settings
    // before the invoke) so the dispatch is explicit at the call site;
    // the remaining body_format / native_html / outlook_flavor knobs read
    // from journal_settings since they don't change per-call. Falling back
    // to settings when the frontend omits the args keeps older callers
    // (and tests) working unchanged.
    let effective_mode = mail_send_mode.unwrap_or(journal_settings.mail_send_mode);
    let effective_user_email = user_email
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| journal_settings.user_email.clone());
    let send = MailSend {
        mode: effective_mode,
        body_format: journal_settings.mail_body_format,
        user_email: effective_user_email.as_deref(),
        outlook_flavor: journal_settings.mail_outlook_flavor,
        native_html: journal_settings.mail_native_html,
        body_in_clipboard: journal_settings.mail_body_delivery == MailBodyDelivery::ClipboardPaste,
    };

    let params = crate::email::ComposeParams {
        summary: &summary,
        week_label: &week_label,
        recipient: &recipient,
        manager_name: manager_name.as_deref(),
        is_resend,
        now,
        format: body_format,
        send: Some(send),
    };
    compose(params).map_err(|e| e.to_string())
}

/// Stamp the sent-log entry for (year, week) after the user successfully
/// hands off the email to their mail client. The frontend supplies the
/// content hash it observed at compose time so we never compute it
/// differently between the two calls.
#[tauri::command]
pub async fn mark_weekly_summary_sent(
    storage_state: State<'_, SharedStorage>,
    year: u32,
    week: u32,
    content_hash: String,
    sent_to: String,
) -> Result<SentRecord, String> {
    let storage = storage_state.read().await;
    let now = Local::now().fixed_offset();
    let record = SentRecord {
        sent_at: now.to_rfc3339(),
        content_hash,
        sent_to,
    };
    upsert_sent_record(&*storage, year, week, record.clone())
        .await
        .map_err(|e| e.to_string())?;
    Ok(record)
}

/// Spawn `osascript -` and pipe `script` into its stdin. Returns `Ok(())`
/// on a zero exit code; on non-zero exit, returns the trimmed stderr
/// prefixed with `permission_denied:` when the message indicates Apple
/// Events denial (`-1743` / "Not authorised" / "Not authorized") so the
/// frontend can offer an "Open Automation Settings" link.
///
/// Stdin is the only viable input path: AppleScript drafts can run to tens
/// of KB once a full weekly summary is in the `content` literal, well past
/// the argv length cap on long invocations.
#[tauri::command]
pub async fn run_applescript(script: String) -> Result<(), String> {
    use tokio::io::AsyncWriteExt;
    use tokio::process::Command;

    let mut child = Command::new("osascript")
        .arg("-")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn osascript: {e}"))?;

    // Drop the stdin handle after writing so osascript sees EOF and
    // starts executing. Without the explicit drop the child would sit
    // waiting on stdin until our handle's lifetime ended.
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(script.as_bytes())
            .await
            .map_err(|e| format!("failed to write applescript: {e}"))?;
        // Explicit shutdown ensures the FD closes before we await the
        // child — tokio's AsyncWrite doesn't drop-flush on stdin pipes
        // until shutdown completes.
        let _ = stdin.shutdown().await;
    }

    let output = child
        .wait_with_output()
        .await
        .map_err(|e| format!("failed to await osascript: {e}"))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    // Apple Events permission denial — surface a discriminable prefix so
    // the frontend can offer the System Settings link without parsing
    // free-form text.
    if stderr.contains("-1743")
        || stderr.contains("Not authorised")
        || stderr.contains("Not authorized")
    {
        return Err(format!("permission_denied:{stderr}"));
    }
    Err(stderr)
}

/// Compute the SHA-256 hash of the current saved Weekly Summary for
/// (year, week). Used by the frontend to drive the Send-button gating
/// (compare against the hash stored in the sent-log entry to detect
/// "edited since last send"). Returns an empty string if no summary
/// exists yet.
#[tauri::command]
pub async fn get_summary_hash(
    storage_state: State<'_, SharedStorage>,
    year: u32,
    week: u32,
) -> Result<String, String> {
    let storage = storage_state.read().await;
    let raw = storage
        .read_week(year, week)
        .await
        .map_err(|e| e.to_string())?;
    Ok(match raw {
        Some(c) => hash_weekly_summary(&parse_weekly_summary(&c)),
        None => String::new(),
    })
}

/// Payload returned by [`render_weekly_summary_preview`] — both rendered
/// bodies plus the metadata the preview modal needs to show the user
/// exactly what's about to be drafted (subject, recipient, week label,
/// resend status).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewPayload {
    pub subject: String,
    pub recipient: String,
    pub week_label: String,
    pub html: String,
    pub text: String,
    pub is_resend: bool,
}

/// Render BOTH the plaintext and HTML bodies for (year, week) without
/// writing any .eml file or building a mailto URL. Used by the
/// SendToManagerButton preview modal to show the manager-facing email
/// before the user opens the draft.
///
/// Reads the same inputs as `compose_weekly_email` so previews match
/// what the actual send will produce: weekly markdown off disk, manager
/// email + name from settings, resend flag from sent-log. Empty
/// recipient is fine — the preview modal surfaces a "no recipient" hint.
#[tauri::command]
pub async fn render_weekly_summary_preview(
    storage_state: State<'_, SharedStorage>,
    _app_handle: AppHandle,
    year: u32,
    week: u32,
) -> Result<PreviewPayload, String> {
    let storage = storage_state.read().await;

    let journal_settings = JournalSettings::load(&*storage)
        .await
        .map_err(|e| e.to_string())?;
    let recipient = journal_settings
        .manager_email
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_default();
    let manager_name = journal_settings
        .manager_name
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let raw = storage
        .read_week(year, week)
        .await
        .map_err(|e| e.to_string())?;
    let summary = match raw {
        Some(c) => parse_weekly_summary(&c),
        None => return Err(format!("no weekly summary saved for {year}-W{week:02}")),
    };

    let is_resend = load_sent_record(&*storage, year, week)
        .await
        .map_err(|e| e.to_string())?
        .is_some();

    let week_label = format_week_label(year, week);
    let subject = if is_resend {
        format!("Update to weekly update - {week_label}")
    } else {
        format!("Weekly update - {week_label}")
    };

    let manager_name_ref = manager_name.as_deref();
    let text = crate::email::render_body(&summary, &week_label, manager_name_ref);
    let html = crate::email_html::render_body_html(&summary, &week_label, manager_name_ref);

    Ok(PreviewPayload {
        subject,
        recipient,
        week_label,
        html,
        text,
        is_resend,
    })
}

/// Format `(year, week)` as the human-readable label used in email subjects,
/// the email body intro line, and the confirmation modal: e.g.
/// `"week of June 22 – June 28, 2026"`. Lowercase leading "week" so the
/// string drops cleanly into sentences ("Weekly update - week of ...",
/// "for the week of ..."); the /summary heading uses its own capitalized
/// variant ("Week of June 22 – June 28, 2026") computed by the frontend.
///
/// ## Stay in sync with the frontend
///
/// The frontend computes its own `weekLabel` in `/summary` for display in
/// the page heading and confirmation modal (via `inlineLabel()` to drop
/// the leading capital). The two strings MUST match character-for-character
/// when lowercased on the leading `W` — otherwise the user sees one rendering
/// in the modal ("for the week of June 22 – June 28, 2026") and a different
/// rendering in the actual email subject they hand off to their mail client.
/// Format conventions kept in lockstep:
///
///   - Full month name (`%B` → "June", not abbreviated "Jun")
///   - En-dash (U+2013, " – ") between start and end dates, not ASCII "-"
///   - No zero-padding on day numbers (`%-d` → "22", not "22")
///   - Cross-year weeks repeat the year on both sides
///
/// If either side changes, the matching test in /summary's weekLabel logic
/// AND `week_label_matches_frontend_format` here must be updated together.
fn format_week_label(year: u32, week: u32) -> String {
    use chrono::{Datelike, Duration, NaiveDate};
    // ISO week 1 is the week containing Jan 4. Walk back to that week's
    // Monday, then offset by (week-1) weeks.
    let Some(jan4) = NaiveDate::from_ymd_opt(year as i32, 1, 4) else {
        return format!("{year}-W{week:02}");
    };
    let dow_from_monday = jan4.weekday().num_days_from_monday();
    let monday_of_week1 = jan4 - Duration::days(dow_from_monday as i64);
    let monday = monday_of_week1 + Duration::weeks((week as i64).saturating_sub(1));
    let sunday = monday + Duration::days(6);

    // "June 22" — %B is the full month name, %-d strips zero-padding.
    let fmt = |d: NaiveDate| d.format("%B %-d").to_string();

    if monday.year() == sunday.year() {
        format!(
            "week of {} \u{2013} {}, {}",
            fmt(monday),
            fmt(sunday),
            monday.year()
        )
    } else {
        format!(
            "week of {}, {} \u{2013} {}, {}",
            fmt(monday),
            monday.year(),
            fmt(sunday),
            sunday.year()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn week_label_matches_frontend_format() {
        // The frontend computes weekLabel for the heading + modal with full
        // month names + en-dash. The backend must produce a string that's
        // identical to inlineLabel(frontend) so the modal and the email
        // subject read the same week.
        let s = format_week_label(2026, 26);
        assert!(s.starts_with("week of June "), "got {s:?}");
        assert!(s.ends_with(", 2026"), "got {s:?}");
        assert!(s.contains(" \u{2013} "), "expected en-dash separator, got {s:?}");
    }

    #[test]
    fn week_label_w01_is_january() {
        let s = format_week_label(2026, 1);
        // W01 always contains Jan 4; the Monday could be late Dec of the
        // previous year, hence the cross-year branch. Either way the label
        // mentions "January" (full month name now).
        assert!(s.contains("January"), "expected January in {s}");
    }

    // ----- run_applescript -----
    //
    // osascript ships with every macOS install, so the no-op script test is
    // a real end-to-end exercise of the spawn/stdin/wait pipeline. Skipped
    // on non-macOS where /usr/bin/osascript doesn't exist. The
    // permission-denied branch isn't unit-tested because it depends on
    // Apple Events authorization state, which is environment-specific.

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn run_applescript_executes_no_op_script() {
        let result = run_applescript("log \"hello\"".to_string()).await;
        assert!(result.is_ok(), "expected Ok, got {result:?}");
    }

    // ----- set_label_color_impl -----

    #[tokio::test]
    async fn set_label_color_sets_then_clears_override() {
        use crate::labels::record_note_labels;
        use crate::notes::Note;
        use crate::storage::LocalFilesystem;
        use chrono::{DateTime, FixedOffset, NaiveDate};
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());

        // Seed the index with a real label entry the way create_note would.
        let note = Note {
            timestamp: DateTime::<FixedOffset>::parse_from_rfc3339("2026-06-22T10:00:00-04:00")
                .unwrap(),
            title: None,
            labels: vec!["release".to_string()],
            body: String::new(),
        };
        record_note_labels(&backend, &note, NaiveDate::from_ymd_opt(2026, 6, 22).unwrap())
            .await
            .unwrap();

        // Set a color.
        set_label_color_impl(&backend, "release", Some("#FF5C08".to_string()))
            .await
            .unwrap();
        let idx = LabelIndex::load(&backend).await.unwrap();
        let entry = idx.labels.iter().find(|e| e.name == "release").unwrap();
        assert_eq!(entry.color.as_deref(), Some("#ff5c08"));

        // Clear the color.
        set_label_color_impl(&backend, "release", None).await.unwrap();
        let idx = LabelIndex::load(&backend).await.unwrap();
        let entry = idx.labels.iter().find(|e| e.name == "release").unwrap();
        assert!(entry.color.is_none());
    }

    #[tokio::test]
    async fn set_label_color_rejects_malformed_hex() {
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());

        // Doesn't matter that the label doesn't exist yet — validation
        // happens before the lookup. The "not a color" error must surface
        // and name the offending value.
        let err = set_label_color_impl(&backend, "release", Some("not-a-color".to_string()))
            .await
            .unwrap_err();
        assert!(
            err.contains("hex color") && err.contains("not-a-color"),
            "expected error to name the bad value, got: {err}"
        );
    }

    #[tokio::test]
    async fn set_label_color_unknown_label_errors() {
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());

        // No labels seeded at all — looking up "release" should fail with
        // a message that names the missing label so the frontend can show
        // a useful error.
        let err = set_label_color_impl(&backend, "release", Some("#ff5c08".to_string()))
            .await
            .unwrap_err();
        assert!(
            err.contains("release") && err.contains("not found"),
            "expected error to name the missing label, got: {err}"
        );
    }

    // ----- Concurrency: write-lock serialization for label-index mutations -----
    //
    // The Tauri command wrappers (`set_label_color`, `create_note`) take the
    // SharedStorage write lock before driving `set_label_color_impl` /
    // `record_note_labels`. The tests below stand up the same wrapper shape
    // (Arc<RwLock<LocalFilesystem>>) and fan out concurrent mutations to
    // confirm no updates are lost.
    //
    // Without the write lock + atomic rename in write_metadata, these would
    // flake: two tasks reading the same labels.json snapshot, each mutating
    // their own copy, and the second save would clobber the first.

    #[tokio::test]
    async fn concurrent_set_label_color_all_persist() {
        use crate::labels::record_note_labels;
        use crate::notes::Note;
        use crate::storage::LocalFilesystem;
        use chrono::{DateTime, FixedOffset, NaiveDate};
        use std::sync::Arc;
        use tempfile::TempDir;
        use tokio::sync::RwLock;

        let dir = TempDir::new().unwrap();
        let storage = Arc::new(RwLock::new(LocalFilesystem::new(dir.path())));

        // Seed 10 labels.
        let names: Vec<String> = (0..10).map(|i| format!("label-{i}")).collect();
        let palette: Vec<String> = (0..10)
            .map(|i| format!("#{:06x}", 0x100000 + i * 0x111111))
            .collect();
        {
            let s = storage.write().await;
            for name in &names {
                let note = Note {
                    timestamp: DateTime::<FixedOffset>::parse_from_rfc3339(
                        "2026-06-22T10:00:00-04:00",
                    )
                    .unwrap(),
                    title: None,
                    labels: vec![name.clone()],
                    body: String::new(),
                };
                record_note_labels(&*s, &note, NaiveDate::from_ymd_opt(2026, 6, 22).unwrap())
                    .await
                    .unwrap();
            }
        }

        // Fan out 10 concurrent set_label_color calls. Each acquires the
        // write lock the same way the Tauri command would.
        let mut handles = Vec::new();
        for (name, color) in names.iter().zip(palette.iter()) {
            let storage = Arc::clone(&storage);
            let name = name.clone();
            let color = color.clone();
            handles.push(tokio::spawn(async move {
                let s = storage.write().await;
                set_label_color_impl(&*s, &name, Some(color)).await
            }));
        }
        for h in handles {
            h.await.unwrap().unwrap();
        }

        // Every color must be present on the right entry. If the read lock
        // was used instead, this would flake under load — at least one
        // color would be lost.
        let s = storage.read().await;
        let idx = LabelIndex::load(&*s).await.unwrap();
        for (name, expected) in names.iter().zip(palette.iter()) {
            let entry = idx
                .labels
                .iter()
                .find(|e| &e.name == name)
                .unwrap_or_else(|| panic!("missing entry for {name}"));
            assert_eq!(
                entry.color.as_deref(),
                Some(expected.as_str()),
                "color for {name} was clobbered"
            );
        }
    }

    #[tokio::test]
    async fn concurrent_set_color_and_record_note_both_survive() {
        use crate::labels::record_note_labels;
        use crate::notes::Note;
        use crate::storage::LocalFilesystem;
        use chrono::{DateTime, FixedOffset, NaiveDate};
        use std::sync::Arc;
        use tempfile::TempDir;
        use tokio::sync::RwLock;

        let dir = TempDir::new().unwrap();
        let storage = Arc::new(RwLock::new(LocalFilesystem::new(dir.path())));

        // Seed.
        {
            let s = storage.write().await;
            let note = Note {
                timestamp: DateTime::<FixedOffset>::parse_from_rfc3339(
                    "2026-06-22T10:00:00-04:00",
                )
                .unwrap(),
                title: None,
                labels: vec!["release".to_string()],
                body: String::new(),
            };
            record_note_labels(&*s, &note, NaiveDate::from_ymd_opt(2026, 6, 22).unwrap())
                .await
                .unwrap();
        }

        // One task sets a color override; the other records 5 more
        // occurrences of the same label. The write lock guarantees that
        // whichever ordering wins, both the count bump and the color
        // override end up reflected in the final on-disk index.
        let color_task = {
            let storage = Arc::clone(&storage);
            tokio::spawn(async move {
                let s = storage.write().await;
                set_label_color_impl(&*s, "release", Some("#ff5c08".to_string())).await
            })
        };
        let count_task = {
            let storage = Arc::clone(&storage);
            tokio::spawn(async move {
                let s = storage.write().await;
                for i in 0..5 {
                    let note = Note {
                        timestamp: DateTime::<FixedOffset>::parse_from_rfc3339(&format!(
                            "2026-06-23T10:00:{:02}-04:00",
                            i
                        ))
                        .unwrap(),
                        title: None,
                        labels: vec!["release".to_string()],
                        body: String::new(),
                    };
                    record_note_labels(&*s, &note, NaiveDate::from_ymd_opt(2026, 6, 23).unwrap())
                        .await
                        .unwrap();
                }
                Ok::<(), String>(())
            })
        };

        color_task.await.unwrap().unwrap();
        count_task.await.unwrap().unwrap();

        let s = storage.read().await;
        let idx = LabelIndex::load(&*s).await.unwrap();
        let entry = idx.labels.iter().find(|e| e.name == "release").unwrap();
        // Original (1 from seed) + 5 from the count task = 6.
        assert_eq!(entry.count, 6, "count update was lost to the color write");
        assert_eq!(
            entry.color.as_deref(),
            Some("#ff5c08"),
            "color override was lost to the count write"
        );
    }

    #[tokio::test]
    async fn atomic_write_leaves_original_intact_on_crash_before_rename() {
        // Simulate "crash between write and rename" by writing a .tmp file
        // ourselves and then asserting the original metadata file is
        // untouched. This pins the contract of the atomic-write helper in
        // storage.rs — readers always see either the pre-write content or
        // the new content, never a half-written file.
        use crate::storage::{LocalFilesystem, StorageBackend};
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());

        // Establish a baseline file via the normal path.
        let original = r#"{"version":1,"labels":[]}"#;
        backend.write_metadata("labels.json", original).await.unwrap();

        // Simulate a crash: dump bytes into the staging file but never rename.
        let metadata_dir = dir.path().join(".metadata");
        let tmp_path = metadata_dir.join("labels.json.tmp");
        tokio::fs::write(&tmp_path, "PARTIAL GARBAGE NEVER RENAMED")
            .await
            .unwrap();

        // The real file must still hold the original content.
        let read_back = backend.read_metadata("labels.json").await.unwrap();
        assert_eq!(read_back.as_deref(), Some(original));

        // And a subsequent successful write must overwrite the stale .tmp
        // (no leftover from the simulated crash should poison the next write).
        let updated = r#"{"version":1,"labels":[{"name":"x","count":1,"firstUsed":"2026-06-22","lastUsed":"2026-06-22"}]}"#;
        backend.write_metadata("labels.json", updated).await.unwrap();
        let read_back = backend.read_metadata("labels.json").await.unwrap();
        assert_eq!(read_back.as_deref(), Some(updated));
    }

    // ----- rebuild_label_index -----

    /// Build a weekly file body with a Weekly Summary `### Labels`
    /// subsection containing the given labels, plus an optional Note with
    /// its `**Labels:**` line. Mirrors the scaffold + render_weekly_summary
    /// path so `scan_label_sites` sees the same shape it sees in real files.
    fn build_weekly_file(
        year: u32,
        week: u32,
        summary_labels: &[&str],
        note_labels: &[&str],
    ) -> String {
        use crate::notes::{
            replace_weekly_summary_in_file, weekly_file_scaffold, WeeklySummary,
        };
        let now = chrono::DateTime::<chrono::FixedOffset>::parse_from_rfc3339(
            "2026-06-22T10:00:00-04:00",
        )
        .unwrap();
        let mut file = weekly_file_scaffold(year, week, now);
        if !summary_labels.is_empty() {
            let summary = WeeklySummary {
                key_accomplishments: "scan me".to_string(),
                labels: summary_labels.iter().map(|s| s.to_string()).collect(),
                last_updated: Some("2026-06-22 10:00".to_string()),
                ..Default::default()
            };
            file = replace_weekly_summary_in_file(&file, &summary);
        }
        if !note_labels.is_empty() {
            let note = Note {
                timestamp: now,
                title: Some("scan-test note".to_string()),
                labels: note_labels.iter().map(|s| s.to_string()).collect(),
                body: "body".to_string(),
            };
            file.push_str(&note.to_markdown());
        }
        file
    }

    #[tokio::test]
    async fn rebuild_on_empty_journal_writes_empty_index() {
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());

        let result = rebuild_label_index_impl(&backend).await.unwrap();
        assert_eq!(result.labels_found, 0);
        assert_eq!(result.files_scanned, 0);
        assert!(result.failed_files.is_empty());

        // labels.json should exist and be an empty index — not absent.
        // The rebuild always writes; that's the seam the frontend's
        // "scanned N files" report depends on.
        let idx = LabelIndex::load(&backend).await.unwrap();
        assert!(idx.labels.is_empty());
    }

    #[tokio::test]
    async fn rebuild_preserves_color_overrides_across_regeneration() {
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());

        // Seed a weekly file with one label so the rebuilt entry exists.
        let content = build_weekly_file(2026, 25, &["release"], &[]);
        backend.write_week(2026, 25, &content).await.unwrap();

        // First rebuild creates the entry with no color.
        rebuild_label_index_impl(&backend).await.unwrap();
        // Manually pin a color override the way set_label_color would.
        set_label_color_impl(&backend, "release", Some("#ff5c08".to_string()))
            .await
            .unwrap();

        // Now drop a second file that records another occurrence and
        // rebuild again. The color override must survive — that's the
        // whole point of pre-loading existing labels.json before walking
        // the weekly files.
        let content2 = build_weekly_file(2026, 26, &["release"], &[]);
        backend.write_week(2026, 26, &content2).await.unwrap();

        let result = rebuild_label_index_impl(&backend).await.unwrap();
        assert_eq!(result.labels_found, 1);
        assert_eq!(result.files_scanned, 2);

        let idx = LabelIndex::load(&backend).await.unwrap();
        let entry = idx
            .labels
            .iter()
            .find(|e| e.name == "release")
            .expect("release entry must survive rebuild");
        assert_eq!(entry.count, 2, "count should reflect 2 distinct files");
        assert_eq!(
            entry.color.as_deref(),
            Some("#ff5c08"),
            "color override must survive the rebuild"
        );
    }

    #[tokio::test]
    async fn rebuild_corrupt_weekly_file_logs_but_continues() {
        // Per locked-decision #7, a per-file read error doesn't abort the
        // rebuild — it just gets reported in failed_files. We can't
        // actually corrupt a tokio::fs read easily; the closest we can do
        // is force a file that fails to scan cleanly. Since scan_label_sites
        // is total (any string parses, just yielding no sites), the only
        // way to surface a failure in the impl loop is a read_week error.
        //
        // Strategy: write two valid weekly files, then symlink/replace one
        // with a directory at the expected path so tokio::fs::read_to_string
        // returns an error. That's specifically what `read_week` propagates
        // through the impl's match arm.
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());

        let good = build_weekly_file(2026, 25, &["release"], &[]);
        backend.write_week(2026, 25, &good).await.unwrap();
        let good2 = build_weekly_file(2026, 26, &["mage"], &[]);
        backend.write_week(2026, 26, &good2).await.unwrap();

        // Replace W25's file with a directory so the read fails with a
        // non-NotFound IO error that bubbles up.
        let bad_path = dir.path().join("2026").join("2026-W25.md");
        tokio::fs::remove_file(&bad_path).await.unwrap();
        tokio::fs::create_dir_all(&bad_path).await.unwrap();

        let result = rebuild_label_index_impl(&backend).await.unwrap();
        // W26 still scanned; W25 reported as failed.
        assert!(result.files_scanned >= 1);
        assert_eq!(result.failed_files.len(), 1);
        assert!(
            result.failed_files[0].contains("2026-W25"),
            "failed_files should name the bad file, got {:?}",
            result.failed_files
        );

        // The good file's label still landed in the rebuilt index — the
        // partial failure didn't poison the whole pass.
        let idx = LabelIndex::load(&backend).await.unwrap();
        assert!(idx.labels.iter().any(|e| e.name == "mage"));
    }

    #[tokio::test]
    async fn rebuild_dedupes_label_within_single_file() {
        // The count rule is "1 increment per unique label per file,"
        // matching record_note_labels semantics. A label that appears in
        // both the Summary `### Labels` subsection AND a Note's
        // `**Labels:**` line in the same weekly file must count as ONE
        // for that file, not two.
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());

        let content = build_weekly_file(2026, 25, &["release"], &["release"]);
        backend.write_week(2026, 25, &content).await.unwrap();

        let result = rebuild_label_index_impl(&backend).await.unwrap();
        assert_eq!(result.labels_found, 1);

        let idx = LabelIndex::load(&backend).await.unwrap();
        let entry = idx.labels.iter().find(|e| e.name == "release").unwrap();
        assert_eq!(
            entry.count, 1,
            "summary + note line in the same file = 1 file occurrence, not 2"
        );
    }

    #[tokio::test]
    async fn rebuild_computes_first_and_last_used_from_week_dates() {
        // first_used/last_used must come from the ISO-week start date of
        // the weekly file the label appears in — matching what
        // `Note::weekly_file_path` and the rest of the journal use for
        // week boundaries.
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());

        // Drop the same label into three different weeks across the year.
        // W3 of 2026 — Monday 2026-01-12. W25 — Monday 2026-06-15.
        // W40 — Monday 2026-09-28.
        for (week, expect_start) in &[
            (3u32, NaiveDate::from_ymd_opt(2026, 1, 12).unwrap()),
            (25u32, NaiveDate::from_ymd_opt(2026, 6, 15).unwrap()),
            (40u32, NaiveDate::from_ymd_opt(2026, 9, 28).unwrap()),
        ] {
            // Sanity-check the test's assumption — if chrono ever changes
            // its ISO week math this should yell.
            assert_eq!(iso_week_start(2026, *week), *expect_start);
            let content = build_weekly_file(2026, *week, &["release"], &[]);
            backend.write_week(2026, *week, &content).await.unwrap();
        }

        rebuild_label_index_impl(&backend).await.unwrap();
        let idx = LabelIndex::load(&backend).await.unwrap();
        let entry = idx.labels.iter().find(|e| e.name == "release").unwrap();
        assert_eq!(entry.count, 3);
        assert_eq!(entry.first_used, NaiveDate::from_ymd_opt(2026, 1, 12).unwrap());
        assert_eq!(entry.last_used, NaiveDate::from_ymd_opt(2026, 9, 28).unwrap());
    }

    // ----- get_label_stats -----
    //
    // The Tauri command wrapper takes a `State` and isn't unit-testable
    // directly, so we exercise the same body inline against a
    // `LocalFilesystem`. The branches we care about are:
    //   - label in Notes only, Summaries only, both
    //   - drift between scanned total and labels.json's index_count
    //
    // The drift-log test captures stderr via the same approach the rest of
    // the suite uses: it asserts on observable state (return value) plus
    // the fact that the call returns Ok despite the mismatch — verifying
    // we don't auto-repair, per locked-decision #8.

    async fn compute_label_stats<B: crate::storage::StorageBackend + ?Sized>(
        backend: &B,
        name: &str,
    ) -> Result<LabelStats, String> {
        // Mirror the command body so we can drive it without Tauri State.
        // Kept in lockstep with the real impl; if the command's logic grows,
        // this helper must grow with it.
        let mut in_notes: u32 = 0;
        let mut in_summaries: u32 = 0;

        let years = backend.list_years().await.map_err(|e| e.to_string())?;
        for year in years {
            let weeks = match backend.list_weeks(year).await {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("[label-stats] list_weeks({year}) failed: {e}");
                    continue;
                }
            };
            for week in weeks {
                let content = match backend.read_week(year, week).await {
                    Ok(Some(c)) => c,
                    Ok(None) => continue,
                    Err(e) => {
                        eprintln!("[label-stats] read_week({year},{week}) failed: {e}");
                        continue;
                    }
                };
                for site in scan_label_sites(&content) {
                    if site.names.iter().any(|n| n == name) {
                        match site.kind {
                            LabelSiteKind::NoteLabelsLine => {
                                in_notes = in_notes.saturating_add(1);
                            }
                            LabelSiteKind::SummaryLabelsSubsection => {
                                in_summaries = in_summaries.saturating_add(1);
                            }
                        }
                    }
                }
            }
        }

        let total = in_notes.saturating_add(in_summaries);
        let index = LabelIndex::load(backend).await.map_err(|e| e.to_string())?;
        let index_count = index
            .labels
            .iter()
            .find(|e| e.name == name)
            .map(|e| e.count)
            .unwrap_or(0);

        if total != index_count {
            eprintln!(
                "[label-stats] drift detected for {name:?}: scanned={total} index={index_count}"
            );
        }

        Ok(LabelStats {
            total,
            in_notes,
            in_summaries,
            index_count,
        })
    }

    #[tokio::test]
    async fn stats_for_label_in_notes_only() {
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());

        // Two weekly files, each with a Note's labels line referencing "release"
        // but no Summary subsection label. in_notes == 2, in_summaries == 0.
        let f1 = build_weekly_file(2026, 25, &[], &["release"]);
        backend.write_week(2026, 25, &f1).await.unwrap();
        let f2 = build_weekly_file(2026, 26, &[], &["release"]);
        backend.write_week(2026, 26, &f2).await.unwrap();

        // Seed labels.json so index_count matches scanned total (no drift).
        rebuild_label_index_impl(&backend).await.unwrap();

        let stats = compute_label_stats(&backend, "release").await.unwrap();
        assert_eq!(stats.in_notes, 2);
        assert_eq!(stats.in_summaries, 0);
        assert_eq!(stats.total, 2);
        // index_count after rebuild reflects unique-file occurrences (2 files).
        assert_eq!(stats.index_count, 2);
    }

    #[tokio::test]
    async fn stats_for_label_in_summaries_only() {
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());

        // Two weekly files, each with a Summary subsection label but no Note line.
        let f1 = build_weekly_file(2026, 25, &["release"], &[]);
        backend.write_week(2026, 25, &f1).await.unwrap();
        let f2 = build_weekly_file(2026, 26, &["release"], &[]);
        backend.write_week(2026, 26, &f2).await.unwrap();

        rebuild_label_index_impl(&backend).await.unwrap();

        let stats = compute_label_stats(&backend, "release").await.unwrap();
        assert_eq!(stats.in_notes, 0);
        assert_eq!(stats.in_summaries, 2);
        assert_eq!(stats.total, 2);
        assert_eq!(stats.index_count, 2);
    }

    #[tokio::test]
    async fn stats_for_label_in_both() {
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());

        // One file with BOTH the Summary subsection and a Note line referencing
        // "release". Per-site counts: in_notes = 1, in_summaries = 1.
        // Per locked-decision: get_label_stats reports per-site, not per-file,
        // so total = 2 even though it's a single file.
        let f1 = build_weekly_file(2026, 25, &["release"], &["release"]);
        backend.write_week(2026, 25, &f1).await.unwrap();

        rebuild_label_index_impl(&backend).await.unwrap();

        let stats = compute_label_stats(&backend, "release").await.unwrap();
        assert_eq!(stats.in_notes, 1);
        assert_eq!(stats.in_summaries, 1);
        assert_eq!(stats.total, 2);
        // Index uses per-file dedup, so the single file is one occurrence.
        assert_eq!(stats.index_count, 1);
    }

    #[tokio::test]
    async fn stats_logs_drift_when_index_count_differs() {
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());

        // Two real weekly files referencing "release" in Notes.
        let f1 = build_weekly_file(2026, 25, &[], &["release"]);
        backend.write_week(2026, 25, &f1).await.unwrap();
        let f2 = build_weekly_file(2026, 26, &[], &["release"]);
        backend.write_week(2026, 26, &f2).await.unwrap();

        // Hand-author labels.json with a clearly-wrong count. This simulates
        // drift — e.g. a user hand-edited a weekly file outside the app, or
        // an old labels.json from a previous schema. The stats call must
        // return Ok, surface the scanned total faithfully, and report the
        // stale index_count without auto-repairing.
        let bogus = r#"{"version":1,"labels":[{"name":"release","count":999,"firstUsed":"2020-01-01","lastUsed":"2020-01-01"}]}"#;
        backend.write_metadata("labels.json", bogus).await.unwrap();

        let stats = compute_label_stats(&backend, "release").await.unwrap();
        // Scanned total wins per locked-decision #8.
        assert_eq!(stats.total, 2);
        assert_eq!(stats.in_notes, 2);
        assert_eq!(stats.in_summaries, 0);
        // Stale index value surfaces unchanged — we do NOT auto-repair.
        assert_eq!(stats.index_count, 999);

        // And labels.json itself must still hold the stale value — the
        // stats call is read-only, so the bogus count survives the call.
        let idx = LabelIndex::load(&backend).await.unwrap();
        let entry = idx.labels.iter().find(|e| e.name == "release").unwrap();
        assert_eq!(entry.count, 999, "stats call must not mutate labels.json");
    }

    // ----- get_notes_for_label (Label Library drill-down) -----
    //
    // Same test seam as get_label_stats: the Tauri command wrapper takes a
    // `State` we can't build without a full app, so we mirror the body
    // inline via `compute_label_refs`. Coverage:
    //   - extract_note_heading_before happy paths + rejection cases
    //   - Cross-year / cross-week ordering (newest first)
    //   - Both site kinds surface with correct metadata shape (Notes carry
    //     timestamp + optional title; Summaries carry neither)

    #[test]
    fn extract_note_heading_before_parses_timestamp_and_title() {
        let content = "\n### 2026-06-25 14:23 — My note title\n**Labels:** #foo\n";
        let offset = content.find("**Labels:**").unwrap();
        let (ts, title) = extract_note_heading_before(content, offset).unwrap();
        assert_eq!(ts, "2026-06-25 14:23");
        assert_eq!(title.as_deref(), Some("My note title"));
    }

    #[test]
    fn extract_note_heading_before_no_title() {
        let content = "\n### 2026-06-25 14:23\n**Labels:** #foo\n";
        let offset = content.find("**Labels:**").unwrap();
        let (ts, title) = extract_note_heading_before(content, offset).unwrap();
        assert_eq!(ts, "2026-06-25 14:23");
        assert_eq!(title, None);
    }

    #[test]
    fn extract_note_heading_before_rejects_summary_subsection() {
        // "### Key accomplishments" is a Summary subsection, NOT a Note.
        // If we accepted it, drill-down would wrongly attribute a
        // SummaryLabelsSubsection site to a phantom Note.
        let content = "\n### Key accomplishments\nfoo\n";
        let offset = content.find("foo").unwrap();
        assert!(extract_note_heading_before(content, offset).is_none());
    }

    #[test]
    fn extract_note_heading_before_returns_none_with_no_heading() {
        let content = "just a body with no heading above";
        assert!(extract_note_heading_before(content, 5).is_none());
    }

    async fn compute_label_refs<B: crate::storage::StorageBackend + ?Sized>(
        backend: &B,
        name: &str,
    ) -> Result<Vec<LabelReference>, String> {
        // Mirror the command body — kept in lockstep with get_notes_for_label.
        let mut refs: Vec<LabelReference> = Vec::new();

        let mut years = backend.list_years().await.map_err(|e| e.to_string())?;
        years.sort_unstable();
        years.reverse();

        for year in years {
            let mut weeks = match backend.list_weeks(year).await {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("[label-refs] list_weeks({year}) failed: {e}");
                    continue;
                }
            };
            weeks.sort_unstable();
            weeks.reverse();

            for week in weeks {
                let content = match backend.read_week(year, week).await {
                    Ok(Some(c)) => c,
                    Ok(None) => continue,
                    Err(e) => {
                        eprintln!("[label-refs] read_week({year},{week}) failed: {e}");
                        continue;
                    }
                };
                for site in scan_label_sites(&content) {
                    if !site.names.iter().any(|n| n == name) {
                        continue;
                    }
                    let reference = match site.kind {
                        LabelSiteKind::SummaryLabelsSubsection => LabelReference {
                            year,
                            week,
                            kind: LabelReferenceKind::Summary,
                            note_timestamp: None,
                            note_title: None,
                        },
                        LabelSiteKind::NoteLabelsLine => {
                            let (ts, title) =
                                extract_note_heading_before(&content, site.byte_range.start)
                                    .unwrap_or((String::new(), None));
                            LabelReference {
                                year,
                                week,
                                kind: LabelReferenceKind::Note,
                                note_timestamp: if ts.is_empty() { None } else { Some(ts) },
                                note_title: title,
                            }
                        }
                    };
                    refs.push(reference);
                }
            }
        }

        Ok(refs)
    }

    #[tokio::test]
    async fn label_refs_orders_years_desc_and_weeks_desc() {
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());

        // Three files across two years, each carrying "release" in the Summary.
        // Newest week/year should come first in the result list.
        let f_2025 = build_weekly_file(2025, 10, &["release"], &[]);
        backend.write_week(2025, 10, &f_2025).await.unwrap();
        let f_2026_25 = build_weekly_file(2026, 25, &["release"], &[]);
        backend.write_week(2026, 25, &f_2026_25).await.unwrap();
        let f_2026_26 = build_weekly_file(2026, 26, &["release"], &[]);
        backend.write_week(2026, 26, &f_2026_26).await.unwrap();

        let refs = compute_label_refs(&backend, "release").await.unwrap();
        assert_eq!(refs.len(), 3, "one Summary site per file");
        assert_eq!((refs[0].year, refs[0].week), (2026, 26));
        assert_eq!((refs[1].year, refs[1].week), (2026, 25));
        assert_eq!((refs[2].year, refs[2].week), (2025, 10));
        for r in &refs {
            assert!(matches!(r.kind, LabelReferenceKind::Summary));
            assert!(r.note_timestamp.is_none());
            assert!(r.note_title.is_none());
        }
    }

    #[tokio::test]
    async fn label_refs_returns_both_kinds_with_note_metadata() {
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());

        // Single file with BOTH a Summary subsection reference AND a Note
        // labels-line reference to "release" — proves both site kinds land
        // in the result list and the Note carries its heading metadata.
        let f = build_weekly_file(2026, 25, &["release"], &["release"]);
        backend.write_week(2026, 25, &f).await.unwrap();

        let refs = compute_label_refs(&backend, "release").await.unwrap();
        assert_eq!(refs.len(), 2, "one Summary + one Note site");

        let has_summary = refs
            .iter()
            .any(|r| matches!(r.kind, LabelReferenceKind::Summary));
        let has_note = refs
            .iter()
            .any(|r| matches!(r.kind, LabelReferenceKind::Note));
        assert!(has_summary, "Summary site should surface");
        assert!(has_note, "Note site should surface");

        let note_ref = refs
            .iter()
            .find(|r| matches!(r.kind, LabelReferenceKind::Note))
            .unwrap();
        assert!(note_ref.note_timestamp.is_some(), "Note carries timestamp");
        assert_eq!(
            note_ref.note_title.as_deref(),
            Some("scan-test note"),
            "Note carries title from fixture heading"
        );
    }

    // ----- search_journal -----
    //
    // Full-text search command tests. The Tauri wrapper needs `State`
    // we can't build outside a running app, so we drive `compute_search`
    // — a helper that mirrors the command body against a plain
    // `StorageBackend`. Coverage:
    //   - Short-query gate (< 2 chars → empty result)
    //   - Case-insensitive substring match
    //   - Newest-first ordering (years desc, weeks desc within year)
    //   - Label filter — OR semantics; empty filter is no-op
    //   - Multi-match: total_matches counted, snippets capped
    //   - Regex metacharacters treated as literal
    //   - Cross-field matches (Summary only)
    //   - Unicode / emoji round-trip
    //   - (Slice 2) Note-body search + kind discriminator + scroll_offset

    /// Test-only convenience wrapper — turns `&[&str]` inline label lists
    /// into the `&[String]` the impl expects without cluttering every
    /// call-site with `.to_string()` conversions.
    async fn compute_search<B: crate::storage::StorageBackend + ?Sized>(
        backend: &B,
        query: &str,
        label_filter: &[&str],
    ) -> Result<Vec<SearchResult>, String> {
        let owned: Vec<String> = label_filter.iter().map(|s| s.to_string()).collect();
        search_journal_impl(backend, query, &owned).await
    }

    /// Build a weekly file with a specific Weekly Summary payload for
    /// search tests. `build_weekly_file` is oriented toward label-index
    /// tests and only takes label lists; this helper writes actual text
    /// into the four summary fields so substring search has something
    /// to match against.
    fn build_weekly_file_with_summary(
        year: u32,
        week: u32,
        summary: &WeeklySummary,
    ) -> String {
        use crate::notes::{replace_weekly_summary_in_file, weekly_file_scaffold};
        let now = chrono::DateTime::<chrono::FixedOffset>::parse_from_rfc3339(
            "2026-06-22T10:00:00-04:00",
        )
        .unwrap();
        let file = weekly_file_scaffold(year, week, now);
        replace_weekly_summary_in_file(&file, summary)
    }

    #[tokio::test]
    async fn search_returns_empty_for_short_query() {
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        let file = build_weekly_file_with_summary(
            2026,
            25,
            &WeeklySummary {
                key_accomplishments: "shipped the thing".to_string(),
                last_updated: Some("2026-06-22 10:00".to_string()),
                ..Default::default()
            },
        );
        backend.write_week(2026, 25, &file).await.unwrap();

        // 1-char query returns empty — guards against noise + slow
        // scans on single-character searches.
        let hits = compute_search(&backend, "s", &[]).await.unwrap();
        assert!(hits.is_empty(), "1-char query should return empty");
        // Whitespace-only query trimmed to empty — same treatment.
        let hits = compute_search(&backend, "   ", &[]).await.unwrap();
        assert!(hits.is_empty(), "whitespace-only query should return empty");
    }

    #[tokio::test]
    async fn search_is_case_insensitive_and_orders_newest_first() {
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());

        // Two years, three files, all mention "Release" in different casings.
        // Query "release" (lower) should match all three.
        let s_2025 = WeeklySummary {
            key_accomplishments: "worked on RELEASE prep".to_string(),
            last_updated: Some("2025-03-01 09:00".to_string()),
            ..Default::default()
        };
        let s_2026_25 = WeeklySummary {
            plans_and_priorities: "Cut the release next Monday.".to_string(),
            last_updated: Some("2026-06-22 10:00".to_string()),
            ..Default::default()
        };
        let s_2026_26 = WeeklySummary {
            key_accomplishments: "release shipped 🚀".to_string(),
            last_updated: Some("2026-06-29 10:00".to_string()),
            ..Default::default()
        };

        backend
            .write_week(2025, 10, &build_weekly_file_with_summary(2025, 10, &s_2025))
            .await
            .unwrap();
        backend
            .write_week(2026, 25, &build_weekly_file_with_summary(2026, 25, &s_2026_25))
            .await
            .unwrap();
        backend
            .write_week(2026, 26, &build_weekly_file_with_summary(2026, 26, &s_2026_26))
            .await
            .unwrap();

        let hits = compute_search(&backend, "release", &[]).await.unwrap();
        assert_eq!(hits.len(), 3, "all three summaries mention release");
        assert_eq!((hits[0].year, hits[0].week), (2026, 26));
        assert_eq!((hits[1].year, hits[1].week), (2026, 25));
        assert_eq!((hits[2].year, hits[2].week), (2025, 10));

        // Each result has at least one snippet with the match visible
        // (post-collapse, case may vary). Verify substring survives.
        for r in &hits {
            assert!(!r.snippets.is_empty(), "each match ships ≥ 1 snippet");
            assert!(
                r.snippets[0]
                    .snippet
                    .to_lowercase()
                    .contains("release"),
                "snippet should contain the query (case-insensitively)"
            );
        }
    }

    #[tokio::test]
    async fn search_label_filter_narrows_results() {
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());

        // Two summaries mention "release"; one has label "mage", the other has "live".
        let s25 = WeeklySummary {
            key_accomplishments: "release prep".to_string(),
            labels: vec!["mage".to_string()],
            last_updated: Some("2026-06-22 10:00".to_string()),
            ..Default::default()
        };
        let s26 = WeeklySummary {
            key_accomplishments: "release cut".to_string(),
            labels: vec!["live".to_string()],
            last_updated: Some("2026-06-29 10:00".to_string()),
            ..Default::default()
        };
        backend
            .write_week(2026, 25, &build_weekly_file_with_summary(2026, 25, &s25))
            .await
            .unwrap();
        backend
            .write_week(2026, 26, &build_weekly_file_with_summary(2026, 26, &s26))
            .await
            .unwrap();

        // Filter to just "mage" — should surface only week 25.
        let hits = compute_search(&backend, "release", &["mage"]).await.unwrap();
        assert_eq!(hits.len(), 1, "label filter narrows to one week");
        assert_eq!((hits[0].year, hits[0].week), (2026, 25));

        // Empty filter is a no-op.
        let hits = compute_search(&backend, "release", &[]).await.unwrap();
        assert_eq!(hits.len(), 2);
    }

    #[tokio::test]
    async fn search_counts_all_matches_and_caps_snippets() {
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());

        // Summary with 5 occurrences of "foo" — total_matches = 5,
        // snippets capped at MAX_SNIPPETS_PER_RESULT (3).
        let s = WeeklySummary {
            key_accomplishments: "foo one, foo two, foo three".to_string(),
            plans_and_priorities: "plan foo four".to_string(),
            challenges_or_roadblocks: "challenge foo five".to_string(),
            last_updated: Some("2026-06-22 10:00".to_string()),
            ..Default::default()
        };
        backend
            .write_week(2026, 25, &build_weekly_file_with_summary(2026, 25, &s))
            .await
            .unwrap();

        let hits = compute_search(&backend, "foo", &[]).await.unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].total_matches, 5, "counts every occurrence");
        assert_eq!(
            hits[0].snippets.len(),
            MAX_SNIPPETS_PER_RESULT,
            "snippets capped at MAX_SNIPPETS_PER_RESULT"
        );
    }

    #[tokio::test]
    async fn search_treats_regex_metacharacters_as_literal() {
        // Locked design: literal substring only, no regex. A query
        // containing `.*` should match a source containing the LITERAL
        // characters, not "any character zero-or-more times."
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        let s = WeeklySummary {
            key_accomplishments: "pattern like [a-z]+ and .* and $end".to_string(),
            last_updated: Some("2026-06-22 10:00".to_string()),
            ..Default::default()
        };
        backend
            .write_week(2026, 25, &build_weekly_file_with_summary(2026, 25, &s))
            .await
            .unwrap();

        // Literal match against a regex metacharacter cluster.
        let hits = compute_search(&backend, "[a-z]+", &[]).await.unwrap();
        assert_eq!(hits.len(), 1, "literal [a-z]+ should match the source");
        assert!(hits[0].snippets[0].snippet.contains("[a-z]+"));

        // Literal `.*` — if regex were interpreted, this would match
        // every character and blow up total_matches. Verify it's 1.
        let hits = compute_search(&backend, ".*", &[]).await.unwrap();
        assert_eq!(hits[0].total_matches, 1, ".* should match once as literal");
    }

    #[tokio::test]
    async fn search_finds_matches_across_summary_fields() {
        // "project" appears once in Key accomplishments and once in
        // Plans — total_matches must be 2. Guards against the fields
        // being scanned in isolation and missing cross-field counting.
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        let s = WeeklySummary {
            key_accomplishments: "finished the project".to_string(),
            plans_and_priorities: "project planning for next week".to_string(),
            last_updated: Some("2026-06-22 10:00".to_string()),
            ..Default::default()
        };
        backend
            .write_week(2026, 25, &build_weekly_file_with_summary(2026, 25, &s))
            .await
            .unwrap();

        let hits = compute_search(&backend, "project", &[]).await.unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(
            hits[0].total_matches, 2,
            "should count matches in both Key accomplishments and Plans"
        );
    }

    // ----- Slice 2: Note-body search -----

    /// Build a weekly file with a summary AND one or more Notes appended.
    /// Each Note is `### YYYY-MM-DD HH:MM — Title` + optional labels
    /// line + body. Ordering matches document order — first entry lands
    /// at the top of the "## Weekly Notes" region, subsequent entries
    /// follow underneath.
    fn build_weekly_file_with_notes(
        year: u32,
        week: u32,
        summary: &WeeklySummary,
        notes: &[(&str, Option<&str>, &[&str], &str)],
    ) -> String {
        use crate::notes::{
            replace_weekly_summary_in_file, weekly_file_scaffold,
        };
        let now = chrono::DateTime::<chrono::FixedOffset>::parse_from_rfc3339(
            "2026-06-22T10:00:00-04:00",
        )
        .unwrap();
        let scaffold = weekly_file_scaffold(year, week, now);
        let mut file = replace_weekly_summary_in_file(&scaffold, summary);
        for (ts, title, labels, body) in notes {
            file.push_str("\n### ");
            file.push_str(ts);
            if let Some(t) = title {
                file.push_str(" — ");
                file.push_str(t);
            }
            file.push('\n');
            if !labels.is_empty() {
                file.push_str("**Labels:** ");
                for (i, l) in labels.iter().enumerate() {
                    if i > 0 {
                        file.push(' ');
                    }
                    file.push('#');
                    file.push_str(l);
                }
                file.push('\n');
            }
            file.push('\n');
            file.push_str(body);
            file.push('\n');
        }
        file
    }

    #[tokio::test]
    async fn search_finds_matches_in_note_bodies() {
        // A weekly file with an empty Summary and two notes — search
        // must surface the Note whose body contains the query.
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        let s = WeeklySummary {
            last_updated: Some("2026-06-22 10:00".to_string()),
            ..Default::default()
        };
        let file = build_weekly_file_with_notes(
            2026,
            25,
            &s,
            &[
                (
                    "2026-06-22 10:15",
                    Some("Kickoff"),
                    &[],
                    "Started the release prep call.",
                ),
                (
                    "2026-06-22 14:00",
                    Some("Wrapup"),
                    &[],
                    "Unrelated content.",
                ),
            ],
        );
        backend.write_week(2026, 25, &file).await.unwrap();

        let hits = compute_search(&backend, "release", &[]).await.unwrap();
        assert_eq!(hits.len(), 1, "only the first note's body mentions release");
        assert!(matches!(hits[0].kind, SearchResultKind::Note));
        assert_eq!(
            hits[0].note_timestamp.as_deref(),
            Some("2026-06-22 10:15"),
            "timestamp identifies WHICH note matched"
        );
        assert_eq!(hits[0].note_title.as_deref(), Some("Kickoff"));
        assert!(
            hits[0].scroll_offset > 0,
            "scroll_offset points to the note heading in the source file"
        );
    }

    #[tokio::test]
    async fn search_scroll_offset_points_at_note_heading() {
        // Verify scroll_offset lands exactly at the "### " bytes of the
        // matching note so MarkdownEditor can scroll the user to the
        // top of that note.
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        let s = WeeklySummary {
            last_updated: Some("2026-06-22 10:00".to_string()),
            ..Default::default()
        };
        let file = build_weekly_file_with_notes(
            2026,
            25,
            &s,
            &[(
                "2026-06-22 10:15",
                Some("Marker note"),
                &[],
                "unique_haystack_token here",
            )],
        );
        backend.write_week(2026, 25, &file).await.unwrap();

        let hits = compute_search(&backend, "unique_haystack_token", &[])
            .await
            .unwrap();
        assert_eq!(hits.len(), 1);
        let offset = hits[0].scroll_offset as usize;
        // The heading starts with "### " — bytes at `offset` should
        // be exactly that prefix.
        assert_eq!(&file[offset..offset + 4], "### ");
        // And the heading line at offset should reference our note.
        let line_end = file[offset..]
            .find('\n')
            .map(|i| offset + i)
            .unwrap_or(file.len());
        assert!(file[offset..line_end].contains("Marker note"));
    }

    #[tokio::test]
    async fn search_both_summary_and_note_surface_in_same_week() {
        // Same query hits both surfaces in the same week — should
        // produce two results with Summary first, then Note.
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        let s = WeeklySummary {
            key_accomplishments: "shipped the release".to_string(),
            last_updated: Some("2026-06-22 10:00".to_string()),
            ..Default::default()
        };
        let file = build_weekly_file_with_notes(
            2026,
            25,
            &s,
            &[(
                "2026-06-22 10:15",
                Some("Post-release retro"),
                &[],
                "release retrospective notes here",
            )],
        );
        backend.write_week(2026, 25, &file).await.unwrap();

        let hits = compute_search(&backend, "release", &[]).await.unwrap();
        assert_eq!(hits.len(), 2, "one Summary result + one Note result");
        assert!(matches!(hits[0].kind, SearchResultKind::Summary));
        assert!(matches!(hits[1].kind, SearchResultKind::Note));
        assert_eq!(hits[0].scroll_offset, 0, "Summary scrolls to top");
        assert!(hits[1].scroll_offset > 0, "Note scrolls to its heading");
    }

    #[tokio::test]
    async fn search_label_filter_applies_to_notes() {
        // Notes have their own `**Labels:**` lines. The filter should
        // let a note through when ITS labels overlap, independent of
        // whether the Summary's labels match.
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        let s = WeeklySummary {
            // Summary has no labels; a global "release" filter should
            // skip the Summary entirely.
            key_accomplishments: "release week overall".to_string(),
            last_updated: Some("2026-06-22 10:00".to_string()),
            ..Default::default()
        };
        let file = build_weekly_file_with_notes(
            2026,
            25,
            &s,
            &[
                (
                    "2026-06-22 10:15",
                    Some("Tagged"),
                    &["release"],
                    "release detail with #release tag",
                ),
                (
                    "2026-06-22 14:00",
                    Some("Untagged"),
                    &[],
                    "release detail with no tag",
                ),
            ],
        );
        backend.write_week(2026, 25, &file).await.unwrap();

        // Global filter: only surfaces with labels containing "release"
        // survive. Summary has no labels → dropped. First note has
        // #release → surfaces. Second note has no labels → dropped.
        let hits = compute_search(&backend, "release", &["release"])
            .await
            .unwrap();
        assert_eq!(hits.len(), 1, "only the labeled note passes the filter");
        assert!(matches!(hits[0].kind, SearchResultKind::Note));
        assert_eq!(hits[0].note_title.as_deref(), Some("Tagged"));
    }

    #[tokio::test]
    async fn search_caps_total_results_at_max() {
        // Regression test for the "search 'qa' hangs the UI" bug —
        // a common query on a dense journal was producing thousands
        // of results that stalled the frontend render. The command
        // now stops scanning further weeks once MAX_RESULTS is hit.
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());

        // Seed enough weeks each carrying a matching Summary to blow
        // past the cap. 250 weeks > MAX_RESULTS (200), so the cap
        // must engage.
        for week in 1..=52u32 {
            let s = WeeklySummary {
                key_accomplishments: "qa work this week".to_string(),
                last_updated: Some("2026-06-22 10:00".to_string()),
                ..Default::default()
            };
            backend
                .write_week(2024, week, &build_weekly_file_with_summary(2024, week, &s))
                .await
                .unwrap();
            backend
                .write_week(2025, week, &build_weekly_file_with_summary(2025, week, &s))
                .await
                .unwrap();
            backend
                .write_week(2026, week, &build_weekly_file_with_summary(2026, week, &s))
                .await
                .unwrap();
        }
        // 156 Summary results — under the cap. Adds an extra 50
        // Notes-heavy weeks to push over 200. Each note carries "qa".
        for week in 40..=52u32 {
            let s = WeeklySummary {
                last_updated: Some("2026-06-22 10:00".to_string()),
                ..Default::default()
            };
            let file = build_weekly_file_with_notes(
                2023,
                week,
                &s,
                &[
                    ("2023-01-01 10:00", None, &[], "qa note one"),
                    ("2023-01-01 11:00", None, &[], "qa note two"),
                    ("2023-01-01 12:00", None, &[], "qa note three"),
                    ("2023-01-01 13:00", None, &[], "qa note four"),
                    ("2023-01-01 14:00", None, &[], "qa note five"),
                ],
            );
            backend.write_week(2023, week, &file).await.unwrap();
        }

        let hits = compute_search(&backend, "qa", &[]).await.unwrap();
        assert_eq!(
            hits.len(),
            MAX_RESULTS,
            "search must cap at MAX_RESULTS on dense queries"
        );
        // Newest-first ordering is preserved even under the cap —
        // 2026 weeks come first, then 2025, then 2024, then 2023.
        assert_eq!(hits[0].year, 2026, "newest-first ordering survives cap");
    }

    #[test]
    fn scan_matches_survives_lowercase_byte_length_changes() {
        // Regression test for the "search hangs on 'qa'" bug. Some
        // Unicode characters change byte length when lowercased —
        // Turkish `İ` (2 bytes) → `i̇` (3 bytes: i + combining dot).
        // If we compute match offsets in the lowercased string and then
        // slice the ORIGINAL for the snippet, the offsets don't align
        // and we get a panic that Tauri swallows into a hanging Promise.
        //
        // Reproduces the exact pattern from Chris's journal: a note
        // whose body contains such a character upstream of a search
        // match. Fix: build snippets from a byte-aligned source.
        //
        // If build_snippet uses `&source[start..end]` directly and the
        // offsets don't align, this call panics. Post-fix, it returns
        // a valid string (possibly empty on truly invalid indices).
        let haystack = "İ some prefix and QA testing here".to_string();
        let haystack_lower = haystack.to_lowercase();
        // Sanity check: this content actually triggers the byte drift.
        assert_ne!(
            haystack.len(),
            haystack_lower.len(),
            "test setup: to_lowercase must shift byte offsets for this case"
        );
        // Find "qa" in the lowercased string — its position is one
        // byte later than in the original because of the İ drift.
        let match_start = haystack_lower.find("qa").expect("qa exists");
        let match_end = match_start + 2;
        // This must NOT panic. Post-fix it returns a best-effort snippet.
        let snippet = build_snippet(&haystack, match_start, match_end);
        // We don't assert on snippet contents (they'll be case-shifted
        // near the drift boundary) — the whole point is not panicking.
        // Snippet may be empty in truly-degenerate cases; that's OK.
        assert!(snippet.len() <= 500, "snippet stays bounded");
    }

    #[tokio::test]
    async fn search_survives_journal_with_unicode_case_drift() {
        // End-to-end regression: a journal file whose Note body starts
        // with a byte-length-changing Unicode char must not hang or
        // crash the search command. The pattern that broke was: 'qa'
        // query against content where an earlier Unicode char shifted
        // downstream byte positions.
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        let s = WeeklySummary {
            last_updated: Some("2026-06-22 10:00".to_string()),
            ..Default::default()
        };
        let file = build_weekly_file_with_notes(
            2026,
            25,
            &s,
            &[(
                "2026-06-22 10:15",
                Some("Byte-drift note"),
                &[],
                // Turkish capital I with dot — 2 bytes → 3 bytes when
                // lowercased. "QA" comes AFTER it, so match offsets in
                // the lowercased haystack shift out of alignment with
                // the original haystack.
                "prefix İ context — QA analyst work continues here",
            )],
        );
        backend.write_week(2026, 25, &file).await.unwrap();

        // Must return successfully — the ONLY way this test passes is
        // if build_snippet stays panic-free on drifting offsets.
        let hits = compute_search(&backend, "qa", &[]).await.unwrap();
        assert_eq!(hits.len(), 1, "should find the QA reference");
        assert!(matches!(hits[0].kind, SearchResultKind::Note));
    }

    #[tokio::test]
    async fn search_handles_unicode_in_queries_and_content() {
        // Multi-byte content — emoji + accented char. build_snippet's
        // char-boundary walk must not panic; find() must locate both.
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        let s = WeeklySummary {
            key_accomplishments: "Shipped 🚀 with café improvements".to_string(),
            last_updated: Some("2026-06-22 10:00".to_string()),
            ..Default::default()
        };
        backend
            .write_week(2026, 25, &build_weekly_file_with_summary(2026, 25, &s))
            .await
            .unwrap();

        let hits = compute_search(&backend, "🚀", &[]).await.unwrap();
        assert_eq!(hits.len(), 1, "should find emoji in content");
        assert!(hits[0].snippets[0].snippet.contains('🚀'));

        let hits = compute_search(&backend, "café", &[]).await.unwrap();
        assert_eq!(hits.len(), 1, "should find accented word");
        assert!(hits[0].snippets[0].snippet.contains("café"));
    }

    // ----- rename_label -----
    //
    // The Tauri command wrapper takes a `State` and can't be unit-tested
    // directly; `rename_label_impl` is the seam these exercise. Coverage
    // mirrors the slice plan's required cases plus a partial-failure path
    // proving we don't roll back when one file errors out.

    #[tokio::test]
    async fn rename_label_simple_case() {
        // Two weekly files mention `release` in their Summary subsections.
        // After rename, both files must read `shipped` and labels.json must
        // carry the renamed entry with its original count + dates intact.
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());

        let f1 = build_weekly_file(2026, 25, &["release"], &[]);
        backend.write_week(2026, 25, &f1).await.unwrap();
        let f2 = build_weekly_file(2026, 26, &["release"], &["release"]);
        backend.write_week(2026, 26, &f2).await.unwrap();

        rebuild_label_index_impl(&backend).await.unwrap();

        let result = rename_label_impl(&backend, "release", "shipped")
            .await
            .unwrap();
        assert_eq!(result.files_modified, 2, "both files held an occurrence");
        // 1 site in W25 (summary), 2 sites in W26 (summary + note line) = 3.
        assert_eq!(result.occurrences_replaced, 3);
        assert!(result.failed_files.is_empty());

        // Disk content must no longer mention `#release` in either file's
        // explicit-labels sites.
        let w25 = backend.read_week(2026, 25).await.unwrap().unwrap();
        assert!(w25.contains("#shipped"), "W25 summary should hold #shipped");
        assert!(!w25.contains("#release"), "W25 must not still hold #release");
        let w26 = backend.read_week(2026, 26).await.unwrap().unwrap();
        assert!(w26.contains("#shipped"));
        assert!(!w26.contains("#release"));

        // labels.json: old gone, new present, with the original count + dates.
        let idx = LabelIndex::load(&backend).await.unwrap();
        assert!(idx.labels.iter().all(|e| e.name != "release"));
        let entry = idx.labels.iter().find(|e| e.name == "shipped").unwrap();
        assert_eq!(entry.count, 2, "rebuild counted W25 + W26 as 2 file occurrences");
    }

    #[tokio::test]
    async fn rename_dedupes_when_destination_already_on_same_line() {
        // A Note's labels line already carries `#shipped`. Renaming
        // `release → shipped` on that same line must drop the renamed
        // token, not produce `#shipped #shipped`.
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());

        // Note line has BOTH labels; Summary subsection has only release.
        let content = build_weekly_file(2026, 25, &["release"], &["shipped", "release"]);
        backend.write_week(2026, 25, &content).await.unwrap();
        rebuild_label_index_impl(&backend).await.unwrap();

        let result = rename_label_impl(&backend, "release", "shipped")
            .await
            .unwrap();
        assert_eq!(result.files_modified, 1);
        // 2 sites touched (summary + note line), each consumed one #release
        // → 2 replacements (one is a swap, one is a drop-dedup).
        assert_eq!(result.occurrences_replaced, 2);

        let written = backend.read_week(2026, 25).await.unwrap().unwrap();
        // Exactly one `#shipped` in the Note labels line — the renamed
        // copy must have been deduped against the existing destination.
        let note_line_count = written
            .lines()
            .find(|l| l.starts_with("**Labels:**"))
            .expect("note labels line should still be present");
        assert_eq!(
            note_line_count.matches("#shipped").count(),
            1,
            "expected dedup to leave a single #shipped on the note line, got {note_line_count:?}"
        );
        assert!(!written.contains("#release"));
    }

    #[tokio::test]
    async fn rename_merges_labels_json_entries_preferring_destination_color() {
        // Both `release` and `shipped` already exist in labels.json with
        // different color overrides + counts + date ranges. After rename,
        // the merged entry must keep the DESTINATION's color (locked
        // decision #5), sum the counts, and span min(first_used) +
        // max(last_used).
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());

        // Seed weekly files so the rebuild produces real entries with
        // distinct date ranges. W3 (early) for release, W40 (late) for
        // shipped, plus an extra W25 for release so its count is 2.
        let f3 = build_weekly_file(2026, 3, &["release"], &[]);
        backend.write_week(2026, 3, &f3).await.unwrap();
        let f25 = build_weekly_file(2026, 25, &["release"], &[]);
        backend.write_week(2026, 25, &f25).await.unwrap();
        let f40 = build_weekly_file(2026, 40, &["shipped"], &[]);
        backend.write_week(2026, 40, &f40).await.unwrap();
        rebuild_label_index_impl(&backend).await.unwrap();

        // Pin distinct colors on each entry — destination's must win.
        set_label_color_impl(&backend, "release", Some("#aa0000".to_string()))
            .await
            .unwrap();
        set_label_color_impl(&backend, "shipped", Some("#00aa00".to_string()))
            .await
            .unwrap();

        let result = rename_label_impl(&backend, "release", "shipped")
            .await
            .unwrap();
        assert_eq!(result.files_modified, 2, "W3 + W25 both held #release");

        let idx = LabelIndex::load(&backend).await.unwrap();
        assert!(idx.labels.iter().all(|e| e.name != "release"));
        let merged = idx.labels.iter().find(|e| e.name == "shipped").unwrap();
        // release count 2 (W3 + W25) + shipped count 1 (W40) = 3.
        assert_eq!(merged.count, 3, "counts must be summed across the merge");
        assert_eq!(
            merged.first_used,
            NaiveDate::from_ymd_opt(2026, 1, 12).unwrap(),
            "first_used = min(release.first_used, shipped.first_used) = W3 Monday"
        );
        assert_eq!(
            merged.last_used,
            NaiveDate::from_ymd_opt(2026, 9, 28).unwrap(),
            "last_used = max(...) = W40 Monday"
        );
        // Destination's existing color wins per locked decision #5.
        assert_eq!(merged.color.as_deref(), Some("#00aa00"));
    }

    #[tokio::test]
    async fn rename_rejects_invalid_chars() {
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());

        // Space, period, slash are all outside `is_label_char` and must be
        // rejected. We don't need any seeded data — validation happens
        // before any I/O.
        for bad in &["with space", "has.dot", "slash/here", "with!bang"] {
            let err = rename_label_impl(&backend, "release", bad)
                .await
                .expect_err(&format!("expected rename to {bad:?} to fail"));
            assert!(
                err.contains("invalid character"),
                "error should name the invalid character, got: {err}"
            );
        }
    }

    #[tokio::test]
    async fn rename_rejects_empty_new_name() {
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());

        // Empty, whitespace-only, and bare `#` (which strips to empty) all
        // hit the same branch — surface a clear error rather than letting
        // an empty rename silently succeed.
        for bad in &["", "   ", "#"] {
            let err = rename_label_impl(&backend, "release", bad)
                .await
                .expect_err(&format!("expected rename to {bad:?} to fail"));
            assert!(
                err.contains("must not be empty"),
                "expected 'must not be empty' error, got: {err}"
            );
        }
    }

    #[tokio::test]
    async fn rename_no_op_returns_err() {
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());

        let err = rename_label_impl(&backend, "release", "release")
            .await
            .expect_err("renaming a label to itself should be an Err");
        assert!(
            err.contains("no-op"),
            "expected 'no-op' in error, got: {err}"
        );

        // Leading `#` on either side strips before the comparison, so
        // `#release` → `release` is also a no-op.
        let err = rename_label_impl(&backend, "#release", "release")
            .await
            .expect_err("# stripping should make this a no-op");
        assert!(err.contains("no-op"), "got: {err}");
    }

    #[tokio::test]
    async fn rename_continues_after_per_file_read_error() {
        // Per locked decision #7, a per-file read error doesn't abort the
        // rename pass — the bad file lands in `failed_files`, every other
        // weekly file gets rewritten, and labels.json still updates.
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());

        let good = build_weekly_file(2026, 25, &["release"], &[]);
        backend.write_week(2026, 25, &good).await.unwrap();
        let good2 = build_weekly_file(2026, 26, &["release"], &[]);
        backend.write_week(2026, 26, &good2).await.unwrap();
        rebuild_label_index_impl(&backend).await.unwrap();

        // Replace W25's file with a directory so the read fails — same
        // trick the rebuild test uses.
        let bad_path = dir.path().join("2026").join("2026-W25.md");
        tokio::fs::remove_file(&bad_path).await.unwrap();
        tokio::fs::create_dir_all(&bad_path).await.unwrap();

        let result = rename_label_impl(&backend, "release", "shipped")
            .await
            .unwrap();
        // W26 rewritten; W25 reported.
        assert_eq!(result.files_modified, 1, "W26 must have been rewritten");
        assert_eq!(result.failed_files.len(), 1);
        assert!(
            result.failed_files[0].contains("2026-W25"),
            "failed_files should name the bad file, got {:?}",
            result.failed_files
        );

        // W26 disk content reflects the rename.
        let w26 = backend.read_week(2026, 26).await.unwrap().unwrap();
        assert!(w26.contains("#shipped"));
        assert!(!w26.contains("#release"));

        // labels.json still updated despite the partial failure.
        let idx = LabelIndex::load(&backend).await.unwrap();
        assert!(idx.labels.iter().any(|e| e.name == "shipped"));
        assert!(idx.labels.iter().all(|e| e.name != "release"));
    }

    // ----- delete_label_cascade -----
    //
    // The Tauri command wrapper takes a `State` and isn't unit-testable
    // directly; `delete_label_cascade_impl` is the seam these exercise.
    // Coverage mirrors the slice plan's required cases plus the
    // load-bearing `delete_does_not_touch_inline_hashtags` proof that the
    // cascade respects locked-decision #2 (explicit arrays only).

    #[tokio::test]
    async fn delete_strips_from_note_labels_arrays() {
        // Two weekly files carry `#release` in Note `**Labels:**` lines.
        // After delete, neither file's note line should still hold the
        // token, and labels.json should drop the entry.
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());

        // Pair `release` with a sibling label so the labels line stays
        // non-empty after the strip — exercises the "rebuild with remaining
        // names" branch.
        let f1 = build_weekly_file(2026, 25, &[], &["release", "planning"]);
        backend.write_week(2026, 25, &f1).await.unwrap();
        let f2 = build_weekly_file(2026, 26, &[], &["release", "planning"]);
        backend.write_week(2026, 26, &f2).await.unwrap();
        rebuild_label_index_impl(&backend).await.unwrap();

        let result = delete_label_cascade_impl(&backend, "release")
            .await
            .unwrap();
        assert_eq!(result.files_modified, 2, "both files held an occurrence");
        assert_eq!(result.occurrences_removed, 2);
        assert!(result.failed_files.is_empty());

        let w25 = backend.read_week(2026, 25).await.unwrap().unwrap();
        let w26 = backend.read_week(2026, 26).await.unwrap().unwrap();
        assert!(!w25.contains("#release"), "W25 must not still hold #release");
        assert!(w25.contains("#planning"), "sibling label must survive");
        assert!(!w26.contains("#release"));
        assert!(w26.contains("#planning"));

        // The note labels lines should still exist (they're not empty),
        // just without #release.
        let note_line_25 = w25
            .lines()
            .find(|l| l.starts_with("**Labels:**"))
            .expect("W25 note labels line should survive");
        assert!(note_line_25.contains("#planning"));
        assert!(!note_line_25.contains("#release"));
    }

    #[tokio::test]
    async fn delete_strips_from_summary_labels_subsections() {
        // Same shape as the notes-arrays test but for Weekly Summary
        // `### Labels` subsections.
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());

        let f1 = build_weekly_file(2026, 25, &["release", "planning"], &[]);
        backend.write_week(2026, 25, &f1).await.unwrap();
        let f2 = build_weekly_file(2026, 26, &["release", "planning"], &[]);
        backend.write_week(2026, 26, &f2).await.unwrap();
        rebuild_label_index_impl(&backend).await.unwrap();

        let result = delete_label_cascade_impl(&backend, "release")
            .await
            .unwrap();
        assert_eq!(result.files_modified, 2);
        assert_eq!(result.occurrences_removed, 2);
        assert!(result.failed_files.is_empty());

        // Re-parse so we read the actual subsection body, not just any
        // substring of the file (`#release` could survive elsewhere if the
        // test seeds drift).
        let w25 = backend.read_week(2026, 25).await.unwrap().unwrap();
        let summary = parse_weekly_summary(&w25);
        assert_eq!(
            summary.labels,
            vec!["planning".to_string()],
            "summary labels should be exactly [planning]"
        );
    }

    #[tokio::test]
    async fn delete_drops_empty_note_labels_line_entirely() {
        // Note's labels line carried ONLY `#release`. After delete, the
        // entire `**Labels:**` line must be gone from the file — not left
        // behind as a bare `**Labels:**` with no chips.
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());

        let f1 = build_weekly_file(2026, 25, &[], &["release"]);
        backend.write_week(2026, 25, &f1).await.unwrap();
        rebuild_label_index_impl(&backend).await.unwrap();

        // Sanity: the seeded file has a labels line before the delete.
        let before = backend.read_week(2026, 25).await.unwrap().unwrap();
        assert!(
            before.lines().any(|l| l.starts_with("**Labels:**")),
            "seeded file should carry a **Labels:** line"
        );

        let result = delete_label_cascade_impl(&backend, "release")
            .await
            .unwrap();
        assert_eq!(result.files_modified, 1);
        assert_eq!(result.occurrences_removed, 1);

        let after = backend.read_week(2026, 25).await.unwrap().unwrap();
        assert!(
            !after.lines().any(|l| l.starts_with("**Labels:**")),
            "empty-after-delete must drop the **Labels:** line entirely, got:\n{after}"
        );
        // And no bare `**Labels:**` anywhere in the file — the line is gone.
        assert!(!after.contains("**Labels:**"));
    }

    #[tokio::test]
    async fn delete_keeps_empty_summary_labels_header() {
        // Summary subsection carried ONLY `#release`. After delete, the
        // `### Labels` heading must survive (locked-decision #6) with an
        // empty body line — matching the empty-summary scaffold.
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());

        let f1 = build_weekly_file(2026, 25, &["release"], &[]);
        backend.write_week(2026, 25, &f1).await.unwrap();
        rebuild_label_index_impl(&backend).await.unwrap();

        let result = delete_label_cascade_impl(&backend, "release")
            .await
            .unwrap();
        assert_eq!(result.files_modified, 1);
        assert_eq!(result.occurrences_removed, 1);

        let after = backend.read_week(2026, 25).await.unwrap().unwrap();
        // Header survives.
        assert!(
            after.contains("### Labels\n"),
            "### Labels heading must remain after the body empties; got:\n{after}"
        );
        // Parsed summary's labels list is empty.
        let summary = parse_weekly_summary(&after);
        assert!(
            summary.labels.is_empty(),
            "summary.labels must be empty after delete, got {:?}",
            summary.labels
        );
    }

    #[tokio::test]
    async fn delete_does_not_touch_inline_hashtags() {
        // LOAD-BEARING per the slice plan: the cascade strips ONLY explicit
        // labels arrays. Inline `#hashtag` text in note prose must survive
        // byte-for-byte — that's locked-decision #2.
        //
        // Strategy: seed a file with BOTH a labels-array occurrence of
        // `release` AND inline `#release` references in note body prose.
        // Snapshot the prose region of the file before the delete; after
        // the delete, the labels array entry is gone but the prose region
        // is unchanged byte-for-byte.
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());

        // Build a weekly file with an explicit Note labels-array containing
        // `release`, plus a Note BODY chunk that mentions `#release` and
        // `#release-train` inline. We append the inline-tag note after the
        // helper-built file so the prose is on disk verbatim.
        let mut content = build_weekly_file(2026, 25, &[], &["release"]);
        let inline_section = "\n### 2026-06-23 14:30 — Stand-up notes\n\
            Discussed the #release timeline and the #release-train cadence. \
            See also #release for context.\n\n";
        content.push_str(inline_section);
        backend.write_week(2026, 25, &content).await.unwrap();
        rebuild_label_index_impl(&backend).await.unwrap();

        // Snapshot the inline section so we can assert it survives verbatim.
        let snapshot_inline = inline_section.to_string();

        let result = delete_label_cascade_impl(&backend, "release")
            .await
            .unwrap();
        assert_eq!(result.files_modified, 1);
        // Only ONE occurrence removed — the labels-array entry. The three
        // inline `#release` / `#release-train` mentions in the body prose
        // are NOT counted as occurrences.
        assert_eq!(
            result.occurrences_removed, 1,
            "only the labels-array entry should count as a removal"
        );

        let after = backend.read_week(2026, 25).await.unwrap().unwrap();

        // The labels-array `**Labels:**` line that held `#release` should
        // have been dropped (it had only that one label).
        assert!(
            !after.lines().any(|l| l == "**Labels:** #release"),
            "labels-array entry should be gone"
        );

        // The inline prose region must still be in the file, byte-for-byte.
        // We tolerate the surrounding labels-line being dropped above the
        // section by checking for the inline section's exact substring.
        assert!(
            after.contains(&snapshot_inline),
            "inline prose region must survive byte-for-byte; \
             expected to find:\n{snapshot_inline}\nin:\n{after}"
        );

        // Belt + braces: all three inline mentions are still present.
        assert_eq!(
            after.matches("#release-train").count(),
            1,
            "#release-train must survive"
        );
        // "#release" matches `#release-train` too (it's a substring), so
        // subtract the 1 train mention from the total expected.
        // Expected inline `#release` mentions: 2 (standalone). Plus 1 from
        // `#release-train` = 3 `#release` substring matches total.
        assert_eq!(
            after.matches("#release").count(),
            3,
            "two standalone #release inline mentions + one inside #release-train must survive"
        );
    }

    #[tokio::test]
    async fn delete_removes_labels_json_entry() {
        // After delete, the entry must be gone from labels.json. (The file
        // rewrites are covered by the other tests; this one is purely about
        // the index update.)
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());

        let f1 = build_weekly_file(2026, 25, &["release", "planning"], &[]);
        backend.write_week(2026, 25, &f1).await.unwrap();
        rebuild_label_index_impl(&backend).await.unwrap();

        // Sanity: index carries both entries before the delete.
        {
            let idx = LabelIndex::load(&backend).await.unwrap();
            assert!(idx.labels.iter().any(|e| e.name == "release"));
            assert!(idx.labels.iter().any(|e| e.name == "planning"));
        }

        delete_label_cascade_impl(&backend, "release")
            .await
            .unwrap();

        let idx = LabelIndex::load(&backend).await.unwrap();
        assert!(
            idx.labels.iter().all(|e| e.name != "release"),
            "release entry should be removed from labels.json"
        );
        // Sibling label untouched.
        assert!(
            idx.labels.iter().any(|e| e.name == "planning"),
            "planning entry should survive the delete"
        );
    }

    #[tokio::test]
    async fn delete_continues_after_per_file_read_error() {
        // Per locked decision #7, a per-file read error doesn't abort the
        // cascade — the bad file lands in `failed_files`, every other
        // weekly file gets rewritten, and labels.json still drops the entry.
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());

        let good = build_weekly_file(2026, 25, &["release"], &[]);
        backend.write_week(2026, 25, &good).await.unwrap();
        let good2 = build_weekly_file(2026, 26, &["release"], &[]);
        backend.write_week(2026, 26, &good2).await.unwrap();
        rebuild_label_index_impl(&backend).await.unwrap();

        // Replace W25's file with a directory so the read fails — same
        // trick the rebuild + rename tests use.
        let bad_path = dir.path().join("2026").join("2026-W25.md");
        tokio::fs::remove_file(&bad_path).await.unwrap();
        tokio::fs::create_dir_all(&bad_path).await.unwrap();

        let result = delete_label_cascade_impl(&backend, "release")
            .await
            .unwrap();
        assert_eq!(result.files_modified, 1, "W26 must have been rewritten");
        assert_eq!(result.failed_files.len(), 1);
        assert!(
            result.failed_files[0].contains("2026-W25"),
            "failed_files should name the bad file, got {:?}",
            result.failed_files
        );

        // W26 disk content reflects the delete (Summary subsection emptied,
        // header retained).
        let w26 = backend.read_week(2026, 26).await.unwrap().unwrap();
        assert!(!w26.contains("#release"));
        assert!(w26.contains("### Labels\n"), "header must remain");

        // labels.json updated despite partial failure.
        let idx = LabelIndex::load(&backend).await.unwrap();
        assert!(idx.labels.iter().all(|e| e.name != "release"));
    }

    #[tokio::test]
    async fn delete_handles_label_not_in_index() {
        // Calling delete on a label that doesn't exist in labels.json (and
        // therefore doesn't exist in any explicit-labels site either) must
        // succeed as a no-op — no files touched, no occurrences removed,
        // index unchanged.
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());

        // Seed an unrelated label so the index has content but doesn't
        // mention the target — exercises "load index, find nothing, save
        // unchanged".
        let f1 = build_weekly_file(2026, 25, &["planning"], &[]);
        backend.write_week(2026, 25, &f1).await.unwrap();
        rebuild_label_index_impl(&backend).await.unwrap();

        let before_w25 = backend.read_week(2026, 25).await.unwrap().unwrap();
        let before_idx = LabelIndex::load(&backend).await.unwrap();

        let result = delete_label_cascade_impl(&backend, "does-not-exist")
            .await
            .unwrap();
        assert_eq!(result.files_modified, 0);
        assert_eq!(result.occurrences_removed, 0);
        assert!(result.failed_files.is_empty());

        // Disk content untouched.
        let after_w25 = backend.read_week(2026, 25).await.unwrap().unwrap();
        assert_eq!(
            before_w25, after_w25,
            "no file should be rewritten when the label doesn't exist"
        );

        // Index unchanged (the unrelated label survives, no new entry).
        let after_idx = LabelIndex::load(&backend).await.unwrap();
        assert_eq!(before_idx.labels.len(), after_idx.labels.len());
        assert!(after_idx.labels.iter().any(|e| e.name == "planning"));
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn run_applescript_surfaces_syntax_error_as_err() {
        // A bare identifier with no surrounding `tell` is a parse error;
        // osascript exits non-zero and writes the message to stderr.
        // We don't pin the exact text (osascript localizes it), only that
        // the call fails and the error is non-empty.
        let result = run_applescript("this is not valid applescript".to_string()).await;
        assert!(result.is_err(), "expected Err for syntax error, got {result:?}");
        let err = result.unwrap_err();
        assert!(!err.is_empty(), "expected non-empty stderr message");
        // Must NOT be flagged as permission_denied — the failure mode is a
        // parse error, not Apple Events denial.
        assert!(
            !err.starts_with("permission_denied:"),
            "syntax error wrongly flagged as permission_denied: {err}"
        );
    }
}
