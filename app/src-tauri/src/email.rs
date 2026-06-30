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
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use urlencoding::encode as percent_encode;

use crate::email_html;
use crate::notes::WeeklySummary;
use crate::settings::{MailBodyFormat, MailSendMode, OutlookFlavor};

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
pub(crate) const CAPTAINS_LOG_REPO_URL: &str =
    "https://github.com/ChrisACarpenter/captains-log";

#[derive(Debug, Error)]
pub enum EmailError {
    #[error("i/o error writing .eml: {0}")]
    Io(#[from] std::io::Error),
}

pub type EmailResult<T> = Result<T, EmailError>;

/// What the frontend should do next. Each variant maps to a different
/// hand-off mechanism — `opener::open_url` for `Mailto` / `WebUrl`,
/// `opener::open_path` for `Eml`, and an `osascript` spawn for
/// `AppleScript` — so the enum doubles as a dispatch signal for the
/// frontend.
///
/// Variants:
///   - `Mailto`        — RFC 6068 `mailto:` URL (legacy default-mail-client path).
///   - `Eml`           — RFC 822 `.eml` file path (HTML / multipart fallback).
///   - `WebUrl`        — Gmail or Outlook compose URL opened in the browser.
///                       `truncation_warning` is set for Gmail when the
///                       encoded URL exceeds Gmail's silent ~2 KB cap.
///   - `AppleScript`   — Source for an `osascript -` invocation that drives
///                       Mac Mail.app via Apple Events.
///
/// `kind` is the serde discriminator; the remaining fields are flattened
/// alongside it, so each variant's JSON shape is `{kind: "...", <fields>}`
/// — what the frontend type union mirrors.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ComposeResult {
    Mailto {
        value: String,
    },
    Eml {
        value: PathBuf,
    },
    #[serde(rename_all = "camelCase")]
    WebUrl {
        url: String,
        truncation_warning: bool,
    },
    AppleScript {
        script: String,
    },
}

/// Which body format to deliver. Text keeps the legacy mailto/single-part .eml
/// path. Html forces a multipart/alternative .eml (mailto can't carry HTML)
/// and adds a `text/html` part alongside the existing plaintext.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BodyFormat {
    #[default]
    Text,
    Html,
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
    /// Defaults to `Text` so existing callers keep their behavior; the
    /// /summary + /journal Send buttons pass `Html` to force the styled
    /// multipart/alternative path.
    pub format: BodyFormat,
    /// Phase 2.9b: when `Some`, takes precedence over `format` and routes
    /// through one of the mode-specific builders (Gmail web URL, Outlook
    /// web URL, AppleScript for Mac Mail). Slice 4 will switch the
    /// `compose_weekly_email` Tauri command to populate this. Slice 3
    /// leaves it `None` everywhere, so existing callers continue to hit
    /// the legacy `BodyFormat::Text`/`Html` branches above.
    pub send: Option<MailSend<'a>>,
}

/// Mode-specific options for the Phase 2.9b dispatch. Carries the picked
/// `MailSendMode` plus everything the per-mode builder needs that isn't
/// already in `ComposeParams` — body format (clean vs raw markdown), the
/// sender's own email (for Gmail's `/mail/u/{addr}` routing + Mac Mail's
/// `sender` property), the Outlook host flavor (Business vs Personal),
/// and whether Mac Mail should drive an HTML message.
#[derive(Debug, Clone, Copy)]
pub struct MailSend<'a> {
    pub mode: MailSendMode,
    pub body_format: MailBodyFormat,
    /// The user's own email. Drives Gmail's `/mail/u/<addr>` slot (and
    /// becomes Mac Mail's outgoing `sender`). `None` falls back to
    /// `/mail/u/0` for Gmail, and Mac Mail uses whatever the default
    /// account is.
    pub user_email: Option<&'a str>,
    pub outlook_flavor: OutlookFlavor,
    /// Native Mac Mail only — when true, route through the existing
    /// multipart/alternative .eml HTML path instead of the AppleScript
    /// plaintext path.
    pub native_html: bool,
    /// "Compose + paste" mode. When true, the URL/AppleScript embeds an
    /// EMPTY body and the frontend (which owns the system clipboard) is
    /// expected to have already called `writeHtml(html, text)` so the
    /// user can press Cmd+V in the compose body to paste the rich
    /// version. Bypasses plaintext_from_markdown entirely on the
    /// backend; sets truncation_warning to false (an empty body can't
    /// overflow). The Native Mac `.eml` HTML path takes precedence —
    /// when `native_html` is true, this flag is ignored (the multipart
    /// .eml already carries a rich body without needing a paste step).
    pub body_in_clipboard: bool,
}

/// The rendered bodies fed into [`write_eml_file`]. `text` is always present
/// (used as the single-part body when `html` is `None`, or the text/plain
/// alternative when `html` is `Some`). `html` is `Some` only for HTML mode.
struct BodyParts {
    text: String,
    html: Option<String>,
}

/// Build the email and decide which delivery channel to use. The decision is
/// purely length-based — if the encoded mailto URL would exceed
/// [`MAILTO_MAX_BYTES`], we fall back to .eml. Otherwise we return the
/// mailto URL directly.
///
/// `recipient` is trimmed before use. An empty recipient is allowed: the
/// mailto URL is built with no `To:`, so the user's mail app opens a
/// draft with a blank recipient line ready to be filled in. The
/// greeting also degrades — name → "Hello {name}," / no name →
/// "Hello,".
pub fn compose_weekly_email(params: ComposeParams<'_>) -> EmailResult<ComposeResult> {
    let recipient = params.recipient.trim();
    let manager_name = params.manager_name.map(str::trim).filter(|s| !s.is_empty());

    let subject = if params.is_resend {
        format!("Update to weekly update - {}", params.week_label)
    } else {
        format!("Weekly update - {}", params.week_label)
    };
    let text = render_body(params.summary, params.week_label, manager_name);

    // Phase 2.9b dispatch — when `send` is `Some`, route through the
    // mode-specific builder (Gmail web URL, Outlook web URL, AppleScript
    // for Mac Mail). The legacy `format`-based path below remains so
    // Slice 3 doesn't change runtime behavior for any existing call site;
    // Slice 4 will switch the command to populate `send`.
    if let Some(send) = params.send {
        // Mode-specific plaintext: respects `mail_body_format`. The
        // greeting + intro + section structure are identical to
        // `render_body`; only the inline formatting markers (## headers,
        // **bold**, etc.) get conditionally stripped. Skipped entirely
        // when `body_in_clipboard` is on (and the path doesn't otherwise
        // need the plaintext) — the frontend has the rich HTML on the
        // clipboard and the user will Cmd+V into the compose body, so
        // we open the draft empty rather than pre-fill plaintext.
        let plaintext = match send.body_format {
            MailBodyFormat::MarkdownSource => text.clone(),
            MailBodyFormat::CleanText => plaintext_from_markdown(&text, send.body_format),
        };
        // Native Mac HTML mode is an independent peer override: when
        // `native_html` is set, we route through the .eml multipart
        // writer regardless of `body_in_clipboard` (the .eml already
        // carries a rich body — no paste step needed).
        let use_clipboard_paste =
            send.body_in_clipboard && !(send.mode == MailSendMode::NativeMail && send.native_html);
        let body_arg: &str = if use_clipboard_paste { "" } else { &plaintext };
        return match send.mode {
            MailSendMode::Gmail => Ok(build_gmail_url(
                send.user_email,
                recipient,
                &subject,
                body_arg,
            )),
            MailSendMode::Outlook => Ok(build_outlook_url(
                send.outlook_flavor,
                recipient,
                &subject,
                body_arg,
            )),
            MailSendMode::NativeMail => {
                if send.native_html {
                    // Mac Mail HTML mode reuses the multipart/alternative
                    // .eml writer — Mail.app opens .eml as an editable
                    // draft, which is the only realistic path to a styled
                    // outgoing message (AppleScript's `content` property
                    // is plaintext-only).
                    let html = email_html::render_body_html(
                        params.summary,
                        params.week_label,
                        manager_name,
                    );
                    let parts = BodyParts {
                        text,
                        html: Some(html),
                    };
                    let path = write_eml_file(recipient, &subject, &parts, params.now)?;
                    Ok(ComposeResult::Eml { value: path })
                } else {
                    Ok(build_applescript(
                        send.user_email,
                        recipient,
                        &subject,
                        body_arg,
                    ))
                }
            }
        };
    }

    match params.format {
        BodyFormat::Text => {
            let url = build_mailto_url(recipient, &subject, &text);
            if url.len() <= MAILTO_MAX_BYTES {
                return Ok(ComposeResult::Mailto { value: url });
            }
            let parts = BodyParts { text, html: None };
            let path = write_eml_file(recipient, &subject, &parts, params.now)?;
            Ok(ComposeResult::Eml { value: path })
        }
        BodyFormat::Html => {
            // HTML mode skips the mailto branch entirely — mailto: cannot
            // carry an HTML body (RFC 6068) — and always writes a
            // multipart/alternative .eml.
            let html = email_html::render_body_html(params.summary, params.week_label, manager_name);
            let parts = BodyParts {
                text,
                html: Some(html),
            };
            let path = write_eml_file(recipient, &subject, &parts, params.now)?;
            Ok(ComposeResult::Eml { value: path })
        }
    }
}

// ---------------------------------------------------------------------------
// Phase 2.9b: per-mode URL / script builders
// ---------------------------------------------------------------------------

/// Hard ceiling on the encoded Gmail compose URL byte length. Gmail's
/// `view=cm` deeplink silently truncates above ~2 KB; we surface a warning
/// at exactly 2000 so the user can decide whether to send anyway (the
/// modal renders a "Send anyway" button) or shorten the body. Outlook's
/// equivalent cap is ~8 KB and unlikely to be hit in practice; only Gmail
/// gets the explicit check.
const GMAIL_URL_WARN_BYTES: usize = 2000;

/// Build a Gmail `view=cm` compose URL with To/Subject/Body pre-filled.
///
/// The `/mail/u/{ACCOUNT}` slot pins routing on multi-account Gmail. When
/// `user_email` is `Some`, we pass the address there — Gmail recognizes
/// the per-account URL and opens compose in the matching account. When
/// `None`, we fall back to `/mail/u/0`, which is the first-signed-in
/// account (works for single-account users, ambiguous for multi-account
/// — hence the strong preference for setting `user_email` in settings).
///
/// All query values are encoded with the `NON_ALPHANUMERIC` set (every
/// non-`[A-Za-z0-9]` byte becomes `%xx`) — Gmail is permissive but the
/// extra-cautious encoding works in every browser. Note Gmail's subject
/// param is `su`, not `subject` (legacy naming, predates `subject` being
/// standardized).
fn build_gmail_url(
    user_email: Option<&str>,
    recipient: &str,
    subject: &str,
    body: &str,
) -> ComposeResult {
    let account_slot = user_email
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|email| utf8_percent_encode(email, NON_ALPHANUMERIC).to_string())
        .unwrap_or_else(|| "0".to_string());
    let to_enc = utf8_percent_encode(recipient, NON_ALPHANUMERIC).to_string();
    let su_enc = utf8_percent_encode(subject, NON_ALPHANUMERIC).to_string();
    let body_enc = utf8_percent_encode(body, NON_ALPHANUMERIC).to_string();
    let url = format!(
        "https://mail.google.com/mail/u/{account}/?view=cm&tf=cm&to={to}&su={su}&body={body}",
        account = account_slot,
        to = to_enc,
        su = su_enc,
        body = body_enc,
    );
    let truncation_warning = url.len() > GMAIL_URL_WARN_BYTES;
    ComposeResult::WebUrl {
        url,
        truncation_warning,
    }
}

/// Build an Outlook web compose URL for either Business (Microsoft 365)
/// or Personal (outlook.com).
///
/// Distinctions vs Gmail:
///   - Subject param is `subject` (not `su`).
///   - Two host paths; the Business one routes through `outlook.office.com`.
///   - No truncation check — Outlook's cap is ~8 KB, well above what a
///     weekly update will ever produce.
fn build_outlook_url(
    flavor: OutlookFlavor,
    recipient: &str,
    subject: &str,
    body: &str,
) -> ComposeResult {
    let host = match flavor {
        OutlookFlavor::Business => "https://outlook.office.com/mail/deeplink/compose",
        OutlookFlavor::Personal => "https://outlook.live.com/mail/0/deeplink/compose",
    };
    let to_enc = utf8_percent_encode(recipient, NON_ALPHANUMERIC).to_string();
    let subject_enc = utf8_percent_encode(subject, NON_ALPHANUMERIC).to_string();
    let body_enc = utf8_percent_encode(body, NON_ALPHANUMERIC).to_string();
    let url = format!(
        "{host}?to={to}&subject={subject}&body={body}",
        host = host,
        to = to_enc,
        subject = subject_enc,
        body = body_enc,
    );
    ComposeResult::WebUrl {
        url,
        truncation_warning: false,
    }
}

/// Escape a string for embedding inside a double-quoted AppleScript
/// literal. Two characters need escaping:
///   - `\` → `\\`  (backslash starts AppleScript's own escape sequences)
///   - `"` → `\"`  (would close the literal early)
///
/// AppleScript handles bare newlines fine inside double-quoted strings,
/// so we don't translate `\n`. The osascript stdin pipeline preserves
/// them as-is.
fn applescript_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            c => out.push(c),
        }
    }
    out
}

/// Build the AppleScript source that drives Mail.app via Apple Events.
///
/// The frontend pipes this into `osascript -` via stdin, which sidesteps
/// argv length caps (osascript's argv path truncates around 256 KB on
/// modern macOS; stdin is unbounded). Apple Events permission denial
/// surfaces as `errAEEventNotPermitted` (-1743) on stderr — the frontend
/// matches on that to show the "Open Automation Settings" link.
///
/// Layout:
///   tell application "Mail"
///       set newMessage to make new outgoing message with properties {…}
///       tell newMessage
///           make new to recipient with properties {address:"…"}
///           [set sender to "…" — only when user_email is set]
///       end tell
///       activate
///   end tell
fn build_applescript(
    user_email: Option<&str>,
    recipient: &str,
    subject: &str,
    body: &str,
) -> ComposeResult {
    let subject_e = applescript_escape(subject);
    let body_e = applescript_escape(body);
    let recipient_e = applescript_escape(recipient);

    // The sender line is the multi-account routing point. Omitted entirely
    // when the user hasn't set their email — Mail.app then uses whatever
    // the default outgoing account is.
    let sender_line = user_email
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|email| {
            let e = applescript_escape(email);
            format!("        set sender to \"{e}\"\n")
        })
        .unwrap_or_default();

    let script = format!(
        "tell application \"Mail\"\n\
\tset newMessage to make new outgoing message with properties {{subject:\"{subject}\", content:\"{body}\", visible:true}}\n\
\ttell newMessage\n\
\t\tmake new to recipient with properties {{address:\"{recipient}\"}}\n\
{sender_line}\
\tend tell\n\
\tactivate\n\
end tell\n",
        subject = subject_e,
        body = body_e,
        recipient = recipient_e,
        sender_line = sender_line,
    );
    ComposeResult::AppleScript { script }
}

/// Render the email body as plaintext, with markdown markers handled per
/// `format`.
///
/// - `MailBodyFormat::MarkdownSource` — pass `md` through unchanged. Used
///   when the user wants Gmail/Outlook to receive the raw `**bold**` /
///   `## headings` markup (some recipients have client-side
///   auto-rendering, and the source is human-readable as-is).
///
/// - `MailBodyFormat::CleanText` — walk the pulldown-cmark event stream
///   and emit a clean plaintext rendering:
///     * H1/H2/H3 headings → uppercase line + blank line.
///     * bullets → `- item` (one per line).
///     * **bold** / *italic* markers stripped (text passes through).
///     * `[label](url)` → `label (url)`.
///     * Fenced code blocks → emitted verbatim between blank lines.
///
/// The output isn't a strict markdown-to-plaintext converter (it
/// intentionally drops blockquote prefixes, ordered-list numbering, etc.
/// — none of which the four weekly summary fields use), it's a
/// pragmatic "what a manager would want to read" reduction.
pub fn plaintext_from_markdown(md: &str, format: MailBodyFormat) -> String {
    if matches!(format, MailBodyFormat::MarkdownSource) {
        return md.to_string();
    }

    let parser = Parser::new(md);
    let mut out = String::with_capacity(md.len());
    // State for emitting inline events under nested containers.
    let mut heading_level: Option<HeadingLevel> = None;
    let mut heading_buf = String::new();
    let mut in_item = false;
    let mut in_link: Option<String> = None;
    let mut link_label = String::new();
    let mut in_code_block = false;

    for ev in parser {
        match ev {
            Event::Start(Tag::Heading { level, .. }) => {
                heading_level = Some(level);
                heading_buf.clear();
            }
            Event::End(TagEnd::Heading(_)) => {
                let line = heading_buf.trim().to_uppercase();
                if !line.is_empty() {
                    // Trim trailing blank lines before emitting the heading
                    // so we don't double-pad when sections butt against
                    // each other.
                    while out.ends_with("\n\n\n") {
                        out.pop();
                    }
                    if !out.is_empty() && !out.ends_with("\n\n") {
                        if out.ends_with('\n') {
                            out.push('\n');
                        } else {
                            out.push_str("\n\n");
                        }
                    }
                    out.push_str(&line);
                    out.push_str("\n\n");
                }
                heading_level = None;
                heading_buf.clear();
            }
            Event::Start(Tag::List(_)) => {}
            Event::End(TagEnd::List(_)) => {
                if !out.ends_with("\n\n") {
                    out.push('\n');
                }
            }
            Event::Start(Tag::Item) => {
                in_item = true;
                out.push_str("- ");
            }
            Event::End(TagEnd::Item) => {
                in_item = false;
                if !out.ends_with('\n') {
                    out.push('\n');
                }
            }
            Event::Start(Tag::Paragraph) => {
                // No-op — text events emit content; we just need the
                // trailing newline on End.
            }
            Event::End(TagEnd::Paragraph) => {
                if !in_item {
                    out.push_str("\n\n");
                }
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                in_link = Some(dest_url.to_string());
                link_label.clear();
            }
            Event::End(TagEnd::Link) => {
                if let Some(url) = in_link.take() {
                    if heading_level.is_some() {
                        heading_buf.push_str(&format!("{label} ({url})", label = link_label));
                    } else {
                        out.push_str(&format!("{label} ({url})", label = link_label));
                    }
                }
                link_label.clear();
            }
            Event::Start(Tag::CodeBlock(_)) => {
                in_code_block = true;
                if !out.ends_with("\n\n") {
                    out.push_str("\n\n");
                }
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;
                if !out.ends_with("\n\n") {
                    out.push('\n');
                }
            }
            // Inline emphasis (bold / italic): we emit nothing on the
            // boundary events — the inner Text events carry the actual
            // characters, and stripping the markers is exactly what
            // "clean text" means.
            Event::Start(Tag::Emphasis)
            | Event::End(TagEnd::Emphasis)
            | Event::Start(Tag::Strong)
            | Event::End(TagEnd::Strong) => {}
            Event::Text(t) | Event::Code(t) => {
                let s = t.as_ref();
                if heading_level.is_some() {
                    heading_buf.push_str(s);
                } else if in_link.is_some() {
                    link_label.push_str(s);
                } else if in_code_block {
                    out.push_str(s);
                } else {
                    out.push_str(s);
                }
            }
            Event::SoftBreak => {
                if heading_level.is_some() {
                    heading_buf.push(' ');
                } else {
                    out.push('\n');
                }
            }
            Event::HardBreak => {
                if heading_level.is_some() {
                    heading_buf.push(' ');
                } else {
                    out.push('\n');
                }
            }
            _ => {}
        }
    }

    // Collapse any trailing blank-line runs down to exactly one trailing
    // newline so the output ends cleanly.
    while out.ends_with("\n\n\n") {
        out.pop();
    }
    out
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
pub fn render_body(
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
///
/// Empty `recipient` produces `mailto:?subject=…&body=…` — RFC 6068
/// permits a missing `to` part, and Mail.app / Mimestream / Spark all
/// open such a URL as a draft with a blank To: field ready for the user
/// to fill in.
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
    parts: &BodyParts,
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

    let unix_ts = now.timestamp();
    let mut content = String::with_capacity(parts.text.len() + 1024);
    content.push_str(&format!("To: {recipient}\r\n"));
    content.push_str(&format!("Subject: {header_subject}\r\n"));
    content.push_str(&format!("Date: {date_header}\r\n"));
    content.push_str("MIME-Version: 1.0\r\n");

    match &parts.html {
        None => {
            // Legacy single-part text/plain path — unchanged behavior.
            content.push_str("Content-Type: text/plain; charset=utf-8\r\n");
            content.push_str("Content-Transfer-Encoding: 8bit\r\n");
            content.push_str("\r\n");
            push_crlf_normalized(&mut content, &parts.text);
        }
        Some(html) => {
            // multipart/alternative — text first, html last (RFC 2046:
            // clients pick the LAST renderable part). No top-level
            // Content-Transfer-Encoding per RFC 2045 §6.4 — the CTE
            // lives on each part instead.
            //
            // The boundary's leading "=_" is unreachable in QP output:
            // QP encodes a literal `=` as `=3D`, never as a bare `=` mid-
            // line, so "=_" cannot appear in either encoded part body.
            let boundary = format!("=_captainslog_{seq:04}_{unix_ts}_=");
            content.push_str(&format!(
                "Content-Type: multipart/alternative; boundary=\"{boundary}\"\r\n"
            ));
            content.push_str("\r\n");
            content.push_str("This is a multi-part message in MIME format.\r\n");

            // Part 1: text/plain, QP-encoded.
            content.push_str(&format!("--{boundary}\r\n"));
            content.push_str("Content-Type: text/plain; charset=utf-8\r\n");
            content.push_str("Content-Transfer-Encoding: quoted-printable\r\n");
            content.push_str("\r\n");
            push_qp_part(&mut content, &parts.text);
            content.push_str("\r\n");

            // Part 2: text/html, QP-encoded.
            content.push_str(&format!("--{boundary}\r\n"));
            content.push_str("Content-Type: text/html; charset=utf-8\r\n");
            content.push_str("Content-Transfer-Encoding: quoted-printable\r\n");
            content.push_str("\r\n");
            push_qp_part(&mut content, html);
            content.push_str("\r\n");

            // Closing boundary.
            content.push_str(&format!("--{boundary}--\r\n"));
        }
    }

    std::fs::write(&file_path, content)?;
    Ok(file_path)
}

/// Append `body` to `out` with all line endings normalized to CRLF. Used by
/// the single-part text path; multipart parts go through `push_qp_part`
/// which produces CRLF natively.
fn push_crlf_normalized(out: &mut String, body: &str) {
    for line in body.split('\n') {
        out.push_str(line.trim_end_matches('\r'));
        out.push_str("\r\n");
    }
}

/// QP-encode `body` and append the result to `out`. The `quoted_printable`
/// crate wraps at 76 chars with soft line breaks. It does NOT translate
/// bare `\n` to CRLF — any standalone LF in the input is treated as a
/// non-printable byte and emitted as the literal escape `=0A`, which
/// reads as a single run-on line in mail clients. We normalize line
/// endings to CRLF first so they pass through as legitimate hard line
/// breaks (CR=`=0D`, LF=`=0A` on their own get escaped, but a CRLF pair
/// is preserved as a real line terminator by the encoder).
fn push_qp_part(out: &mut String, body: &str) {
    let normalized: String = body
        .split('\n')
        .map(|line| line.trim_end_matches('\r'))
        .collect::<Vec<_>>()
        .join("\r\n");
    out.push_str(&quoted_printable::encode_to_str(&normalized));
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
            format: BodyFormat::Text,
            send: None,
        }
    }

    #[test]
    fn empty_recipient_produces_mailto_with_blank_to() {
        // Phase 2.7 dropped the "no recipient = error" gate; users
        // without a manager email on file still get a draft, just one
        // with a blank To: line they can fill in.
        let s = summary("a", "b", "c", "d", &[]);
        let r = compose_weekly_email(params(&s, "Week of X", "")).unwrap();
        match r {
            ComposeResult::Mailto { value: url } => {
                assert!(
                    url.starts_with("mailto:?"),
                    "expected blank To; got {url}"
                );
                assert!(url.contains("subject=Weekly%20update"));
            }
            ComposeResult::Eml { value: _ } => panic!("expected mailto, got eml"),
            other => panic!("unexpected ComposeResult variant: {other:?}"),
        }
    }

    #[test]
    fn whitespace_recipient_is_treated_as_empty() {
        let s = summary("a", "b", "c", "d", &[]);
        let r = compose_weekly_email(params(&s, "Week of X", "   ")).unwrap();
        match r {
            ComposeResult::Mailto { value: url } => {
                assert!(url.starts_with("mailto:?"));
            }
            ComposeResult::Eml { value: _ } => panic!("expected mailto, got eml"),
            other => panic!("unexpected ComposeResult variant: {other:?}"),
        }
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
            ComposeResult::Mailto { value: url } => {
                assert!(url.starts_with("mailto:boss%40example.com?"));
                assert!(url.contains("subject=Weekly%20update"));
                assert!(url.contains("Shipped%20X."));
            }
            ComposeResult::Eml { value: _ } => panic!("expected mailto, got eml"),
            other => panic!("unexpected ComposeResult variant: {other:?}"),
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
            ComposeResult::Eml { value: p } => p,
            ComposeResult::Mailto { value: _ } => panic!("expected eml fallback"),
            other => panic!("unexpected ComposeResult variant: {other:?}"),
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
            ComposeResult::Mailto { value: u } => u,
            ComposeResult::Eml { value: _ } => panic!("expected mailto"),
            other => panic!("unexpected ComposeResult variant: {other:?}"),
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
            ComposeResult::Mailto { value: u } => u,
            ComposeResult::Eml { value: _ } => panic!("expected mailto"),
            other => panic!("unexpected ComposeResult variant: {other:?}"),
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
            ComposeResult::Mailto { value: u } => u,
            ComposeResult::Eml { value: _ } => panic!("expected mailto"),
            other => panic!("unexpected ComposeResult variant: {other:?}"),
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
            ComposeResult::Mailto { value: u } => u,
            ComposeResult::Eml { value: _ } => panic!("expected mailto"),
            other => panic!("unexpected ComposeResult variant: {other:?}"),
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
            ComposeResult::Mailto { value: u } => u,
            ComposeResult::Eml { value: _ } => panic!("expected mailto"),
            other => panic!("unexpected ComposeResult variant: {other:?}"),
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
            ComposeResult::Mailto { value: u } => u,
            ComposeResult::Eml { value: _ } => panic!("expected mailto"),
            other => panic!("unexpected ComposeResult variant: {other:?}"),
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
            ComposeResult::Mailto { value: u } => u,
            ComposeResult::Eml { value: _ } => panic!("expected mailto"),
            other => panic!("unexpected ComposeResult variant: {other:?}"),
        };
        assert!(url.contains("Hello%20Pat%2C"), "expected `Hello Pat,` in body, url={url}");
    }

    #[test]
    fn greeting_falls_back_when_manager_name_missing() {
        let s = summary("k", "", "", "", &[]);
        let url = match compose_weekly_email(params(&s, "Week of X", "boss@example.com")).unwrap()
        {
            ComposeResult::Mailto { value: u } => u,
            ComposeResult::Eml { value: _ } => panic!("expected mailto"),
            other => panic!("unexpected ComposeResult variant: {other:?}"),
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
            ComposeResult::Mailto { value: u } => u,
            ComposeResult::Eml { value: _ } => panic!("expected mailto"),
            other => panic!("unexpected ComposeResult variant: {other:?}"),
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
            ComposeResult::Mailto { value: u } => u,
            ComposeResult::Eml { value: _ } => panic!("expected mailto"),
            other => panic!("unexpected ComposeResult variant: {other:?}"),
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
            ComposeResult::Mailto { value: u } => u,
            ComposeResult::Eml { value: _ } => panic!("expected mailto"),
            other => panic!("unexpected ComposeResult variant: {other:?}"),
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
            ComposeResult::Mailto { value: u } => u,
            ComposeResult::Eml { value: _ } => panic!("expected mailto"),
            other => panic!("unexpected ComposeResult variant: {other:?}"),
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
            ComposeResult::Eml { value: p } => p,
            ComposeResult::Mailto { value: _ } => panic!("expected eml fallback"),
            other => panic!("unexpected ComposeResult variant: {other:?}"),
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

    // ---- Phase 2.9: HTML / multipart-alternative path ----

    fn read_eml(path: &std::path::Path) -> String {
        let written = std::fs::read_to_string(path).unwrap();
        let _ = std::fs::remove_file(path);
        written
    }

    #[test]
    fn html_format_emits_multipart_alternative() {
        let s = summary("Shipped X.", "Plan Y.", "", "Note.", &["mage"]);
        let mut p = params(&s, "week of Jun 22 - Jun 28, 2026", "boss@example.com");
        p.format = BodyFormat::Html;
        let r = compose_weekly_email(p).unwrap();
        let path = match r {
            ComposeResult::Eml { value: p } => p,
            ComposeResult::Mailto { value: _ } => panic!("expected eml for html format"),
            other => panic!("unexpected ComposeResult variant: {other:?}"),
        };
        let written = read_eml(&path);
        // Header must declare multipart/alternative and carry a boundary.
        let boundary_line = written
            .lines()
            .find(|l| l.starts_with("Content-Type: multipart/alternative"))
            .expect("multipart/alternative top-level Content-Type");
        let boundary = boundary_line
            .split("boundary=\"")
            .nth(1)
            .and_then(|s| s.split('"').next())
            .expect("boundary token present");
        // Open + middle + close delimiters — three `--boundary` markers.
        // (The boundary token itself also appears in the Content-Type header,
        // so a raw substring count would be 4.)
        let delim_open = format!("--{boundary}");
        let delim_close = format!("--{boundary}--");
        // Closing delimiter has its own `--boundary--`; that's also matched
        // by `--boundary`. The opener test counts open+mid (2) plus close (1).
        let open_count = written.matches(&delim_open).count();
        let close_count = written.matches(&delim_close).count();
        assert_eq!(
            open_count, 3,
            "expected 3 `--boundary` markers (open/mid/close), got {open_count}"
        );
        assert_eq!(close_count, 1, "expected exactly one closing `--boundary--`");
        // Both per-part content types must be present.
        assert!(written.contains("Content-Type: text/plain; charset=utf-8"));
        assert!(written.contains("Content-Type: text/html; charset=utf-8"));
        // text/plain MUST come before text/html (RFC 2046 ordering).
        let text_idx = written.find("Content-Type: text/plain").unwrap();
        let html_idx = written.find("Content-Type: text/html").unwrap();
        assert!(text_idx < html_idx, "text/plain must precede text/html");
        // Top-level CTE must NOT appear (multipart containers don't take one).
        let header_block = &written[..written.find("\r\n\r\n").unwrap()];
        assert!(
            !header_block.contains("Content-Transfer-Encoding"),
            "top-level headers must not declare CTE"
        );
        // Each part declares its own CTE.
        assert_eq!(
            written.matches("Content-Transfer-Encoding: quoted-printable").count(),
            2
        );
    }

    #[test]
    fn html_format_always_eml_never_mailto() {
        // Even with a tiny body — well under MAILTO_MAX_BYTES — html mode
        // skips the mailto branch entirely.
        let s = summary("k", "", "", "", &[]);
        let mut p = params(&s, "Week of X", "boss@example.com");
        p.format = BodyFormat::Html;
        let r = compose_weekly_email(p).unwrap();
        match r {
            ComposeResult::Eml { value: path } => {
                let _ = std::fs::remove_file(path);
            }
            ComposeResult::Mailto { value: url } => panic!("expected eml, got mailto: {url}"),
            other => panic!("unexpected ComposeResult variant: {other:?}"),
        }
    }

    #[test]
    fn quoted_printable_lines_under_76_chars() {
        // Long URL + long prose forces QP to insert soft line breaks.
        let long = "https://github.com/ChrisACarpenter/captains-log/blob/main/whatever-this-is-a-very-long-pathname-to-force-qp-wrapping.md ".repeat(5);
        let s = summary(&long, "more very long content ".repeat(20).as_str(), "", "", &[]);
        let mut p = params(&s, "Week of X", "boss@example.com");
        p.format = BodyFormat::Html;
        let path = match compose_weekly_email(p).unwrap() {
            ComposeResult::Eml { value: p } => p,
            ComposeResult::Mailto { value: _ } => panic!("expected eml"),
            other => panic!("unexpected ComposeResult variant: {other:?}"),
        };
        let written = read_eml(&path);
        // Only the QP-encoded part bodies must respect the 76-char wrap.
        // Top-level headers (Content-Type with a boundary, RFC 2047 Subject)
        // and boundary delimiter lines can legitimately exceed 76; the
        // 76-char rule is from RFC 2045 §6.7 about QP, not about headers.
        for (i, line) in written.split("\r\n").enumerate() {
            if line.starts_with("To:")
                || line.starts_with("Subject:")
                || line.starts_with("Date:")
                || line.starts_with("MIME-Version:")
                || line.starts_with("Content-Type:")
                || line.starts_with("Content-Transfer-Encoding:")
                || line.starts_with("--=_captainslog_")
                || line == "This is a multi-part message in MIME format."
                || line.is_empty()
            {
                continue;
            }
            assert!(
                line.len() <= 76,
                "QP body line {i} exceeds 76 chars ({}): {line:?}",
                line.len()
            );
        }
    }

    #[test]
    fn quoted_printable_no_bare_equals() {
        // After QP encoding, every `=` in either part body must be followed
        // by a hex pair (=XX) or by CRLF (soft line break).
        let s = summary(
            "Shipped X & Y = success",
            "Stuff = more stuff",
            "",
            "",
            &[],
        );
        let mut p = params(&s, "Week of X", "boss@example.com");
        p.format = BodyFormat::Html;
        let path = match compose_weekly_email(p).unwrap() {
            ComposeResult::Eml { value: p } => p,
            ComposeResult::Mailto { value: _ } => panic!("expected eml"),
            other => panic!("unexpected ComposeResult variant: {other:?}"),
        };
        let written = read_eml(&path);
        // Isolate the part bodies: everything after the first blank line that
        // follows a Content-Transfer-Encoding: quoted-printable header, up to
        // the next boundary marker. Easier path: scan the whole file but skip
        // header lines that legitimately contain `=` (Content-Type boundary=,
        // RFC 2047 subject =?UTF-8?B?, the boundary lines themselves).
        for line in written.split("\r\n") {
            // Skip header-y lines and boundary delimiters.
            if line.starts_with("Content-Type:")
                || line.starts_with("Content-Transfer-Encoding:")
                || line.starts_with("Subject:")
                || line.starts_with("--=_captainslog_")
                || line.is_empty()
            {
                continue;
            }
            let bytes = line.as_bytes();
            for (i, &b) in bytes.iter().enumerate() {
                if b != b'=' {
                    continue;
                }
                // Either followed by two hex digits, or `=` is the LAST char
                // on the line (soft line break — the actual CRLF terminator
                // was consumed by split).
                if i + 1 == bytes.len() {
                    continue;
                }
                let h1 = bytes.get(i + 1).copied().unwrap_or(0);
                let h2 = bytes.get(i + 2).copied().unwrap_or(0);
                let is_hex = |b: u8| b.is_ascii_digit() || (b'A'..=b'F').contains(&b);
                assert!(
                    is_hex(h1) && is_hex(h2),
                    "bare `=` at offset {i} in line {line:?} — expected hex pair or end-of-line"
                );
            }
        }
    }

    #[test]
    fn html_part_contains_styled_markup() {
        // The HTML body from email_html::render_body_html ships with inlined
        // styles; after QP-encoding the markup should still be recognizable
        // as styled HTML. QP encodes `=` as `=3D`, so an inline style="..."
        // becomes style=3D"...". We assert either the literal `<style>` block
        // (= unencoded `<` ASCII passthrough) OR a QP-encoded `style=3D"`.
        let s = summary("Shipped X.", "", "", "", &[]);
        let mut p = params(&s, "Week of X", "boss@example.com");
        p.format = BodyFormat::Html;
        let path = match compose_weekly_email(p).unwrap() {
            ComposeResult::Eml { value: p } => p,
            ComposeResult::Mailto { value: _ } => panic!("expected eml"),
            other => panic!("unexpected ComposeResult variant: {other:?}"),
        };
        let written = read_eml(&path);
        // QP preserves ASCII `<`/`>` (they're in the safe range) — only `=`
        // and non-ASCII get encoded. So `<style>` (if present) survives,
        // and `style="` becomes `style=3D"`.
        assert!(
            written.contains("<style>")
                || written.contains("style=3D\"")
                || written.contains("style=\""),
            "html part should contain inline styles"
        );
    }

    /// Extract the body of a multipart part (between the part's blank-line
    /// header/body separator and the next boundary delimiter) for a part
    /// matched by a `Content-Type:` substring. Used by the QP regression
    /// test to inspect decoded text/plain + text/html bodies independently.
    fn extract_part_body<'a>(raw: &'a str, content_type_marker: &str) -> &'a str {
        // Find the part's Content-Type header line.
        let ct_idx = raw
            .find(&format!("Content-Type: {content_type_marker}"))
            .expect("expected part with matching Content-Type");
        // Body starts after the next blank line ("\r\n\r\n").
        let after_headers = raw[ct_idx..]
            .find("\r\n\r\n")
            .map(|p| ct_idx + p + 4)
            .expect("expected blank line after part headers");
        // Body ends at the next boundary delimiter line. Boundaries start
        // with "--=_captainslog_".
        let tail = &raw[after_headers..];
        let body_end_rel = tail
            .find("\r\n--=_captainslog_")
            .expect("expected closing boundary for part");
        &tail[..body_end_rel]
    }

    #[test]
    fn qp_part_decodes_to_crlf_not_bare_lf() {
        // Regression: push_qp_part used to feed the body straight into
        // quoted_printable::encode_to_str, which escapes bare LF as `=0A`
        // and emits no real line break. Decoded bodies would arrive as
        // one long run-on paragraph. We now normalize to CRLF first.
        let s = summary(
            "Line one.\nLine two.\n\nNew paragraph.",
            "Plan one.\nPlan two.",
            "",
            "Done.",
            &["mage"],
        );
        let mut p = params(&s, "week of Jun 22 - Jun 28, 2026", "boss@example.com");
        p.format = BodyFormat::Html;
        let path = match compose_weekly_email(p).unwrap() {
            ComposeResult::Eml { value: p } => p,
            ComposeResult::Mailto { value: _ } => panic!("expected eml for html format"),
            other => panic!("unexpected ComposeResult variant: {other:?}"),
        };
        let raw = std::fs::read_to_string(&path).unwrap();
        let text_part = extract_part_body(&raw, "text/plain");
        let html_part = extract_part_body(&raw, "text/html");
        let text_decoded = String::from_utf8(
            quoted_printable::decode(text_part.as_bytes(), quoted_printable::ParseMode::Strict)
                .unwrap(),
        )
        .unwrap();
        let html_decoded = String::from_utf8(
            quoted_printable::decode(html_part.as_bytes(), quoted_printable::ParseMode::Strict)
                .unwrap(),
        )
        .unwrap();
        // Every LF byte in either decoded body must be preceded by CR.
        for (name, body) in [("text/plain", &text_decoded), ("text/html", &html_decoded)] {
            let bytes = body.as_bytes();
            for (i, &b) in bytes.iter().enumerate() {
                if b == b'\n' {
                    assert!(
                        i > 0 && bytes[i - 1] == b'\r',
                        "bare LF in {name} at byte {i}: ...{:?}",
                        &body[i.saturating_sub(20)..(i + 1).min(body.len())]
                    );
                }
            }
            // Sanity: both bodies must contain at least one real CRLF break.
            assert!(body.contains("\r\n"), "{name} body should contain CRLF breaks");
        }
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
            ComposeResult::Mailto { value: u } => u,
            ComposeResult::Eml { value: _ } => panic!("expected mailto"),
            other => panic!("unexpected ComposeResult variant: {other:?}"),
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

    // ---- Phase 2.9b: per-mode dispatch ----

    /// Convenience builder for the new dispatch params. Defaults to Gmail
    /// + clean text + no user_email + Business Outlook (irrelevant for
    /// Gmail tests) + native_html off — every test overrides what it
    /// cares about.
    fn send(mode: MailSendMode) -> MailSend<'static> {
        MailSend {
            mode,
            body_format: MailBodyFormat::CleanText,
            user_email: None,
            outlook_flavor: OutlookFlavor::Business,
            native_html: false,
            body_in_clipboard: false,
        }
    }

    fn assert_web_url(r: ComposeResult) -> (String, bool) {
        match r {
            ComposeResult::WebUrl {
                url,
                truncation_warning,
            } => (url, truncation_warning),
            other => panic!("expected WebUrl, got {other:?}"),
        }
    }

    fn assert_applescript(r: ComposeResult) -> String {
        match r {
            ComposeResult::AppleScript { script } => script,
            other => panic!("expected AppleScript, got {other:?}"),
        }
    }

    #[test]
    fn gmail_url_uses_account_zero_without_user_email() {
        // No user_email → `/mail/u/0/` (Gmail's "first signed-in account"
        // fallback). Multi-account users should set userEmail in settings;
        // this is the single-account/unset path.
        let s = summary("k", "", "", "", &[]);
        let mut p = params(&s, "Week of X", "boss@example.com");
        p.send = Some(send(MailSendMode::Gmail));
        let (url, _warn) = assert_web_url(compose_weekly_email(p).unwrap());
        assert!(
            url.starts_with("https://mail.google.com/mail/u/0/?view=cm&tf=cm&to="),
            "expected /mail/u/0/ slot, got: {url}"
        );
        assert!(url.contains("&su="), "subject param must be `su`, not `subject`");
    }

    #[test]
    fn gmail_url_pins_account_to_user_email() {
        // user_email is percent-encoded into the `/mail/u/{addr}` slot so
        // multi-account Gmail routes to the right inbox.
        let s = summary("k", "", "", "", &[]);
        let mut p = params(&s, "Week of X", "boss@example.com");
        p.send = Some(MailSend {
            user_email: Some("chris.carpenter@prodigygame.com"),
            ..send(MailSendMode::Gmail)
        });
        let (url, _warn) = assert_web_url(compose_weekly_email(p).unwrap());
        // `@` and `.` are non-alphanumeric → encoded as %40 and %2E.
        assert!(
            url.contains("/mail/u/chris%2Ecarpenter%40prodigygame%2Ecom/"),
            "expected encoded user_email in slot, got: {url}"
        );
    }

    #[test]
    fn gmail_url_encodes_ampersand_in_body() {
        // `&` must encode to `%26` — otherwise Gmail would split the URL
        // into multiple query params and silently drop body content.
        let s = summary("Q & A session with PM", "", "", "", &[]);
        let mut p = params(&s, "Week of X", "boss@example.com");
        p.send = Some(MailSend {
            body_format: MailBodyFormat::MarkdownSource,
            ..send(MailSendMode::Gmail)
        });
        let (url, _warn) = assert_web_url(compose_weekly_email(p).unwrap());
        assert!(url.contains("Q%20%26%20A"), "&  must encode to %26 in body, url={url}");
        // Genuine query separators only — Gmail's URL has 4 of them
        // (view=cm&tf=cm&to=…&su=…&body=…). Anything more would mean a
        // raw `&` slipped through from the body.
        assert_eq!(
            url.matches('&').count(),
            4,
            "expected exactly 4 query-separator `&`s (view/tf/to/su/body), got url={url}"
        );
    }

    #[test]
    fn gmail_url_encodes_newlines_in_body_as_percent_0a() {
        // Newlines in the rendered body must become %0A — Gmail strips
        // bare LFs from the body param and the message arrives as one
        // run-on paragraph otherwise.
        let s = summary("Line one.\nLine two.", "", "", "", &[]);
        let mut p = params(&s, "Week of X", "boss@example.com");
        p.send = Some(MailSend {
            body_format: MailBodyFormat::MarkdownSource,
            ..send(MailSendMode::Gmail)
        });
        let (url, _warn) = assert_web_url(compose_weekly_email(p).unwrap());
        // NON_ALPHANUMERIC encodes `.` as `%2E`. The point of the test
        // is the `%0A` separator between the two sentences — not the
        // literal `.` byte.
        assert!(
            url.contains("Line%20one%2E%0ALine%20two%2E"),
            "expected encoded LFs in body, url={url}"
        );
    }

    #[test]
    fn gmail_url_long_body_sets_truncation_warning() {
        // Anything over ~2 KB encoded triggers Gmail's silent truncation;
        // the warning flag lets the frontend show a "Send anyway" gate.
        let big = "x".repeat(3000);
        let s = summary(&big, "", "", "", &[]);
        let mut p = params(&s, "Week of X", "boss@example.com");
        p.send = Some(send(MailSendMode::Gmail));
        let (url, warn) = assert_web_url(compose_weekly_email(p).unwrap());
        assert!(warn, "expected truncation_warning=true for long body");
        assert!(url.len() > GMAIL_URL_WARN_BYTES);
    }

    #[test]
    fn gmail_url_short_body_no_truncation_warning() {
        let s = summary("k", "", "", "", &[]);
        let mut p = params(&s, "Week of X", "boss@example.com");
        p.send = Some(send(MailSendMode::Gmail));
        let (_url, warn) = assert_web_url(compose_weekly_email(p).unwrap());
        assert!(!warn, "short body should not warn");
    }

    #[test]
    fn outlook_business_url_uses_office_host() {
        let s = summary("Shipped X.", "", "", "", &[]);
        let mut p = params(&s, "Week of X", "boss@example.com");
        p.send = Some(MailSend {
            outlook_flavor: OutlookFlavor::Business,
            ..send(MailSendMode::Outlook)
        });
        let (url, warn) = assert_web_url(compose_weekly_email(p).unwrap());
        assert!(
            url.starts_with("https://outlook.office.com/mail/deeplink/compose?to="),
            "expected outlook.office.com host, got: {url}"
        );
        // Outlook uses `subject=`, not Gmail's legacy `su=`.
        assert!(url.contains("&subject="), "expected `subject` param, got: {url}");
        // No truncation check for Outlook — flag must be false.
        assert!(!warn, "Outlook never sets truncation_warning");
    }

    #[test]
    fn outlook_personal_url_uses_live_host() {
        let s = summary("Shipped X.", "", "", "", &[]);
        let mut p = params(&s, "Week of X", "boss@example.com");
        p.send = Some(MailSend {
            outlook_flavor: OutlookFlavor::Personal,
            ..send(MailSendMode::Outlook)
        });
        let (url, _warn) = assert_web_url(compose_weekly_email(p).unwrap());
        assert!(
            url.starts_with("https://outlook.live.com/mail/0/deeplink/compose?to="),
            "expected outlook.live.com host, got: {url}"
        );
    }

    #[test]
    fn applescript_template_substitutes_subject_body_recipient() {
        let s = summary("Shipped X.", "", "", "", &[]);
        let mut p = params(&s, "Week of X", "boss@example.com");
        p.send = Some(send(MailSendMode::NativeMail));
        let script = assert_applescript(compose_weekly_email(p).unwrap());
        // Core template shape: outgoing message + to recipient.
        assert!(script.contains(r#"tell application "Mail""#));
        assert!(script.contains("make new outgoing message with properties"));
        assert!(script.contains(r#"subject:"Weekly update - Week of X""#));
        assert!(script.contains(r#"address:"boss@example.com""#));
        assert!(script.contains("activate"));
        // No user_email → no `set sender` line.
        assert!(!script.contains("set sender to"));
    }

    #[test]
    fn applescript_escapes_double_quotes_in_subject() {
        // A `"` in the subject would close the AppleScript literal early
        // and produce a syntax error. Must escape to `\"`.
        let s = summary("k", "", "", "", &[]);
        let mut p = params(&s, "she said \"hi\"", "boss@example.com");
        p.is_resend = false;
        p.send = Some(send(MailSendMode::NativeMail));
        let script = assert_applescript(compose_weekly_email(p).unwrap());
        // Subject becomes: Weekly update - she said \"hi\"
        assert!(
            script.contains(r#"subject:"Weekly update - she said \"hi\"""#),
            "expected escaped quotes in subject, script=\n{script}"
        );
    }

    #[test]
    fn applescript_escapes_backslashes_in_body() {
        // Backslashes in user content (e.g. file paths in code blocks)
        // must double up — AppleScript treats `\` as an escape character.
        let s = summary("path: C:\\Users\\Chris", "", "", "", &[]);
        let mut p = params(&s, "Week of X", "boss@example.com");
        p.send = Some(MailSend {
            body_format: MailBodyFormat::MarkdownSource,
            ..send(MailSendMode::NativeMail)
        });
        let script = assert_applescript(compose_weekly_email(p).unwrap());
        assert!(
            script.contains(r"C:\\Users\\Chris"),
            "expected backslashes to be doubled, script=\n{script}"
        );
    }

    #[test]
    fn applescript_includes_sender_when_user_email_set() {
        // Multi-account Mail.app routes outgoing messages by `sender`. When
        // user_email is set we drop a `set sender to "..."` line into the
        // inner `tell newMessage` block.
        let s = summary("k", "", "", "", &[]);
        let mut p = params(&s, "Week of X", "boss@example.com");
        p.send = Some(MailSend {
            user_email: Some("chris.carpenter@prodigygame.com"),
            ..send(MailSendMode::NativeMail)
        });
        let script = assert_applescript(compose_weekly_email(p).unwrap());
        assert!(
            script.contains(r#"set sender to "chris.carpenter@prodigygame.com""#),
            "expected `set sender to` line, script=\n{script}"
        );
    }

    // ---- Phase 2.9b "Compose + paste" — body_in_clipboard ----

    #[test]
    fn gmail_url_empty_body_when_body_in_clipboard() {
        // Clipboard-paste mode: the frontend has already written rich HTML
        // to the clipboard, so the URL must open compose with an EMPTY
        // body so the user's Cmd+V doesn't collide with prefilled text.
        let s = summary("Shipped the thing", "Plan", "Block", "Else", &["mage"]);
        let mut p = params(&s, "Week of X", "boss@example.com");
        p.send = Some(MailSend {
            body_in_clipboard: true,
            ..send(MailSendMode::Gmail)
        });
        let (url, warn) = assert_web_url(compose_weekly_email(p).unwrap());
        assert!(url.contains("&body=&") || url.ends_with("&body="),
            "expected empty body= param, got: {url}");
        assert!(!url.contains("Shipped"), "expected body content stripped, got: {url}");
        // Empty body can't overflow the 2000-byte guideline.
        assert!(!warn, "empty-body URL must never set truncation_warning");
    }

    #[test]
    fn outlook_business_url_empty_body_when_body_in_clipboard() {
        let s = summary("Shipped", "", "", "", &[]);
        let mut p = params(&s, "Week of X", "boss@example.com");
        p.send = Some(MailSend {
            body_in_clipboard: true,
            ..send(MailSendMode::Outlook)
        });
        let (url, _warn) = assert_web_url(compose_weekly_email(p).unwrap());
        assert!(url.starts_with("https://outlook.office.com/"));
        assert!(url.contains("&body=") && !url.contains("Shipped"),
            "expected empty body param, got: {url}");
    }

    #[test]
    fn applescript_empty_body_when_body_in_clipboard() {
        // Mac Mail plaintext path also goes empty in clipboard-paste mode;
        // the user pastes rich HTML via Cmd+V into Mail.app's compose body.
        let s = summary("Shipped", "Plan", "", "", &[]);
        let mut p = params(&s, "Week of X", "boss@example.com");
        p.send = Some(MailSend {
            body_in_clipboard: true,
            ..send(MailSendMode::NativeMail)
        });
        let script = assert_applescript(compose_weekly_email(p).unwrap());
        assert!(script.contains(r#"content:"""#),
            "expected empty content property, script=\n{script}");
        assert!(!script.contains("Shipped"), "body should not be in the script");
    }

    #[test]
    fn native_html_takes_precedence_over_body_in_clipboard() {
        // The .eml HTML toggle is an independent peer override per the
        // 2.9c spec — when both are set, the .eml path wins (recipient
        // gets a styled body without needing the user to paste anything).
        let s = summary("Shipped", "", "", "", &[]);
        let mut p = params(&s, "Week of X", "boss@example.com");
        p.send = Some(MailSend {
            body_in_clipboard: true,
            native_html: true,
            ..send(MailSendMode::NativeMail)
        });
        let result = compose_weekly_email(p).unwrap();
        match result {
            ComposeResult::Eml { .. } => {}
            _ => panic!("expected ComposeResult::Eml when native_html=true regardless of body_in_clipboard"),
        }
    }

    // ---- plaintext_from_markdown ----

    #[test]
    fn plaintext_h2_becomes_all_caps_with_blank_line() {
        let md = "## Key accomplishments\n\nShipped X.\n";
        let out = plaintext_from_markdown(md, MailBodyFormat::CleanText);
        assert!(
            out.contains("KEY ACCOMPLISHMENTS\n\n"),
            "expected uppercase + blank line after H2, got:\n{out}"
        );
        assert!(out.contains("Shipped X."));
        // The `## ` markers themselves must be gone.
        assert!(!out.contains("##"));
    }

    #[test]
    fn plaintext_preserves_bullet_list_with_dashes() {
        let md = "- first\n- second\n";
        let out = plaintext_from_markdown(md, MailBodyFormat::CleanText);
        assert!(out.contains("- first"), "expected bullet, got:\n{out}");
        assert!(out.contains("- second"), "expected bullet, got:\n{out}");
    }

    #[test]
    fn plaintext_strips_bold_markers() {
        // `Shipped **the thing**` → `Shipped the thing` in clean-text mode.
        let md = "Shipped **the thing** today.";
        let out = plaintext_from_markdown(md, MailBodyFormat::CleanText);
        assert!(
            out.contains("Shipped the thing today."),
            "expected bold markers stripped, got:\n{out}"
        );
        assert!(!out.contains("**"));
    }

    #[test]
    fn plaintext_link_becomes_label_with_url_in_parens() {
        let md = "See the [project doc](https://example.com/x) for details.";
        let out = plaintext_from_markdown(md, MailBodyFormat::CleanText);
        assert!(
            out.contains("project doc (https://example.com/x)"),
            "expected `label (url)`, got:\n{out}"
        );
    }

    #[test]
    fn plaintext_markdown_source_passes_through_unchanged() {
        // MarkdownSource mode is the identity transform — useful for users
        // who want the raw markdown visible in their sent folder.
        let md = "## Heading\n\n- **bold** item\n";
        let out = plaintext_from_markdown(md, MailBodyFormat::MarkdownSource);
        assert_eq!(out, md);
    }

    #[test]
    fn plaintext_keeps_code_block_content() {
        let md = "```\nfoo bar\n```\n";
        let out = plaintext_from_markdown(md, MailBodyFormat::CleanText);
        assert!(
            out.contains("foo bar"),
            "expected code block contents preserved, got:\n{out}"
        );
    }
}
