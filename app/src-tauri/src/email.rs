//! Compose a weekly summary into a mail-client-openable artifact.
//!
//! The "Send to manager" button on `/summary` calls into [`compose_weekly_email`],
//! which returns one of two things:
//!
//!   - `ComposeResult::Mailto { url }` — a `mailto:?subject=…&body=…` URL the
//!     frontend hands to `tauri-plugin-opener::open_url`. macOS LaunchServices
//!     routes it to the user's default mail client (Mail.app, Mimestream,
//!     Chrome-as-Gmail-handler, etc.), which opens a compose window with
//!     To/Subject/Body pre-filled. The user reviews and sends.
//!
//!   - `ComposeResult::Eml { path }` — an absolute path to a `.eml` file
//!     written to a scratch dir. The frontend opens it via
//!     `tauri-plugin-opener::open_path`. The default `.eml` handler (Mail.app
//!     on most Chris-style Macs) opens it as an editable draft, same UX.
//!
//! The split exists because mailto: has a real URL-length ceiling. On macOS
//! the LaunchServices pipeline truncates mailto URLs around ~2 KB (the classic
//! IE limit, inherited via the URL handler chain). A long week with all four
//! summary fields filled out can comfortably blow past that once URL-encoded.
//! The .eml branch handles arbitrary-length bodies for ~30 lines of std-fs
//! code with no extra deps.

use std::path::PathBuf;

use chrono::{DateTime, FixedOffset};
use serde::Serialize;
use thiserror::Error;
use urlencoding::encode as percent_encode;

use crate::notes::WeeklySummary;

/// Hard ceiling on the encoded mailto URL byte length. Below this, we build
/// a mailto: URL; at or above it, we write a .eml file. The classic
/// LaunchServices-route mailto cap is ~2083 bytes; we leave a safety margin
/// because mail clients also vary (Outlook for Mac is more lenient; Spark
/// less so).
const MAILTO_MAX_BYTES: usize = 1800;

/// Subdirectory under `std::env::temp_dir()` where we drop .eml files. Stable
/// path so the startup janitor knows where to prune.
pub const EML_TEMP_SUBDIR: &str = "captainslog";

/// Public GitHub URL for Captain's Log. The email body links the words
/// "Captain's Log" to this so the manager can poke around (and so the user
/// implicitly attributes the cadence to a real tool, not a one-off email).
/// Kept as a constant so a future fork / rename is one edit.
const CAPTAINS_LOG_REPO_URL: &str = "https://github.com/ChrisACarpenter/captains-log";

#[derive(Debug, Error)]
pub enum EmailError {
    #[error("manager email is empty — set one in Settings before sending")]
    NoRecipient,

    #[error("i/o error writing .eml: {0}")]
    Io(#[from] std::io::Error),
}

pub type EmailResult<T> = Result<T, EmailError>;

/// What the frontend should do next: either open a mailto: URL, or open an
/// .eml file. Two variants instead of one universal "open this" because the
/// `opener` plugin distinguishes them at the capability layer.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "value")]
pub enum ComposeResult {
    Mailto(String),
    Eml(PathBuf),
}

/// Inputs to [`compose_weekly_email`]. Split out as a struct because the call
/// site has six values to thread through and a positional argument list would
/// be easy to get wrong (recipient and manager_name are both `&str`, etc.).
pub struct ComposeParams<'a> {
    pub summary: &'a WeeklySummary,
    pub week_label: &'a str,
    pub recipient: &'a str,
    /// Manager's display name. `None` (or empty after trim) falls back to a
    /// plain "Hello," greeting.
    pub manager_name: Option<&'a str>,
    /// Set to `true` when there's already a sent-log entry for this week
    /// (i.e. the user is resending an edited version). Changes the subject
    /// line to make the thread context clear to the manager.
    pub is_resend: bool,
    pub now: DateTime<FixedOffset>,
}

/// Build the email and decide which delivery channel to use. The decision is
/// purely length-based — if the encoded mailto URL would exceed
/// [`MAILTO_MAX_BYTES`], we fall back to .eml. Otherwise we return the
/// mailto URL directly.
///
/// `recipient` is trimmed before use. Empty (or whitespace-only) recipients
/// return [`EmailError::NoRecipient`] — the frontend gates Send on this too,
/// but defense-in-depth keeps a bad call from silently composing a draft with
/// an empty To: line.
pub fn compose_weekly_email(params: ComposeParams<'_>) -> EmailResult<ComposeResult> {
    let recipient = params.recipient.trim();
    if recipient.is_empty() {
        return Err(EmailError::NoRecipient);
    }

    let subject = if params.is_resend {
        format!("Update to weekly update - {}", params.week_label)
    } else {
        format!("Weekly update - {}", params.week_label)
    };
    let body = render_body(
        params.summary,
        params.week_label,
        params.manager_name.map(str::trim).filter(|s| !s.is_empty()),
    );

    let url = build_mailto_url(recipient, &subject, &body);
    if url.len() <= MAILTO_MAX_BYTES {
        return Ok(ComposeResult::Mailto(url));
    }

    let path = write_eml_file(recipient, &subject, &body, params.now)?;
    Ok(ComposeResult::Eml(path))
}

/// Build the plain-text email body. Headings use `##` so Gmail/Mail.app
/// render them as readable Markdown if the recipient's client supports
/// it; raw, it still reads cleanly. Empty sections are dropped so an
/// unfilled week sends a tight email instead of a skeleton of empty headings.
///
/// ## Markdown passthrough
///
/// The four summary fields are written verbatim into the body — whatever
/// text the user typed into the textareas (or, post-Phase 2.5, into the
/// CodeMirror markdown editor) is preserved character-for-character.
/// Bold, italics, lists, links — anything the user writes as Markdown
/// today shows up as Markdown source in the email. Recipients on Gmail
/// web get partial auto-rendering of common Markdown; everyone else
/// reads the source, which is the expected behavior for a plain-text
/// mailto: body. Switching to HTML email later is a body-format
/// change, not an API change.
fn render_body(
    summary: &WeeklySummary,
    week_label: &str,
    manager_name: Option<&str>,
) -> String {
    let mut out = String::with_capacity(512);

    // Greeting — personalized when we know the manager's name, neutral
    // otherwise. The trailing newline gap separates greeting from intro so
    // mail clients render it as a paragraph break, not a run-on line.
    match manager_name {
        Some(name) => out.push_str(&format!("Hello {name},\n\n")),
        None => out.push_str("Hello,\n\n"),
    }

    // One-line context for the recipient. The repo URL goes inline (rather
    // than as Markdown `[Captain's Log](...)`) because plain-text mailto:
    // bodies don't render Markdown link syntax — most mail clients
    // auto-linkify a bare URL, which is the universally-supported way to
    // make "Captain's Log" reachable.
    out.push_str("This is my update for the ");
    out.push_str(week_label);
    out.push_str(", sent through Captain's Log: ");
    out.push_str(CAPTAINS_LOG_REPO_URL);
    out.push_str("\n\n");

    push_section(&mut out, "Key accomplishments", &summary.key_accomplishments);
    push_section(
        &mut out,
        "Plans and priorities for next week",
        &summary.plans_and_priorities,
    );
    push_section(
        &mut out,
        "Challenges or roadblocks",
        &summary.challenges_or_roadblocks,
    );
    push_section(&mut out, "Anything else on your mind", &summary.anything_else);

    if !summary.labels.is_empty() {
        out.push_str("Labels: ");
        for (i, label) in summary.labels.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            out.push('#');
            out.push_str(label.trim_start_matches('#'));
        }
        out.push_str("\n");
    }

    out
}

fn push_section(out: &mut String, heading: &str, body: &str) {
    let body = body.trim();
    if body.is_empty() {
        return;
    }
    out.push_str("## ");
    out.push_str(heading);
    out.push_str("\n\n");
    out.push_str(body);
    out.push_str("\n\n");
}

/// Build a `mailto:` URL with percent-encoded subject and body.
///
/// urlencoding::encode is RFC 3986 form-encoding (spaces as `%20`, not `+`),
/// which is what mailto: actually wants — Mail.app and friends decode literal
/// `+` as a literal `+`, not a space, so the `+`-style encoding from
/// `application/x-www-form-urlencoded` would mangle plus signs into spaces in
/// the rendered body.
fn build_mailto_url(recipient: &str, subject: &str, body: &str) -> String {
    let subject_enc = percent_encode(subject);
    let body_enc = percent_encode(body);
    // The recipient is also percent-encoded — addresses with `+`-aliases
    // (`user+tag@example.com`) would otherwise lose the `+` to mailto's
    // ambiguity. macOS LaunchServices is forgiving here but encoding makes
    // the URL portable to other handlers.
    let to_enc = percent_encode(recipient);
    format!(
        "mailto:{to}?subject={subject}&body={body}",
        to = to_enc,
        subject = subject_enc,
        body = body_enc
    )
}

/// Wrap `subject` as an RFC 2047 "encoded-word" if it contains any non-ASCII
/// bytes; otherwise return it unchanged. RFC 5322 strictly requires headers
/// to be ASCII; lenient mail clients accept UTF-8 anyway, but encoding keeps
/// strict parsers (some MTA pipelines, forwarders) happy. The base64 variant
/// is preferred for arbitrary UTF-8 over Q-encoding because it doesn't need
/// per-character escape decisions.
fn encode_header_subject(subject: &str) -> String {
    if subject.is_ascii() {
        return subject.to_string();
    }
    use base64::Engine;
    let encoded = base64::engine::general_purpose::STANDARD.encode(subject.as_bytes());
    format!("=?UTF-8?B?{encoded}?=")
}

/// Write an RFC 822 .eml file the user's default mail client can open as an
/// editable draft. Returns the file path so the caller can hand it to
/// `opener::open_path`. The caller is responsible for cleanup — we don't
/// know when LaunchServices is done reading the file. A startup janitor
/// (`prune_old_eml_files`) sweeps anything older than 24h.
fn write_eml_file(
    recipient: &str,
    subject: &str,
    body: &str,
    now: DateTime<FixedOffset>,
) -> std::io::Result<PathBuf> {
    let dir = std::env::temp_dir().join(EML_TEMP_SUBDIR);
    std::fs::create_dir_all(&dir)?;

    // Filename includes a timestamp + an atomic monotonic counter so
    // successive sends don't collide. The counter guards specifically
    // against parallel callers (test threads, racing UI clicks) that
    // would otherwise share the same millisecond stamp and clobber each
    // other's files. The janitor's age-based prune still works because
    // file mtime is unaffected.
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let stamp = now.format("%Y%m%dT%H%M%S%.3f");
    let file_path = dir.join(format!("weekly-{stamp}-{seq:04}.eml"));

    // The subject often contains an en-dash (U+2013) from the week label —
    // strict RFC 5322 forbids non-ASCII in headers, so wrap the subject as
    // an RFC 2047 encoded-word when needed. Body is declared UTF-8 in the
    // Content-Type header so it can hold arbitrary unicode unchanged.
    let header_subject = encode_header_subject(subject);
    let date_header = now.to_rfc2822();
    let mut content = String::with_capacity(body.len() + 256);
    content.push_str(&format!("To: {recipient}\r\n"));
    content.push_str(&format!("Subject: {header_subject}\r\n"));
    content.push_str(&format!("Date: {date_header}\r\n"));
    content.push_str("Content-Type: text/plain; charset=utf-8\r\n");
    content.push_str("Content-Transfer-Encoding: 8bit\r\n");
    content.push_str("MIME-Version: 1.0\r\n");
    content.push_str("\r\n");
    // .eml bodies traditionally use CRLF line endings; converting here keeps
    // strict parsers happy. Mail.app tolerates LF too in practice.
    for line in body.split('\n') {
        content.push_str(line.trim_end_matches('\r'));
        content.push_str("\r\n");
    }

    std::fs::write(&file_path, content)?;
    Ok(file_path)
}

/// Delete .eml files in `<tempdir>/captainslog/` older than 24 hours.
/// Called fire-and-forget from `lib::run`'s setup hook on app launch. Errors
/// are logged but never propagate — a janitor that fails should never block
/// app start.
pub fn prune_old_eml_files() {
    let dir = std::env::temp_dir().join(EML_TEMP_SUBDIR);
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
        Err(e) => {
            eprintln!("[email-janitor] read_dir({}) failed: {e}", dir.display());
            return;
        }
    };
    let now = std::time::SystemTime::now();
    let cutoff = std::time::Duration::from_secs(24 * 60 * 60);
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("eml") {
            continue;
        }
        if let Ok(meta) = entry.metadata() {
            if let Ok(modified) = meta.modified() {
                if let Ok(age) = now.duration_since(modified) {
                    if age > cutoff {
                        if let Err(e) = std::fs::remove_file(&path) {
                            eprintln!(
                                "[email-janitor] failed to prune {}: {e}",
                                path.display()
                            );
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn fixed_now() -> DateTime<FixedOffset> {
        FixedOffset::west_opt(4 * 3600)
            .unwrap()
            .with_ymd_and_hms(2026, 6, 24, 16, 12, 33)
            .unwrap()
    }

    fn summary(key: &str, plans: &str, challenges: &str, other: &str, labels: &[&str]) -> WeeklySummary {
        WeeklySummary {
            key_accomplishments: key.to_string(),
            plans_and_priorities: plans.to_string(),
            challenges_or_roadblocks: challenges.to_string(),
            anything_else: other.to_string(),
            labels: labels.iter().map(|s| s.to_string()).collect(),
            last_updated: None,
        }
    }

    /// Builder shorthand for ComposeParams. Most tests don't care about
    /// manager_name / is_resend, so default both to None / false and let
    /// individual tests override.
    fn params<'a>(
        summary: &'a WeeklySummary,
        week_label: &'a str,
        recipient: &'a str,
    ) -> ComposeParams<'a> {
        ComposeParams {
            summary,
            week_label,
            recipient,
            manager_name: None,
            is_resend: false,
            now: fixed_now(),
        }
    }

    #[test]
    fn empty_recipient_is_rejected() {
        let s = summary("a", "b", "c", "d", &[]);
        let r = compose_weekly_email(params(&s, "Week of X", ""));
        assert!(matches!(r, Err(EmailError::NoRecipient)));
    }

    #[test]
    fn whitespace_recipient_is_rejected() {
        let s = summary("a", "b", "c", "d", &[]);
        let r = compose_weekly_email(params(&s, "Week of X", "   "));
        assert!(matches!(r, Err(EmailError::NoRecipient)));
    }

    #[test]
    fn short_summary_returns_mailto() {
        let s = summary("Shipped X.", "Y.", "", "", &["mage"]);
        let r = compose_weekly_email(params(
            &s,
            "week of Jun 22 - Jun 28, 2026",
            "boss@example.com",
        ))
        .unwrap();
        match r {
            ComposeResult::Mailto(url) => {
                assert!(url.starts_with("mailto:boss%40example.com?"));
                assert!(url.contains("subject=Weekly%20update"));
                assert!(url.contains("Shipped%20X."));
            }
            ComposeResult::Eml(_) => panic!("expected mailto, got eml"),
        }
    }

    #[test]
    fn very_long_summary_falls_back_to_eml() {
        // Build a body that will definitely exceed MAILTO_MAX_BYTES after
        // percent-encoding. ~3000 chars of letters encodes 1:1, so this
        // alone is over the threshold.
        let big = "abcdefghij ".repeat(300);
        let s = summary(&big, "", "", "", &[]);
        let r = compose_weekly_email(params(
            &s,
            "week of Jun 22 - Jun 28, 2026",
            "boss@example.com",
        ))
        .unwrap();
        let path = match r {
            ComposeResult::Eml(p) => p,
            ComposeResult::Mailto(_) => panic!("expected eml fallback"),
        };
        assert!(path.exists(), "eml file should be on disk");
        assert!(
            path.to_string_lossy().contains(EML_TEMP_SUBDIR),
            "eml file should land in captainslog/ scratch dir, got: {}",
            path.display()
        );
        let written = std::fs::read_to_string(&path).unwrap();
        assert!(written.starts_with("To: boss@example.com\r\n"));
        assert!(written.contains("Subject: Weekly update - week of Jun 22 - Jun 28, 2026\r\n"));
        assert!(written.contains("Content-Type: text/plain; charset=utf-8"));
        assert!(written.contains(&big[..50]));
        // Cleanup so we don't leave megabytes of test fixtures in $TMPDIR.
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn body_omits_empty_sections() {
        let s = summary("got this done", "", "", "", &[]);
        let r = compose_weekly_email(params(&s, "Week of X", "boss@example.com")).unwrap();
        let url = match r {
            ComposeResult::Mailto(u) => u,
            ComposeResult::Eml(_) => panic!("expected mailto"),
        };
        // Key accomplishments heading present:
        assert!(url.contains("Key%20accomplishments"));
        // Empty sections NOT present:
        assert!(!url.contains("Plans%20and%20priorities"));
        assert!(!url.contains("Challenges%20or%20roadblocks"));
        assert!(!url.contains("Anything%20else"));
    }

    #[test]
    fn body_includes_labels_when_present() {
        let s = summary("k", "p", "", "", &["mage", "qa"]);
        let r = compose_weekly_email(params(&s, "Week of X", "boss@example.com")).unwrap();
        let url = match r {
            ComposeResult::Mailto(u) => u,
            ComposeResult::Eml(_) => panic!("expected mailto"),
        };
        // "Labels: #mage, #qa" — percent-encoded
        assert!(url.contains("Labels%3A"));
        assert!(url.contains("%23mage"));
        assert!(url.contains("%23qa"));
    }

    #[test]
    fn body_omits_labels_line_when_no_labels() {
        let s = summary("k", "p", "", "", &[]);
        let r = compose_weekly_email(params(&s, "Week of X", "boss@example.com")).unwrap();
        let url = match r {
            ComposeResult::Mailto(u) => u,
            ComposeResult::Eml(_) => panic!("expected mailto"),
        };
        assert!(!url.contains("Labels%3A"));
    }

    #[test]
    fn ampersand_in_body_is_encoded() {
        // & must be encoded inside the mailto query, or the mail client will
        // mis-parse the URL into multiple parameters.
        let s = summary("Q & A session", "", "", "", &[]);
        let r = compose_weekly_email(params(&s, "Week of X", "boss@example.com")).unwrap();
        let url = match r {
            ComposeResult::Mailto(u) => u,
            ComposeResult::Eml(_) => panic!("expected mailto"),
        };
        assert!(url.contains("Q%20%26%20A"));
        // Only the two intentional `&` separators (between to/subject and
        // subject/body) should remain in the URL.
        assert_eq!(url.matches('&').count(), 1);
    }

    #[test]
    fn space_uses_percent_20_not_plus() {
        // application/x-www-form-urlencoded encodes spaces as `+`, but mailto:
        // handlers don't decode `+` as space — they leave it literal. Using
        // %20 keeps spaces working everywhere.
        let s = summary("hello world", "", "", "", &[]);
        let r = compose_weekly_email(params(&s, "Week of X", "boss@example.com")).unwrap();
        let url = match r {
            ComposeResult::Mailto(u) => u,
            ComposeResult::Eml(_) => panic!("expected mailto"),
        };
        assert!(url.contains("hello%20world"));
        assert!(!url.contains("hello+world"));
    }

    #[test]
    fn plus_aliased_recipient_is_encoded() {
        let s = summary("k", "", "", "", &[]);
        let r = compose_weekly_email(params(
            &s,
            "Week of X",
            "chris.carpenter+work@prodigygame.com",
        ))
        .unwrap();
        let url = match r {
            ComposeResult::Mailto(u) => u,
            ComposeResult::Eml(_) => panic!("expected mailto"),
        };
        // The `+` must survive — encoded as %2B so mail clients don't decode
        // it back to a space.
        assert!(url.contains("chris.carpenter%2Bwork%40prodigygame.com"));
    }

    // ---- new in this revision: greeting / intro / resend subject ----

    #[test]
    fn greeting_uses_manager_name_when_set() {
        let s = summary("k", "", "", "", &[]);
        let mut p = params(&s, "Week of X", "boss@example.com");
        p.manager_name = Some("Pat");
        let url = match compose_weekly_email(p).unwrap() {
            ComposeResult::Mailto(u) => u,
            ComposeResult::Eml(_) => panic!("expected mailto"),
        };
        assert!(url.contains("Hello%20Pat%2C"), "expected `Hello Pat,` in body, url={url}");
    }

    #[test]
    fn greeting_falls_back_when_manager_name_missing() {
        let s = summary("k", "", "", "", &[]);
        let url = match compose_weekly_email(params(&s, "Week of X", "boss@example.com")).unwrap()
        {
            ComposeResult::Mailto(u) => u,
            ComposeResult::Eml(_) => panic!("expected mailto"),
        };
        // "Hello,\n\n" — note: must NOT have a name token between Hello and ,
        assert!(url.contains("Hello%2C"));
        assert!(!url.contains("Hello%20%2C"), "should not have trailing space before comma");
    }

    #[test]
    fn greeting_treats_whitespace_only_name_as_missing() {
        let s = summary("k", "", "", "", &[]);
        let mut p = params(&s, "Week of X", "boss@example.com");
        p.manager_name = Some("   ");
        let url = match compose_weekly_email(p).unwrap() {
            ComposeResult::Mailto(u) => u,
            ComposeResult::Eml(_) => panic!("expected mailto"),
        };
        assert!(url.contains("Hello%2C"));
        assert!(!url.contains("Hello%20%20%20"));
    }

    #[test]
    fn body_includes_week_label_and_repo_url_in_intro() {
        let s = summary("k", "", "", "", &[]);
        let url = match compose_weekly_email(params(
            &s,
            "week of Jun 22 - Jun 28, 2026",
            "boss@example.com",
        ))
        .unwrap()
        {
            ComposeResult::Mailto(u) => u,
            ComposeResult::Eml(_) => panic!("expected mailto"),
        };
        // "This is my update for the {label}, sent through Captain's Log: {URL}"
        assert!(url.contains("This%20is%20my%20update%20for%20the"));
        assert!(url.contains("sent%20through%20Captain%27s%20Log"));
        // Repo URL — ':' is %3A, '/' is %2F.
        assert!(
            url.contains("https%3A%2F%2Fgithub.com%2FChrisACarpenter%2Fcaptains-log"),
            "expected encoded repo URL in body, url={url}"
        );
    }

    #[test]
    fn resend_changes_the_subject_line() {
        let s = summary("k", "", "", "", &[]);
        let mut p = params(&s, "week of Jun 22 - Jun 28, 2026", "boss@example.com");
        p.is_resend = true;
        let url = match compose_weekly_email(p).unwrap() {
            ComposeResult::Mailto(u) => u,
            ComposeResult::Eml(_) => panic!("expected mailto"),
        };
        assert!(
            url.contains("subject=Update%20to%20weekly%20update%20-%20week%20of%20Jun%2022"),
            "expected resend subject, url={url}"
        );
    }

    #[test]
    fn first_send_uses_plain_subject() {
        let s = summary("k", "", "", "", &[]);
        let url = match compose_weekly_email(params(
            &s,
            "week of Jun 22 - Jun 28, 2026",
            "boss@example.com",
        ))
        .unwrap()
        {
            ComposeResult::Mailto(u) => u,
            ComposeResult::Eml(_) => panic!("expected mailto"),
        };
        assert!(url.contains("subject=Weekly%20update%20-%20week%20of%20Jun%2022"));
        assert!(!url.contains("Update%20to%20weekly%20update"));
    }

    #[test]
    fn ascii_subject_is_not_encoded_in_eml() {
        // ASCII-only subjects should pass through untouched (no encoded-word
        // wrapping) — keeps .eml files human-readable when nothing requires
        // encoding.
        assert_eq!(
            encode_header_subject("Weekly update - week of Jun 22, 2026"),
            "Weekly update - week of Jun 22, 2026"
        );
    }

    #[test]
    fn non_ascii_subject_is_rfc2047_encoded() {
        // Production subjects contain an en-dash (U+2013) from the week
        // label. RFC 5322 forbids non-ASCII in headers; we wrap as
        // =?UTF-8?B?<base64>?=.
        let encoded = encode_header_subject("Weekly update - week of June 22 \u{2013} June 28, 2026");
        assert!(encoded.starts_with("=?UTF-8?B?"));
        assert!(encoded.ends_with("?="));
        // Round-trip: the base64 between the markers must decode to the
        // original bytes.
        use base64::Engine;
        let payload = &encoded["=?UTF-8?B?".len()..encoded.len() - "?=".len()];
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(payload)
            .expect("base64 must decode");
        assert_eq!(
            decoded,
            "Weekly update - week of June 22 \u{2013} June 28, 2026".as_bytes()
        );
    }

    #[test]
    fn eml_subject_is_encoded_when_week_label_contains_en_dash() {
        // End-to-end: a long body forces the .eml branch, and the production
        // week label contains an en-dash. The Subject header in the file
        // must be RFC 2047 encoded, not raw UTF-8.
        let big = "x".repeat(3000);
        let s = summary(&big, "", "", "", &[]);
        let r = compose_weekly_email(params(
            &s,
            "week of June 22 \u{2013} June 28, 2026",
            "boss@example.com",
        ))
        .unwrap();
        let path = match r {
            ComposeResult::Eml(p) => p,
            ComposeResult::Mailto(_) => panic!("expected eml fallback"),
        };
        let written = std::fs::read_to_string(&path).unwrap();
        assert!(
            written.contains("Subject: =?UTF-8?B?"),
            "expected RFC 2047 encoded subject, got: {}",
            written.lines().find(|l| l.starts_with("Subject:")).unwrap_or("(no Subject line)")
        );
        // The raw en-dash byte sequence (\xe2\x80\x93 in UTF-8) must NOT
        // appear in the Subject header line itself.
        let subject_line = written
            .lines()
            .find(|l| l.starts_with("Subject:"))
            .unwrap();
        assert!(!subject_line.contains('\u{2013}'));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn markdown_in_summary_passes_through_verbatim() {
        // When the CodeMirror markdown editor lands, users will type things
        // like **bold** in the textareas. The body must preserve that
        // character-for-character so recipients see formatting (whatever
        // their mail client renders).
        let s = summary("Shipped **the thing**", "", "", "", &[]);
        let url = match compose_weekly_email(params(&s, "Week of X", "boss@example.com")).unwrap()
        {
            ComposeResult::Mailto(u) => u,
            ComposeResult::Eml(_) => panic!("expected mailto"),
        };
        // urlencoding treats `*` as a reserved character and percent-encodes
        // it as `%2A`. The point of this test is that the markdown survives
        // round-trip: after the mail client decodes, the body contains
        // literal `**the thing**` again.
        let body_start = url
            .find("body=")
            .expect("URL must have a body param");
        let body_encoded = &url[body_start + "body=".len()..];
        let body = urlencoding::decode(body_encoded).expect("body must decode");
        assert!(
            body.contains("Shipped **the thing**"),
            "decoded body must contain literal markdown, got: {body}"
        );
    }
}
