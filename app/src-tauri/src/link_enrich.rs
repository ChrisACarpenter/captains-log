//! Phase 4 link enrichment.
//!
//! Fetches the HTML head of a URL and pulls out the fields the
//! frontend needs to render a link chip: `og:site_name`, `og:title`
//! (falling back to `<title>`), and the favicon (from
//! `<link rel="icon">`, falling back to `/favinion.ico`). Results are
//! cached in `.metadata/link-cache.json` keyed by the URL string.
//!
//! **Failure is a value.** Timeout, non-2xx, DNS miss, or auth-wall
//! login redirect all return an `EnrichmentResult` with every field
//! `None` except `url` + `fetched_at`. The frontend renders a hostname
//! chip in that case — no error surface. Cache entries are opaque to
//! the frontend; a cache hit that returns "all-None" still records
//! that we tried (avoids re-fetching a wall of 401s every render).
//!
//! **Pure parsing is split from HTTP.** `parse_head_metadata` takes an
//! HTML string and returns what it found; `enrich` orchestrates
//! reqwest + parse + favicon fetch + cache upsert. This is what makes
//! the parser tests hermetic — no network, no timing, no flake.
//!
//! **No curated service list.** The parse pipeline is service-agnostic
//! by design (see the Phase 4 planning conversation). GitHub, Notion,
//! YouTube, blogs, docs sites all get branded chips for free from their
//! `og:*` tags. Auth-gated Jira/Slack/Linear return the "all-None"
//! path and the user can Alt-click the chip to edit the label.

use crate::storage::{StorageBackend, StorageError, StorageResult};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use chrono::{DateTime, Utc};
use reqwest::Url;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const LINK_CACHE_FILE: &str = "link-cache.json";
pub const CURRENT_LINK_CACHE_VERSION: u32 = 1;

const HTML_FETCH_TIMEOUT: Duration = Duration::from_secs(3);
const FAVICON_FETCH_TIMEOUT: Duration = Duration::from_secs(2);
// Real-world og-rich pages are 50-500 KB. 2 MB is the ceiling before we
// bail — protects against a paste linking a huge PDF or download page
// whose response is the entire binary streamed as text/*.
const MAX_HTML_BYTES: usize = 2 * 1024 * 1024;
// Favicons are typically 1-5 KB (ICO) or 5-30 KB (PNG/SVG). 128 KB is
// generous headroom and keeps the sidecar file from ballooning on the
// occasional oversized asset.
const MAX_FAVICON_BYTES: usize = 128 * 1024;
// User-agent identifies us politely; some sites 403 anonymous requests
// that don't send one. Version bumps with the app.
const USER_AGENT: &str = concat!("CaptainsLog/", env!("CARGO_PKG_VERSION"), " (link-chip)");

/// What the frontend needs to render one chip. Every field is optional
/// so the render path can degrade to whatever's present:
///  - both title + favicon → full chip
///  - title only → text chip, generic globe icon
///  - favicon only → favicon + hostname
///  - nothing → hostname + globe (auth-gated fallback)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EnrichmentResult {
    pub url: String,
    pub title: Option<String>,
    pub site_name: Option<String>,
    /// `data:image/...;base64,...` URI. Inlined so the frontend can
    /// stick it on an `<img src>` with no extra IPC.
    pub favicon_data_url: Option<String>,
    pub fetched_at: DateTime<Utc>,
}

impl EnrichmentResult {
    pub fn empty(url: &str) -> Self {
        Self {
            url: url.to_string(),
            title: None,
            site_name: None,
            favicon_data_url: None,
            fetched_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LinkCache {
    #[serde(default = "LinkCache::current_version")]
    pub version: u32,
    #[serde(default)]
    pub entries: Vec<EnrichmentResult>,
}

impl Default for LinkCache {
    fn default() -> Self {
        Self {
            version: CURRENT_LINK_CACHE_VERSION,
            entries: Vec::new(),
        }
    }
}

impl LinkCache {
    fn current_version() -> u32 {
        CURRENT_LINK_CACHE_VERSION
    }

    pub async fn load<B: StorageBackend + ?Sized>(backend: &B) -> StorageResult<Self> {
        match backend.read_metadata(LINK_CACHE_FILE).await? {
            Some(content) => match serde_json::from_str::<LinkCache>(&content) {
                Ok(cache) => Ok(cache),
                Err(e) => {
                    eprintln!(
                        "link-cache.json failed to parse ({}). Starting with an empty cache.",
                        e
                    );
                    Ok(Self::default())
                }
            },
            None => Ok(Self::default()),
        }
    }

    pub async fn save<B: StorageBackend + ?Sized>(&self, backend: &B) -> StorageResult<()> {
        let content =
            serde_json::to_string_pretty(self).map_err(|e| StorageError::Serde(e.to_string()))?;
        backend.write_metadata(LINK_CACHE_FILE, &content).await
    }

    pub fn find(&self, url: &str) -> Option<&EnrichmentResult> {
        self.entries.iter().find(|e| e.url == url)
    }

    pub fn upsert(&mut self, entry: EnrichmentResult) {
        if let Some(idx) = self.entries.iter().position(|e| e.url == entry.url) {
            self.entries[idx] = entry;
        } else {
            self.entries.push(entry);
        }
    }
}

// ---------------------------------------------------------------------
// Pure HTML parsing — hermetic tests hit this directly, no network.
// ---------------------------------------------------------------------

/// Fields we care about pulled from a page's `<head>`. Every field is
/// optional; a page that omits `og:site_name` still yields useful data
/// via `title` alone.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HeadMetadata {
    pub title: Option<String>,
    pub site_name: Option<String>,
    /// Absolute or relative — resolved against page URL later.
    pub favicon_href: Option<String>,
}

/// Parse a page's `<head>` for chip-relevant metadata.
///
/// Preference order for title: `og:title` → `<title>`. `og:site_name`
/// stands on its own — the frontend picks between title and site_name
/// depending on chip variant.
///
/// For favicon: the first `<link rel="icon">` wins (sites publish
/// multiple sizes; the first is enough for a 16-20 px chip). If none
/// exist, callers should try `/favicon.ico` at the origin as a
/// last-ditch attempt.
pub fn parse_head_metadata(html: &str) -> HeadMetadata {
    let doc = Html::parse_document(html);

    // Selectors are unwrap()-safe — they're literals that scraper
    // parses at first use. If any of these were invalid the tests would
    // catch it. Cost: one-time parse per selector per call; acceptable
    // for a per-URL background command.
    let og_title = Selector::parse(r#"meta[property="og:title"]"#).unwrap();
    let title = Selector::parse("title").unwrap();
    let og_site = Selector::parse(r#"meta[property="og:site_name"]"#).unwrap();
    let icon = Selector::parse(r#"link[rel~="icon"]"#).unwrap();

    let extracted_title = doc
        .select(&og_title)
        .next()
        .and_then(|el| el.value().attr("content"))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            doc.select(&title)
                .next()
                .map(|el| el.text().collect::<String>().trim().to_string())
                .filter(|s| !s.is_empty())
        });

    let site_name = doc
        .select(&og_site)
        .next()
        .and_then(|el| el.value().attr("content"))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let favicon_href = doc
        .select(&icon)
        .filter_map(|el| el.value().attr("href"))
        .map(|s| s.trim().to_string())
        .find(|s| !s.is_empty());

    HeadMetadata {
        title: extracted_title,
        site_name,
        favicon_href,
    }
}

/// Resolve a favicon `href` (possibly relative) against the page URL
/// into an absolute URL. Returns None if the href is malformed or the
/// base is missing (both should be impossible in practice).
pub fn resolve_favicon_url(page_url: &str, href: &str) -> Option<String> {
    let base = Url::parse(page_url).ok()?;
    let abs = base.join(href).ok()?;
    Some(abs.to_string())
}

/// Encode arbitrary image bytes as a `data:` URI. Content type is
/// sniffed from the leading magic bytes — falls back to
/// `application/octet-stream` for unknowns (which `<img>` refuses to
/// render, correctly).
pub fn favicon_data_url(bytes: &[u8]) -> String {
    let mime = sniff_image_mime(bytes);
    let b64 = B64.encode(bytes);
    format!("data:{};base64,{}", mime, b64)
}

fn sniff_image_mime(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        "image/png"
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        "image/gif"
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "image/jpeg"
    } else if bytes.starts_with(&[0x00, 0x00, 0x01, 0x00]) || bytes.starts_with(&[0x00, 0x00, 0x02, 0x00]) {
        "image/x-icon"
    } else if bytes.len() >= 4 && &bytes[..4] == b"RIFF" && bytes.len() >= 12 && &bytes[8..12] == b"WEBP" {
        "image/webp"
    } else if bytes.starts_with(b"<svg") || bytes.starts_with(b"<?xml") {
        "image/svg+xml"
    } else {
        "application/octet-stream"
    }
}

// ---------------------------------------------------------------------
// The async enrich pipeline — HTTP fetch, parse, cache.
// ---------------------------------------------------------------------

/// Fetch and enrich a URL. Consults the cache first; on miss, runs the
/// full HTTP + parse pipeline and upserts the result.
///
/// `force_refresh = true` bypasses the cache (used by an explicit
/// "re-fetch" UI action, if one ever ships).
pub async fn enrich<B: StorageBackend + ?Sized>(
    backend: &B,
    url: &str,
    force_refresh: bool,
) -> StorageResult<EnrichmentResult> {
    // Reject anything that doesn't parse as http/https up front. Data
    // URIs, mailto:, chrome-extension:// etc. never get enriched.
    let parsed = match Url::parse(url) {
        Ok(u) if matches!(u.scheme(), "http" | "https") => u,
        _ => return Ok(EnrichmentResult::empty(url)),
    };

    let mut cache = LinkCache::load(backend).await?;
    if !force_refresh {
        if let Some(hit) = cache.find(url) {
            return Ok(hit.clone());
        }
    }

    let result = fetch_and_parse(&parsed).await;
    cache.upsert(result.clone());
    cache.save(backend).await?;
    Ok(result)
}

/// One-shot fetch + parse. Never returns an error — any failure is
/// captured as an empty `EnrichmentResult` (see module docstring).
async fn fetch_and_parse(url: &Url) -> EnrichmentResult {
    let empty = || EnrichmentResult::empty(url.as_str());

    let client = match reqwest::Client::builder()
        .timeout(HTML_FETCH_TIMEOUT)
        .user_agent(USER_AGENT)
        // Some sites redirect anonymous requests to a login page —
        // reqwest follows up to 10 hops by default which is fine. Any
        // final page with useful og-tags is a win; the login-wall case
        // just yields nothing useful.
        .build()
    {
        Ok(c) => c,
        Err(_) => return empty(),
    };

    let resp = match client.get(url.as_str()).send().await {
        Ok(r) => r,
        Err(_) => return empty(),
    };

    if !resp.status().is_success() {
        return empty();
    }

    // Cap the download at MAX_HTML_BYTES. `bytes()` reads to memory in
    // one shot; the Content-Length check is defense in depth (some
    // servers under-report). We accept truncation over OOM.
    if let Some(len) = resp.content_length() {
        if len as usize > MAX_HTML_BYTES {
            return empty();
        }
    }
    let bytes = match resp.bytes().await {
        Ok(b) if b.len() <= MAX_HTML_BYTES => b,
        _ => return empty(),
    };
    let html = String::from_utf8_lossy(&bytes);

    let head = parse_head_metadata(&html);

    // Favicon fetch is best-effort — a failed favicon shouldn't sink
    // the whole enrichment. Do it AFTER we've committed to returning
    // something so a slow icon server can't stall the title.
    let favicon_data_url = match resolve_favicon_href(url, head.favicon_href.as_deref()) {
        Some(icon_url) => fetch_favicon(&client, &icon_url).await,
        None => None,
    };

    EnrichmentResult {
        url: url.to_string(),
        title: head.title,
        site_name: head.site_name,
        favicon_data_url,
        fetched_at: Utc::now(),
    }
}

/// Given the page URL and a `<link rel="icon">` href (if any), return
/// the absolute favicon URL to fetch. Falls back to `/favicon.ico` at
/// the origin when the page didn't advertise one — that's the pre-HTML5
/// convention every browser still honors and most sites still serve.
fn resolve_favicon_href(page_url: &Url, href: Option<&str>) -> Option<Url> {
    match href {
        Some(h) => page_url.join(h).ok(),
        None => page_url.join("/favicon.ico").ok(),
    }
}

async fn fetch_favicon(client: &reqwest::Client, url: &Url) -> Option<String> {
    let resp = client
        .get(url.as_str())
        .timeout(FAVICON_FETCH_TIMEOUT)
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    if let Some(len) = resp.content_length() {
        if len as usize > MAX_FAVICON_BYTES {
            return None;
        }
    }
    let bytes = resp.bytes().await.ok()?;
    if bytes.len() > MAX_FAVICON_BYTES || bytes.is_empty() {
        return None;
    }
    Some(favicon_data_url(&bytes))
}

// ---------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_og_title_preferred_over_title_tag() {
        let html = r#"<!DOCTYPE html><html><head>
            <title>Old raw title</title>
            <meta property="og:title" content="Preferred OG Title" />
        </head><body></body></html>"#;
        let meta = parse_head_metadata(html);
        assert_eq!(meta.title.as_deref(), Some("Preferred OG Title"));
    }

    #[test]
    fn falls_back_to_title_tag_when_og_title_absent() {
        let html = r#"<!DOCTYPE html><html><head>
            <title>Just A Title</title>
        </head><body></body></html>"#;
        let meta = parse_head_metadata(html);
        assert_eq!(meta.title.as_deref(), Some("Just A Title"));
    }

    #[test]
    fn extracts_og_site_name_when_present() {
        let html = r#"<html><head>
            <meta property="og:site_name" content="GitHub" />
            <meta property="og:title" content="pull/123" />
        </head></html>"#;
        let meta = parse_head_metadata(html);
        assert_eq!(meta.site_name.as_deref(), Some("GitHub"));
        assert_eq!(meta.title.as_deref(), Some("pull/123"));
    }

    #[test]
    fn returns_none_when_no_head_metadata_present() {
        let html = "<html><head></head><body>hi</body></html>";
        let meta = parse_head_metadata(html);
        assert_eq!(meta, HeadMetadata::default());
    }

    #[test]
    fn empty_content_attrs_are_filtered_out() {
        // Some pages emit `<meta property="og:title" content="" />` — treat as absent.
        let html = r#"<html><head>
            <meta property="og:title" content="" />
            <title>Fallback</title>
        </head></html>"#;
        let meta = parse_head_metadata(html);
        assert_eq!(meta.title.as_deref(), Some("Fallback"));
    }

    #[test]
    fn extracts_favicon_href_from_link_rel_icon() {
        let html = r#"<html><head>
            <link rel="icon" href="/favicon.ico" />
        </head></html>"#;
        let meta = parse_head_metadata(html);
        assert_eq!(meta.favicon_href.as_deref(), Some("/favicon.ico"));
    }

    #[test]
    fn extracts_favicon_href_from_shortcut_icon() {
        // rel~="icon" attribute selector matches "shortcut icon" too.
        let html = r#"<html><head>
            <link rel="shortcut icon" href="/legacy-favicon.ico" />
        </head></html>"#;
        let meta = parse_head_metadata(html);
        assert_eq!(meta.favicon_href.as_deref(), Some("/legacy-favicon.ico"));
    }

    #[test]
    fn takes_first_icon_when_multiple_present() {
        let html = r#"<html><head>
            <link rel="icon" href="/16.png" />
            <link rel="icon" href="/32.png" />
            <link rel="apple-touch-icon" href="/apple.png" />
        </head></html>"#;
        let meta = parse_head_metadata(html);
        assert_eq!(meta.favicon_href.as_deref(), Some("/16.png"));
    }

    #[test]
    fn resolves_relative_favicon_against_page_url() {
        let abs = resolve_favicon_url("https://example.com/blog/post", "/favicon.ico");
        assert_eq!(abs.as_deref(), Some("https://example.com/favicon.ico"));
    }

    #[test]
    fn resolves_absolute_favicon_url_passthrough() {
        let abs = resolve_favicon_url("https://example.com/x", "https://cdn.example.com/f.png");
        assert_eq!(abs.as_deref(), Some("https://cdn.example.com/f.png"));
    }

    #[test]
    fn sniffs_png_signature() {
        let png = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        assert_eq!(sniff_image_mime(&png), "image/png");
    }

    #[test]
    fn sniffs_ico_signature() {
        let ico = [0x00, 0x00, 0x01, 0x00, 0x01];
        assert_eq!(sniff_image_mime(&ico), "image/x-icon");
    }

    #[test]
    fn sniffs_svg_signature() {
        assert_eq!(sniff_image_mime(b"<svg xmlns=\"...\">"), "image/svg+xml");
    }

    #[test]
    fn favicon_data_url_wraps_bytes_with_correct_mime() {
        let png = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        let data_url = favicon_data_url(&png);
        assert!(data_url.starts_with("data:image/png;base64,"));
    }

    #[test]
    fn link_cache_default_carries_current_version() {
        let c = LinkCache::default();
        assert_eq!(c.version, CURRENT_LINK_CACHE_VERSION);
        assert!(c.entries.is_empty());
    }

    #[test]
    fn link_cache_find_returns_matching_entry() {
        let mut cache = LinkCache::default();
        cache.upsert(EnrichmentResult {
            url: "https://a.example/1".into(),
            title: Some("A".into()),
            site_name: None,
            favicon_data_url: None,
            fetched_at: Utc::now(),
        });
        assert!(cache.find("https://a.example/1").is_some());
        assert!(cache.find("https://a.example/2").is_none());
    }

    #[test]
    fn link_cache_upsert_replaces_existing_entry() {
        let mut cache = LinkCache::default();
        cache.upsert(EnrichmentResult {
            url: "https://x/1".into(),
            title: Some("v1".into()),
            site_name: None,
            favicon_data_url: None,
            fetched_at: Utc::now(),
        });
        cache.upsert(EnrichmentResult {
            url: "https://x/1".into(),
            title: Some("v2".into()),
            site_name: None,
            favicon_data_url: None,
            fetched_at: Utc::now(),
        });
        assert_eq!(cache.entries.len(), 1);
        assert_eq!(cache.entries[0].title.as_deref(), Some("v2"));
    }

    #[tokio::test]
    async fn enrich_returns_empty_for_non_http_url() {
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        let result = enrich(&backend, "mailto:someone@example.com", false).await.unwrap();
        assert!(result.title.is_none());
        assert!(result.site_name.is_none());
        assert!(result.favicon_data_url.is_none());
    }

    #[tokio::test]
    async fn enrich_returns_empty_for_data_uri() {
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        let result = enrich(&backend, "data:text/plain,hello", false).await.unwrap();
        assert!(result.title.is_none());
    }

    #[tokio::test]
    async fn cache_load_corrupt_file_returns_default() {
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        backend
            .write_metadata(LINK_CACHE_FILE, "{ not: valid json")
            .await
            .unwrap();
        let loaded = LinkCache::load(&backend).await.unwrap();
        assert_eq!(loaded.version, CURRENT_LINK_CACHE_VERSION);
        assert!(loaded.entries.is_empty());
    }

    #[tokio::test]
    async fn cache_load_missing_file_returns_default() {
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        let loaded = LinkCache::load(&backend).await.unwrap();
        assert_eq!(loaded.version, CURRENT_LINK_CACHE_VERSION);
        assert!(loaded.entries.is_empty());
    }

    #[tokio::test]
    async fn cache_save_load_roundtrip_preserves_entries() {
        use crate::storage::LocalFilesystem;
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        let mut cache = LinkCache::default();
        cache.upsert(EnrichmentResult {
            url: "https://example.com/foo".into(),
            title: Some("Foo".into()),
            site_name: Some("Example".into()),
            favicon_data_url: Some("data:image/png;base64,AAAA".into()),
            fetched_at: Utc::now(),
        });
        cache.save(&backend).await.unwrap();
        let reloaded = LinkCache::load(&backend).await.unwrap();
        assert_eq!(reloaded.entries.len(), 1);
        assert_eq!(reloaded.entries[0].url, "https://example.com/foo");
        assert_eq!(reloaded.entries[0].title.as_deref(), Some("Foo"));
        assert_eq!(reloaded.entries[0].site_name.as_deref(), Some("Example"));
    }
}
