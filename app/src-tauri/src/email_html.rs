//! Render a [`WeeklySummary`] into a complete, inline-styled HTML document
//! suitable for the `text/html` part of a multipart/alternative email.
//!
//! ## Pipeline
//!
//! For each non-empty summary field:
//!
//!   1. Feed the user's markdown into `pulldown_cmark::Parser` with
//!      `ENABLE_TABLES | ENABLE_STRIKETHROUGH | ENABLE_SMART_PUNCTUATION`.
//!      Skip `ENABLE_TASKLISTS` for v1 — Outlook desktop's Word engine
//!      renders `<input type=checkbox>` inconsistently.
//!   2. `html::push_html` into a `String`.
//!   3. `ammonia::Builder` with an allowlist of the tags we expect — this
//!      strips `<script>` and any raw HTML the user pasted into the markdown,
//!      and constrains `href` to `http`/`https`/`mailto` so `javascript:`
//!      links don't survive.
//!   4. [`inline_styles`] post-processes the cleaned HTML, rewriting opening
//!      tags (`<p>`, `<h2>`, etc.) to carry an inline `style="..."`
//!      attribute. We do this after ammonia (instead of authoring a custom
//!      pulldown-cmark event adapter) because (a) it's ~30 lines of
//!      find-and-rewrite, (b) ammonia is the only thing that should be
//!      deciding which tags survive, and (c) the rewrite is byte-cheap.
//!
//! The four sections are concatenated under the same document shell as the
//! plaintext body (greeting, intro paragraph, optional labels footer).
//!
//! ## Email-client targeting
//!
//! Styles target Apple Mail, Gmail, and Outlook web. Word-engine Outlook
//! (classic Windows) will drop rounded corners and some padding — the
//! report is still legible there, just less pretty. See the Phase 2.9
//! research notes for the full client matrix.

use pulldown_cmark::{html, Options, Parser};

use crate::email::CAPTAINS_LOG_REPO_URL;
use crate::notes::WeeklySummary;

/// Render the summary as a self-contained HTML document.
///
/// `manager_name` drives the greeting ("Hello Manny," vs. "Hello,"),
/// mirroring `render_body` in `email.rs` so the plaintext/HTML alternative
/// parts open identically. Whitespace-only names fall back to the plain
/// greeting.
pub fn render_body_html(
    summary: &WeeklySummary,
    week_label: &str,
    manager_name: Option<&str>,
) -> String {
    let mut sections = String::new();
    push_section_html(
        &mut sections,
        "Key accomplishments",
        &summary.key_accomplishments,
    );
    push_section_html(
        &mut sections,
        "Plans and priorities for next week",
        &summary.plans_and_priorities,
    );
    push_section_html(
        &mut sections,
        "Challenges or roadblocks",
        &summary.challenges_or_roadblocks,
    );
    push_section_html(&mut sections, "Anything else on your mind", &summary.anything_else);

    let labels_html = render_labels_html(&summary.labels);

    let greeting = match manager_name.map(str::trim).filter(|s| !s.is_empty()) {
        Some(name) => format!("Hello {},", escape_text(name)),
        None => "Hello,".to_string(),
    };

    let intro = format!(
        "This is my update for the {label}, sent through \
         <a href=\"{url}\" style=\"{a_style}\">Captain\u{2019}s Log</a>.",
        label = escape_text(week_label),
        url = escape_attr(CAPTAINS_LOG_REPO_URL),
        a_style = STYLE_A,
    );

    format!(
        "<!DOCTYPE html>\n\
         <html>\n\
         <head>\n\
         <meta charset=\"utf-8\">\n\
         <title>Weekly update</title>\n\
         </head>\n\
         <body style=\"{body_style}\">\n\
         <div style=\"{card_style}\">\n\
         <p style=\"{p_style}\">{greeting}</p>\n\
         <p style=\"{p_style}\">{intro}</p>\n\
         {sections}\
         {labels}\
         </div>\n\
         </body>\n\
         </html>\n",
        body_style = STYLE_BODY,
        card_style = STYLE_CARD,
        p_style = STYLE_P,
        greeting = greeting,
        intro = intro,
        sections = sections,
        labels = labels_html,
    )
}

/// Render a single summary section (heading + markdown body) into the
/// output buffer. Empty fields produce nothing, matching the plaintext
/// `render_body` behavior.
fn push_section_html(out: &mut String, heading: &str, body: &str) {
    let body = body.trim();
    if body.is_empty() {
        return;
    }
    out.push_str(&format!(
        "<h2 style=\"{h2}\">{heading}</h2>\n",
        h2 = STYLE_H2,
        heading = escape_text(heading),
    ));
    out.push_str(&render_markdown_html(body));
    out.push('\n');
}

/// Run a markdown body through pulldown-cmark + ammonia + inline_styles.
/// Public for tests; not part of the module's stable API.
fn render_markdown_html(md: &str) -> String {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_SMART_PUNCTUATION);

    let mut raw = String::with_capacity(md.len() * 2);
    html::push_html(&mut raw, Parser::new_ext(md, opts));

    let cleaned = ammonia::Builder::default()
        .tags(std::collections::HashSet::from([
            "h1", "h2", "h3", "p", "ul", "ol", "li", "strong", "em", "del", "a", "code", "pre",
            "blockquote", "hr", "br", "table", "thead", "tbody", "tr", "th", "td",
        ]))
        .url_schemes(std::collections::HashSet::from(["http", "https", "mailto"]))
        .clean(&raw)
        .to_string();

    inline_styles(&cleaned)
}

/// Rewrite opening tags in `html` to carry an inline `style="..."`. We do
/// this byte-level on the cleaned (well-formed) HTML — ammonia normalizes
/// attributes and quote style, so the openings we care about always look
/// like `<tagname>` or `<tagname attr="...">`. We skip closing tags
/// (`</tagname>`) by checking for the leading `/`.
fn inline_styles(html: &str) -> String {
    // (tag, style) — order doesn't matter; we match whole openings.
    const RULES: &[(&str, &str)] = &[
        ("h1", STYLE_H1),
        ("h2", STYLE_H2),
        ("h3", STYLE_H3),
        ("p", STYLE_P),
        ("ul", STYLE_LIST),
        ("ol", STYLE_LIST),
        ("li", STYLE_LI),
        ("strong", ""),
        ("em", ""),
        ("del", "text-decoration:line-through;"),
        ("a", STYLE_A),
        ("code", STYLE_CODE),
        ("pre", STYLE_PRE),
        ("blockquote", STYLE_BLOCKQUOTE),
        ("hr", STYLE_HR),
        ("table", STYLE_TABLE),
        ("thead", ""),
        ("tbody", ""),
        ("tr", ""),
        ("th", STYLE_TH),
        ("td", STYLE_TD),
    ];

    let mut out = String::with_capacity(html.len() + html.len() / 4);
    let bytes = html.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Multi-byte UTF-8 codepoints copy through as a single str slice so
        // we never split a codepoint mid-sequence (the classic mojibake bug:
        // bytes[i] as char would mis-cast 0xE2 of a curly quote to U+00E2).
        if bytes[i] != b'<' || i + 1 >= bytes.len() || bytes[i + 1] == b'/' {
            if bytes[i] < 0x80 {
                out.push(bytes[i] as char);
                i += 1;
            } else {
                // UTF-8 lead byte; copy the whole codepoint.
                let width = utf8_char_width(bytes[i]);
                out.push_str(&html[i..i + width]);
                i += width;
            }
            continue;
        }
        // Find the end of the tag.
        let Some(end_rel) = bytes[i..].iter().position(|&b| b == b'>') else {
            // Malformed; copy the rest verbatim and bail.
            out.push_str(&html[i..]);
            break;
        };
        let end = i + end_rel;
        let tag_body = &html[i + 1..end];
        // Pull the tag name (up to first space or end).
        let name_end = tag_body
            .find(|c: char| c.is_whitespace())
            .unwrap_or(tag_body.len());
        let name = &tag_body[..name_end];
        let rest = &tag_body[name_end..]; // includes leading space if any attrs

        // Skip self-closing-style "/>" by trimming a trailing slash on rest.
        let (rest_trimmed, self_close) = match rest.strip_suffix('/') {
            Some(s) => (s.trim_end(), true),
            None => (rest, false),
        };

        if let Some((_, style)) = RULES.iter().find(|(t, _)| *t == name) {
            if style.is_empty() {
                // Tag is allowlisted but we don't style it; pass through.
                out.push('<');
                out.push_str(name);
                out.push_str(rest);
                out.push('>');
            } else if rest_trimmed.trim().is_empty() {
                out.push('<');
                out.push_str(name);
                out.push_str(" style=\"");
                out.push_str(style);
                out.push('"');
                if self_close {
                    out.push_str(" /");
                }
                out.push('>');
            } else {
                // Preserve existing attrs (e.g. href on <a>), append style.
                out.push('<');
                out.push_str(name);
                out.push_str(rest_trimmed);
                out.push_str(" style=\"");
                out.push_str(style);
                out.push('"');
                if self_close {
                    out.push_str(" /");
                }
                out.push('>');
            }
        } else {
            // Not in our rule table — emit verbatim.
            out.push('<');
            out.push_str(tag_body);
            out.push('>');
        }
        i = end + 1;
    }
    out
}

/// Width in bytes of a UTF-8 codepoint given its lead byte. Mirrors the
/// std::str::utf8_char_width that's still nightly-only.
fn utf8_char_width(b: u8) -> usize {
    if b < 0x80 {
        1
    } else if b < 0xC0 {
        // Continuation byte hit as a lead; treat as 1 to make forward
        // progress (input is &str so this branch should be unreachable).
        1
    } else if b < 0xE0 {
        2
    } else if b < 0xF0 {
        3
    } else {
        4
    }
}

/// Render the `Labels: #tag1 #tag2` footer as a styled paragraph. Matches
/// the plaintext body's behavior — omitted entirely when there are no
/// labels.
fn render_labels_html(labels: &[String]) -> String {
    if labels.is_empty() {
        return String::new();
    }
    let mut s = String::new();
    s.push_str(&format!(
        "<p style=\"{p}\"><strong>Labels:</strong> ",
        p = STYLE_LABELS_P
    ));
    for (i, label) in labels.iter().enumerate() {
        if i > 0 {
            s.push(' ');
        }
        let trimmed = label.trim_start_matches('#');
        s.push('#');
        s.push_str(&escape_text(trimmed));
    }
    s.push_str("</p>\n");
    s
}

/// HTML-escape `<`, `>`, `&`, `"`, `'` for text contexts (constants we
/// drop into the template). User markdown takes the pulldown-cmark +
/// ammonia path which handles its own escaping.
fn escape_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Same as `escape_text` — kept as a separate name to signal "this value
/// goes inside an attribute" at the call site.
fn escape_attr(s: &str) -> String {
    escape_text(s)
}

// ---- Inline styles. See Phase 2.9 research notes for the rationale. ----

const STYLE_BODY: &str = "margin:0;padding:24px;background:#f6f8fa;color:#1f2328;\
                         font:14px/1.6 'Segoe UI',Helvetica,Arial,\
                         -apple-system,BlinkMacSystemFont,sans-serif;";
const STYLE_CARD: &str = "max-width:680px;margin:0 auto;background:#ffffff;padding:32px;\
                         border-radius:8px;border:1px solid #d1d9e0;";
const STYLE_H1: &str = "margin:0 0 16px;font-size:24px;line-height:1.25;font-weight:600;\
                       color:#1f2328;";
const STYLE_H2: &str = "margin:24px 0 8px;font-size:18px;line-height:1.3;font-weight:600;\
                       border-bottom:1px solid #d1d9e0;padding-bottom:4px;color:#1f2328;";
const STYLE_H3: &str = "margin:20px 0 6px;font-size:15px;line-height:1.3;font-weight:600;\
                       color:#1f2328;";
const STYLE_P: &str = "margin:0 0 12px;color:#1f2328;";
const STYLE_LIST: &str = "margin:8px 0 16px;padding-left:24px;color:#1f2328;";
const STYLE_LI: &str = "margin:4px 0;";
const STYLE_A: &str = "color:#0969da;text-decoration:underline;overflow-wrap:anywhere;";
const STYLE_CODE: &str = "background:#f6f8fa;padding:1px 4px;border-radius:3px;\
                         font:13px/1.4 ui-monospace,'SF Mono',Menlo,Consolas,monospace;\
                         color:#1f2328;";
const STYLE_PRE: &str = "background:#f6f8fa;padding:12px;border-radius:6px;margin:12px 0;\
                        white-space:pre-wrap;word-break:break-word;overflow-wrap:anywhere;\
                        font:13px/1.5 ui-monospace,'SF Mono',Menlo,Consolas,monospace;\
                        color:#1f2328;";
const STYLE_BLOCKQUOTE: &str = "margin:12px 0;padding:0 16px;border-left:3px solid #d1d9e0;\
                               color:#59636e;";
const STYLE_HR: &str = "border:0;border-top:1px solid #d1d9e0;margin:24px 0;";
const STYLE_TABLE: &str = "border-collapse:collapse;width:100%;margin:12px 0;";
const STYLE_TH: &str = "border:1px solid #d1d9e0;padding:6px 10px;text-align:left;\
                       background:#f6f8fa;font-weight:600;";
const STYLE_TD: &str = "border:1px solid #d1d9e0;padding:6px 10px;text-align:left;";
const STYLE_LABELS_P: &str = "margin:24px 0 0;color:#59636e;font-size:13px;";

#[cfg(test)]
mod tests {
    use super::*;

    fn s(
        key: &str,
        plans: &str,
        challenges: &str,
        other: &str,
        labels: &[&str],
    ) -> WeeklySummary {
        WeeklySummary {
            key_accomplishments: key.to_string(),
            plans_and_priorities: plans.to_string(),
            challenges_or_roadblocks: challenges.to_string(),
            anything_else: other.to_string(),
            labels: labels.iter().map(|x| x.to_string()).collect(),
            last_updated: None,
        }
    }

    #[test]
    fn empty_summary_renders_shell_only() {
        let out = render_body_html(&s("", "", "", "", &[]), "Week of X", None);
        assert!(out.starts_with("<!DOCTYPE html>"));
        assert!(out.contains("<meta charset=\"utf-8\">"));
        assert!(out.contains("<title>Weekly update</title>"));
        // No section heading should appear.
        assert!(!out.contains("Key accomplishments"));
        assert!(!out.contains("Plans and priorities"));
        assert!(!out.contains("Challenges or roadblocks"));
        assert!(!out.contains("Anything else"));
        assert!(!out.contains("Labels"));
    }

    #[test]
    fn markdown_table_round_trips_to_styled_table() {
        let md = "| col1 | col2 |\n|---|---|\n| a | b |";
        let out = render_body_html(&s(md, "", "", "", &[]), "Week of X", None);
        assert!(out.contains("<table"));
        assert!(out.contains("<thead"));
        assert!(out.contains("<th"));
        assert!(out.contains("col1"));
        assert!(out.contains("<tbody"));
        assert!(out.contains("<td"));
        assert!(out.contains("border-collapse:collapse"));
    }

    #[test]
    fn script_tag_is_stripped() {
        let md = "Hello <script>alert(1)</script> world";
        let out = render_body_html(&s(md, "", "", "", &[]), "Week of X", None);
        assert!(!out.contains("<script"), "ammonia must strip <script>: {out}");
        assert!(!out.contains("alert(1)"));
    }

    #[test]
    fn javascript_scheme_link_is_stripped() {
        let md = "[click](javascript:alert(1))";
        let out = render_body_html(&s(md, "", "", "", &[]), "Week of X", None);
        assert!(
            !out.contains("href=\"javascript:"),
            "javascript: scheme must not appear in any href: {out}"
        );
    }

    #[test]
    fn smart_quotes_curl_straight_quotes() {
        let md = "\"hello\"";
        let out = render_body_html(&s(md, "", "", "", &[]), "Week of X", None);
        // pulldown-cmark with ENABLE_SMART_PUNCTUATION curls "..." into “...”.
        assert!(out.contains('\u{201c}'), "expected U+201C, got: {out}");
        assert!(out.contains('\u{201d}'), "expected U+201D, got: {out}");
    }

    #[test]
    fn intro_contains_repo_link() {
        let out = render_body_html(&s("k", "", "", "", &[]), "Week of X", None);
        // Intro paragraph wraps "Captain’s Log" (curly apostrophe) in an
        // <a href="..."> pointing at the public repo. Just look for href
        // + the label text — they should be in the same paragraph.
        assert!(
            out.contains(&format!("href=\"{}\"", CAPTAINS_LOG_REPO_URL)),
            "expected repo URL as href, got: {out}"
        );
        assert!(
            out.contains("Captain\u{2019}s Log</a>"),
            "expected 'Captain’s Log' inside link text, got: {out}"
        );
    }

    #[test]
    fn manager_name_personalizes_greeting() {
        let out = render_body_html(&s("k", "", "", "", &[]), "Week of X", Some("Manny"));
        assert!(out.contains("Hello Manny,"), "got: {out}");
    }

    #[test]
    fn missing_manager_name_falls_back_to_plain_hello() {
        let out = render_body_html(&s("k", "", "", "", &[]), "Week of X", None);
        assert!(out.contains("Hello,"));
        assert!(!out.contains("Hello ,"));
    }

    #[test]
    fn whitespace_manager_name_falls_back() {
        let out = render_body_html(&s("k", "", "", "", &[]), "Week of X", Some("   "));
        assert!(out.contains("Hello,"));
    }

    #[test]
    fn ampersand_in_body_is_escaped() {
        let out = render_body_html(&s("AT&T won", "", "", "", &[]), "Week of X", None);
        assert!(out.contains("AT&amp;T"), "got: {out}");
        // The raw `&T` (without `amp;`) must not appear in the section
        // body — we check by ensuring every occurrence of `AT&` is
        // followed by `amp;`.
        for (idx, _) in out.match_indices("AT&") {
            let tail = &out[idx + 3..];
            assert!(tail.starts_with("amp;"), "bare ampersand at idx {idx}: {tail}");
        }
    }

    #[test]
    fn labels_render_as_footer() {
        let out = render_body_html(&s("k", "", "", "", &["mage", "qa"]), "Week of X", None);
        assert!(out.contains("Labels:"));
        assert!(out.contains("#mage"));
        assert!(out.contains("#qa"));
    }

    #[test]
    fn week_label_is_escaped() {
        let out = render_body_html(
            &s("k", "", "", "", &[]),
            "Week of X & Y",
            None,
        );
        assert!(out.contains("Week of X &amp; Y"), "got: {out}");
    }

    #[test]
    fn non_empty_sections_emit_h2_with_inline_style() {
        let out = render_body_html(
            &s("got stuff done", "", "", "", &[]),
            "Week of X",
            None,
        );
        assert!(
            out.contains("<h2 style=\""),
            "h2 must carry inline style, got: {out}"
        );
        assert!(out.contains("Key accomplishments"));
    }

    #[test]
    fn paragraph_carries_inline_style() {
        let out = render_body_html(&s("just a paragraph", "", "", "", &[]), "Week of X", None);
        assert!(out.contains("<p style=\""), "p must carry inline style");
    }

    #[test]
    fn writes_realistic_fixture_to_tmp() {
        // Eyeball fixture: a realistic week with mixed markdown features.
        // Path is hard-coded per slice spec so Chris can open it manually.
        let summary = WeeklySummary {
            key_accomplishments: "- Filed **MAGE-994** through **MAGE-996** for PT-BR \
                                  systemic issues\n\
                                  - Started full regression suite scaffolding\n\
                                  - Reviewed [PR #1234](https://github.com/SMARTeacher/prodigy-game/pull/1234)"
                .to_string(),
            plans_and_priorities: "1. Finish the regression suite outline\n\
                                   2. Wire the Captain's Log HTML email path\n\
                                   3. Pair with @manny on the Magicoin epic"
                .to_string(),
            challenges_or_roadblocks: "Manager Jira project was migrated mid-week; \
                                       ~~blocked~~ unblocked after the move."
                .to_string(),
            anything_else: "| Metric | This week | Last week |\n\
                            |---|---|---|\n\
                            | Bugs filed | 6 | 3 |\n\
                            | PRs reviewed | 4 | 2 |\n\
                            \n\
                            See the [Captain's Log repo](https://github.com/ChrisACarpenter/captains-log) for the running log."
                .to_string(),
            labels: vec!["mage".into(), "qa".into(), "localization".into()],
            last_updated: None,
        };
        let out = render_body_html(
            &summary,
            "week of June 22 \u{2013} June 28, 2026",
            Some("Manny"),
        );
        std::fs::write("/tmp/captainslog-preview-fixture.html", &out)
            .expect("should write fixture to /tmp");
        // Sanity checks on the fixture itself.
        assert!(out.contains("Hello Manny,"));
        assert!(out.contains("<table"));
        assert!(out.contains("MAGE-994"));
        assert!(out.contains("Labels:"));
    }
}
