//! Tracks which Weekly Summaries have been emailed to the user's manager.
//!
//! ## Why a sidecar
//!
//! The weekly `.md` files are user-facing content (often hand-edited, version-
//! controlled by users who care about that, exportable). "I emailed this on
//! day X" is operation metadata, not content — so it lives in
//! `<journal_root>/.metadata/sent-log.json` next to the other sidecars
//! (`labels.json`, `settings.json`, `capture-draft.json`) instead of being
//! spliced into the markdown.
//!
//! ## Shape
//!
//! ```json
//! {
//!   "2026-W26": {
//!     "sentAt": "2026-06-24T16:12:33-04:00",
//!     "contentHash": "8f3a…b1d4",
//!     "sentTo": "chris.manager@prodigygame.com"
//!   }
//! }
//! ```
//!
//! One entry per ISO year-week. If the user re-sends after editing a week
//! we overwrite the entry (no history array). This was a deliberate design
//! choice (see ROADMAP Phase 2.6) — keep the schema simple; the latest send
//! is the load-bearing fact.
//!
//! ## Content hashing
//!
//! `hash_weekly_summary` produces a SHA-256 hex of a canonical serialization
//! of the four summary fields + labels. We use the SAME function at send
//! time (to stamp `contentHash` into the entry) and at gate time (to compare
//! against the entry the next time the page loads). If the hash matches, the
//! UI shows "Sent {when}"; if it differs, the summary was edited after the
//! last send and the button re-enables as "Send updated version".

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::notes::WeeklySummary;
use crate::storage::{StorageBackend, StorageError, StorageResult};

pub const SENT_LOG_FILENAME: &str = "sent-log.json";

/// One sent-log entry. Keyed in the outer map by ISO year-week
/// (`"2026-W26"`). Field names serialize as camelCase to match the
/// frontend.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SentRecord {
    /// RFC 3339 timestamp at the moment of send (local time + offset).
    pub sent_at: String,
    /// SHA-256 hex of the canonical WeeklySummary form. Detects edits after send.
    pub content_hash: String,
    /// Manager email address the send was addressed to. Retained even if
    /// the user later changes their manager so the disabled-state tooltip
    /// still reads correctly.
    pub sent_to: String,
}

/// Format a (year, week) pair as the ISO key used in `sent-log.json`
/// (e.g. `2026-W26`). Stable serialization is important — the frontend
/// uses the same shape when querying.
pub fn year_week_key(year: u32, week: u32) -> String {
    format!("{:04}-W{:02}", year, week)
}

/// SHA-256 hex of the canonical WeeklySummary form. Compares stable across
/// frontend and backend because both pass through this same function.
///
/// ## Why length-prefix instead of a separator
///
/// The summary fields are free-form multi-line text (these are textareas;
/// bulleted lists are the norm). A naive separator like `\n` collides on
/// content boundaries — `{key: "a\nb", plans: ""}` and `{key: "a", plans: "b"}`
/// would both feed the hasher the same bytes (`a\nb\n\n\n\n`), so moving a
/// bullet from one section to another would produce the same hash, and the
/// Send button would refuse to re-send a legitimately-edited summary.
///
/// Length-prefixing each field with a fixed-width u64 byte length makes the
/// hash boundary-safe: there is no way for content to "look like" a
/// separator, because the separator IS the length. Labels are prefixed
/// individually instead of joined with a delimiter for the same reason
/// (a label containing a comma would otherwise collide with two labels).
///
/// ## Stability
///
/// This function is the contract between sent-log entries and the gating
/// UI. Any change to the canonicalization invalidates every prior sent-log
/// record (they'd all read as "edited since send" the next time the user
/// loaded /summary). Bump a version constant and migrate sent-log.json if
/// the algorithm ever has to change.
pub fn hash_weekly_summary(summary: &WeeklySummary) -> String {
    let mut hasher = Sha256::new();
    for field in [
        summary.key_accomplishments.as_str(),
        summary.plans_and_priorities.as_str(),
        summary.challenges_or_roadblocks.as_str(),
        summary.anything_else.as_str(),
    ] {
        hasher.update((field.len() as u64).to_le_bytes());
        hasher.update(field.as_bytes());
    }
    // Label count + each label length-prefixed, so adding/removing/
    // reordering labels — or labels containing commas — all change the hash.
    hasher.update((summary.labels.len() as u64).to_le_bytes());
    for label in &summary.labels {
        hasher.update((label.len() as u64).to_le_bytes());
        hasher.update(label.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

/// Read the full sent-log map from disk. Missing file → empty map. Corrupt
/// file → empty map with a stderr warning; we never block the UI on metadata
/// corruption (same posture as labels.json).
pub async fn read_sent_log<B: StorageBackend + ?Sized>(
    backend: &B,
) -> StorageResult<HashMap<String, SentRecord>> {
    match backend.read_metadata(SENT_LOG_FILENAME).await? {
        Some(content) => match serde_json::from_str(&content) {
            Ok(map) => Ok(map),
            Err(e) => {
                eprintln!(
                    "[sent_log] {SENT_LOG_FILENAME} is corrupt ({e}); treating as empty"
                );
                Ok(HashMap::new())
            }
        },
        None => Ok(HashMap::new()),
    }
}

/// Overwrite the sent-log on disk with the given map.
pub async fn write_sent_log<B: StorageBackend + ?Sized>(
    backend: &B,
    map: &HashMap<String, SentRecord>,
) -> StorageResult<()> {
    let content =
        serde_json::to_string_pretty(map).map_err(|e| StorageError::Serde(e.to_string()))?;
    backend.write_metadata(SENT_LOG_FILENAME, &content).await
}

/// Read just the entry for one (year, week). `None` if absent.
pub async fn get_sent_record<B: StorageBackend + ?Sized>(
    backend: &B,
    year: u32,
    week: u32,
) -> StorageResult<Option<SentRecord>> {
    let map = read_sent_log(backend).await?;
    Ok(map.get(&year_week_key(year, week)).cloned())
}

/// Insert (or overwrite) the entry for one (year, week). One entry per week.
pub async fn upsert_sent_record<B: StorageBackend + ?Sized>(
    backend: &B,
    year: u32,
    week: u32,
    record: SentRecord,
) -> StorageResult<()> {
    let mut map = read_sent_log(backend).await?;
    map.insert(year_week_key(year, week), record);
    write_sent_log(backend, &map).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::LocalFilesystem;
    use tempfile::TempDir;

    fn summary_with(key: &str, plans: &str, labels: &[&str]) -> WeeklySummary {
        WeeklySummary {
            key_accomplishments: key.to_string(),
            plans_and_priorities: plans.to_string(),
            challenges_or_roadblocks: String::new(),
            anything_else: String::new(),
            labels: labels.iter().map(|s| s.to_string()).collect(),
            last_updated: None,
            ..Default::default()
        }
    }

    #[test]
    fn year_week_key_pads_short_weeks() {
        assert_eq!(year_week_key(2026, 1), "2026-W01");
        assert_eq!(year_week_key(2026, 26), "2026-W26");
    }

    #[test]
    fn hash_is_stable_across_calls() {
        let s = summary_with("shipped X", "Y, Z", &["mage", "qa"]);
        assert_eq!(hash_weekly_summary(&s), hash_weekly_summary(&s));
    }

    #[test]
    fn hash_changes_when_a_field_changes() {
        let a = summary_with("shipped X", "Y, Z", &["mage"]);
        let mut b = a.clone();
        b.key_accomplishments.push_str("\nand X.1");
        assert_ne!(hash_weekly_summary(&a), hash_weekly_summary(&b));
    }

    #[test]
    fn hash_changes_when_labels_change() {
        let a = summary_with("k", "p", &["mage"]);
        let b = summary_with("k", "p", &["mage", "qa"]);
        assert_ne!(hash_weekly_summary(&a), hash_weekly_summary(&b));
    }

    #[test]
    fn hash_ignores_last_updated() {
        // last_updated is a server-side timestamp, not content — it must
        // not affect the hash (otherwise every save would flip the gate).
        let mut a = summary_with("k", "p", &["x"]);
        let mut b = a.clone();
        a.last_updated = Some("2026-06-24 10:00".to_string());
        b.last_updated = Some("2026-06-25 11:00".to_string());
        assert_eq!(hash_weekly_summary(&a), hash_weekly_summary(&b));
    }

    #[test]
    fn hash_does_not_collide_on_field_boundaries() {
        // If we joined without separators, "ab" + "" + ... could collide with
        // "a" + "b" + ... — the length-prefixing must prevent that.
        let a = summary_with("ab", "", &[]);
        let b = summary_with("a", "b", &[]);
        assert_ne!(hash_weekly_summary(&a), hash_weekly_summary(&b));
    }

    #[test]
    fn hash_does_not_collide_when_field_contains_newline() {
        // The original implementation used `\n` as the field separator. With
        // multi-line textareas (bulleted lists everywhere), a user moving a
        // bullet from key_accomplishments into plans_and_priorities would
        // collide:
        //   A: key="a\nb", plans=""           → "a\nb\n\n\n\n"
        //   B: key="a",    plans="b"          → "a\nb\n\n\n\n"
        // After sending A, editing into B would leave the gate disabled
        // because the hash matched — user could not re-send the edit.
        // Length-prefixing makes this impossible.
        let a = summary_with("a\nb", "", &[]);
        let b = summary_with("a", "b", &[]);
        assert_ne!(
            hash_weekly_summary(&a),
            hash_weekly_summary(&b),
            "moving a line between sections must change the hash"
        );
    }

    #[test]
    fn hash_does_not_collide_when_label_contains_comma() {
        // Same class of bug for labels — the original implementation joined
        // them with `,`, so a label containing a comma would collide with
        // two labels.
        let a = summary_with("k", "p", &["foo,bar"]);
        let b = summary_with("k", "p", &["foo", "bar"]);
        assert_ne!(
            hash_weekly_summary(&a),
            hash_weekly_summary(&b),
            "a label containing a delimiter character must not collide with two labels"
        );
    }

    #[test]
    fn hash_does_not_collide_on_label_count_boundary() {
        // An empty labels array vs a single empty-string label could collide
        // without explicit count prefixing.
        let a = summary_with("k", "p", &[]);
        let b = summary_with("k", "p", &[""]);
        assert_ne!(hash_weekly_summary(&a), hash_weekly_summary(&b));
    }

    #[tokio::test]
    async fn read_sent_log_missing_returns_empty() {
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        let map = read_sent_log(&backend).await.unwrap();
        assert!(map.is_empty());
    }

    #[tokio::test]
    async fn upsert_then_get_roundtrips() {
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        let rec = SentRecord {
            sent_at: "2026-06-24T16:12:33-04:00".to_string(),
            content_hash: "deadbeef".to_string(),
            sent_to: "boss@example.com".to_string(),
        };
        upsert_sent_record(&backend, 2026, 26, rec.clone())
            .await
            .unwrap();

        let loaded = get_sent_record(&backend, 2026, 26).await.unwrap();
        assert_eq!(loaded, Some(rec));
    }

    #[tokio::test]
    async fn upsert_overwrites_existing_entry() {
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        let first = SentRecord {
            sent_at: "2026-06-24T10:00:00-04:00".to_string(),
            content_hash: "aaaa".to_string(),
            sent_to: "v1@example.com".to_string(),
        };
        let second = SentRecord {
            sent_at: "2026-06-24T18:00:00-04:00".to_string(),
            content_hash: "bbbb".to_string(),
            sent_to: "v2@example.com".to_string(),
        };
        upsert_sent_record(&backend, 2026, 26, first).await.unwrap();
        upsert_sent_record(&backend, 2026, 26, second.clone())
            .await
            .unwrap();

        let loaded = get_sent_record(&backend, 2026, 26).await.unwrap();
        assert_eq!(loaded, Some(second));
    }

    #[tokio::test]
    async fn corrupt_sent_log_returns_empty() {
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        backend
            .write_metadata(SENT_LOG_FILENAME, "{ not valid json")
            .await
            .unwrap();

        // Should not error — should silently treat the file as empty.
        let map = read_sent_log(&backend).await.unwrap();
        assert!(map.is_empty());
    }

    #[tokio::test]
    async fn upserts_to_different_weeks_coexist() {
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        let rec_a = SentRecord {
            sent_at: "2026-06-24T10:00:00-04:00".to_string(),
            content_hash: "aaaa".to_string(),
            sent_to: "boss@example.com".to_string(),
        };
        let rec_b = SentRecord {
            sent_at: "2026-07-01T10:00:00-04:00".to_string(),
            content_hash: "bbbb".to_string(),
            sent_to: "boss@example.com".to_string(),
        };
        upsert_sent_record(&backend, 2026, 26, rec_a.clone())
            .await
            .unwrap();
        upsert_sent_record(&backend, 2026, 27, rec_b.clone())
            .await
            .unwrap();

        assert_eq!(get_sent_record(&backend, 2026, 26).await.unwrap(), Some(rec_a));
        assert_eq!(get_sent_record(&backend, 2026, 27).await.unwrap(), Some(rec_b));
    }
}
