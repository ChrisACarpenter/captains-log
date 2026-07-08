//! Settings: two-tier persistence for app and journal preferences.
//!
//! ## Why two files?
//!
//! - **App-level** (`AppSettings`) lives in the OS application support directory
//!   (`~/Library/Application Support/com.prodigygame.captainslog/app-settings.json`
//!   on macOS). It holds **per-machine** state — currently just where the journal
//!   data lives. This file never syncs.
//!
//! - **Journal-level** (`JournalSettings`) lives in `<journal_root>/.metadata/settings.json`.
//!   It holds **journal-specific** state — the user's display name and reminder
//!   preferences. This file travels with the journal (so Google Drive sync in
//!   Phase 6 carries it across machines, while machine-specific config stays per-machine).
//!
//! ## First-run detection
//!
//! "First run" is defined as: `app-settings.json` does not exist on disk.
//! The first-run wizard writes both files, after which the app boots straight
//! into normal mode.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::storage::{StorageBackend, StorageError, StorageResult};

pub const APP_SETTINGS_FILENAME: &str = "app-settings.json";
pub const JOURNAL_SETTINGS_FILENAME: &str = "settings.json";
pub const CURRENT_VERSION: u32 = 1;

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("i/o error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("serialization error: {0}")]
    Serde(String),
}

pub type SettingsResult<T> = Result<T, SettingsError>;

// ---------------------------------------------------------------------------
// AppSettings — per-machine
// ---------------------------------------------------------------------------

/// Theme preference, persisted in `AppSettings`.
///
/// Dark is the default and matches `:root` in `app.css`. Light is opt-in
/// via the settings panel and applied by setting `data-theme="light"` on
/// the `<html>` element. Custom (Phase 2.8) drives a user-derived OKLCH
/// token map; its payload lives in `AppSettings::custom_theme`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Dark,
    Light,
    Custom,
}

impl Default for Theme {
    fn default() -> Self {
        Theme::Dark
    }
}

/// User-editable primary tokens for the Custom theme (Phase 2.8).
///
/// 12 tokens (3 bg, 3 text, 2 borders, 4 accents). Every field is a 6-digit
/// hex color (`#rrggbb`). The derivation engine in `app/src/lib/theme.ts`
/// expands these into the full ~30-token CSS variable map at apply-time;
/// only the primaries are persisted because everything downstream is a
/// pure function of them.
///
/// Field shape mirrors the TypeScript `PrimaryTokens` type. Wire format is
/// camelCase (`bgBase`, `accentPrimary`, …) so the Tauri serde boundary is
/// transparent to the frontend.
///
/// Validation: each value is checked against `^#[0-9a-fA-F]{6}$` on
/// deserialize. Malformed input is rejected with a clear error rather
/// than silently coerced — a bad hex string in the wire payload almost
/// always means the frontend bug needs fixing, not a fallback applied.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CustomTheme {
    #[serde(deserialize_with = "deserialize_hex6")]
    pub bg_base: String,
    #[serde(deserialize_with = "deserialize_hex6")]
    pub bg_surface: String,
    #[serde(deserialize_with = "deserialize_hex6")]
    pub bg_elevated: String,
    #[serde(deserialize_with = "deserialize_hex6")]
    pub text_primary: String,
    #[serde(deserialize_with = "deserialize_hex6")]
    pub text_secondary: String,
    #[serde(deserialize_with = "deserialize_hex6")]
    pub text_muted: String,
    #[serde(deserialize_with = "deserialize_hex6")]
    pub border_structural: String,
    #[serde(deserialize_with = "deserialize_hex6")]
    pub border_decorative: String,
    #[serde(deserialize_with = "deserialize_hex6")]
    pub accent_primary: String,
    #[serde(deserialize_with = "deserialize_hex6")]
    pub accent_green: String,
    #[serde(deserialize_with = "deserialize_hex6")]
    pub accent_pink: String,
    #[serde(deserialize_with = "deserialize_hex6")]
    pub btn_sapphire: String,
}

/// Strict 6-digit hex validator. Accepts `#rrggbb` (case-insensitive),
/// rejects everything else with a descriptive error. Used by `CustomTheme`
/// fields so a bad payload fails fast at the serde boundary rather than
/// leaking into the derivation engine where the failure mode would be a
/// silent blank theme.
///
/// `pub(crate)` so siblings in the crate (notably `labels::LabelEntry`,
/// which validates the per-label color override) can reuse the same
/// canonical validator instead of growing a second copy that drifts.
pub(crate) fn deserialize_hex6<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let s: String = serde::Deserialize::deserialize(deserializer)?;
    if is_hex6(&s) {
        // Normalize to lowercase so on-disk round-trips are stable
        // regardless of which case the frontend sent.
        Ok(s.to_ascii_lowercase())
    } else {
        Err(D::Error::custom(format!(
            "expected 6-digit hex color like #rrggbb, got {s:?}"
        )))
    }
}

/// Option-aware variant of [`deserialize_hex6`]. `None` (or a JSON `null`)
/// stays as `None` — useful for optional color overrides where "not set"
/// is the default. A present string runs through the same strict
/// validator; anything malformed errors at the serde boundary.
pub(crate) fn deserialize_hex6_option<'de, D>(
    deserializer: D,
) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let opt: Option<String> = serde::Deserialize::deserialize(deserializer)?;
    match opt {
        None => Ok(None),
        Some(s) => {
            if is_hex6(&s) {
                Ok(Some(s.to_ascii_lowercase()))
            } else {
                Err(D::Error::custom(format!(
                    "expected 6-digit hex color like #rrggbb, got {s:?}"
                )))
            }
        }
    }
}

fn is_hex6(s: &str) -> bool {
    let bytes = s.as_bytes();
    bytes.len() == 7
        && bytes[0] == b'#'
        && bytes[1..].iter().all(|b| b.is_ascii_hexdigit())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub version: u32,
    pub journal_root: PathBuf,

    /// Theme preference. Optional in the JSON so older app-settings.json files
    /// (written before this field existed) still parse — they get the default.
    #[serde(default)]
    pub theme: Theme,

    /// User's Custom theme primaries (Phase 2.8). Persists independently of
    /// the active `theme` choice: switching to Light/Dark must NOT clobber
    /// the saved Custom payload — the user expects "switch back to Custom"
    /// to restore their colors without re-entering all 12 tokens.
    ///
    /// `serde(default)` so older app-settings.json files (pre-2.8) load
    /// cleanly as `None`. When `theme == Theme::Custom`, this MUST be
    /// `Some(...)` — that invariant is enforced by `update_settings`,
    /// not by the type (Option keeps the wire format clean).
    #[serde(default)]
    pub custom_theme: Option<CustomTheme>,
}

impl AppSettings {
    fn path_in(app_data_dir: &Path) -> PathBuf {
        app_data_dir.join(APP_SETTINGS_FILENAME)
    }

    /// Load app settings if present. Returns `Ok(None)` if the file doesn't
    /// exist — that's the signal that the app is in first-run state.
    pub async fn load(app_data_dir: &Path) -> SettingsResult<Option<Self>> {
        let path = Self::path_in(app_data_dir);
        match tokio::fs::read_to_string(&path).await {
            Ok(content) => serde_json::from_str(&content)
                .map(Some)
                .map_err(|e| SettingsError::Serde(e.to_string())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(SettingsError::Io { path, source: e }),
        }
    }

    pub async fn save(&self, app_data_dir: &Path) -> SettingsResult<()> {
        tokio::fs::create_dir_all(app_data_dir)
            .await
            .map_err(|e| SettingsError::Io {
                path: app_data_dir.to_path_buf(),
                source: e,
            })?;
        let path = Self::path_in(app_data_dir);
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| SettingsError::Serde(e.to_string()))?;
        tokio::fs::write(&path, content)
            .await
            .map_err(|e| SettingsError::Io { path, source: e })
    }
}

// ---------------------------------------------------------------------------
// JournalSettings — per-journal
// ---------------------------------------------------------------------------

/// Custom deserializer that accepts BOTH the new `daysOfWeek: [u8, …]`
/// shape AND the legacy `dayOfWeek: u8` shape (written by Captain's Log
/// builds before Phase 2.7's multi-day reminder support landed).
///
/// On a successful load:
///   - `daysOfWeek` is present  → used verbatim (deduped + clamped 0..=6).
///   - `dayOfWeek` is present   → wrapped in a single-element vec.
///   - Both present             → `daysOfWeek` wins (forward-compat).
///   - Neither present          → empty vec (a no-op reminder until the
///                                user configures days in /settings).
fn deserialize_days_of_week<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let raw: serde_json::Value = serde::Deserialize::deserialize(deserializer)?;
    if !raw.is_array() {
        return Err(D::Error::custom("daysOfWeek must be an array of u8"));
    }
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for v in raw.as_array().unwrap() {
        let n = v
            .as_u64()
            .ok_or_else(|| D::Error::custom("daysOfWeek entries must be unsigned integers"))?;
        if n > 6 {
            // Clamp out-of-range silently — handle hand-edited JSON cases
            // gracefully rather than refusing to load the whole settings file.
            continue;
        }
        let n = n as u8;
        if seen.insert(n) {
            out.push(n);
        }
    }
    out.sort_unstable();
    Ok(out)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", from = "ReminderSettingsRaw")]
pub struct ReminderSettings {
    pub enabled: bool,
    /// 0 = Monday … 6 = Sunday (ISO weekday convention).
    /// Sorted ascending and deduped on every read.
    /// Empty vec ≡ reminder is configured-but-has-no-days (no-op fire).
    pub days_of_week: Vec<u8>,
    pub hour: u8,
    pub minute: u8,
}

/// On-disk raw shape used purely for deserialization back-compat. The
/// production struct converts FROM this on every load via
/// `#[serde(from = "ReminderSettingsRaw")]` on `ReminderSettings`.
///
/// Reads both:
///   - `daysOfWeek: [u8]` (current shape, written by Phase 2.7+ code)
///   - `dayOfWeek: u8`    (legacy shape, written before Phase 2.7)
///
/// The legacy field can't simply be aliased to the new one because the
/// types differ (u8 vs Vec<u8>); a wrapper struct + From impl is the
/// cleanest serde-friendly bridge. Writes always go through the
/// production struct which only emits `daysOfWeek`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReminderSettingsRaw {
    #[serde(default = "default_enabled")]
    enabled: bool,
    #[serde(default, deserialize_with = "deserialize_days_of_week_opt")]
    days_of_week: Option<Vec<u8>>,
    /// Legacy field. When present and `daysOfWeek` is absent, becomes the
    /// single element of `days_of_week`.
    #[serde(default)]
    day_of_week: Option<u8>,
    #[serde(default = "default_hour")]
    hour: u8,
    #[serde(default)]
    minute: u8,
}

fn default_enabled() -> bool {
    false
}
fn default_hour() -> u8 {
    16
}

fn deserialize_days_of_week_opt<'de, D>(deserializer: D) -> Result<Option<Vec<u8>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    // Lets the field truly be `None` when absent vs `Some(empty)` when
    // present-but-empty — important for the dayOfWeek fallback logic.
    Ok(Some(deserialize_days_of_week(deserializer)?))
}

impl From<ReminderSettingsRaw> for ReminderSettings {
    fn from(raw: ReminderSettingsRaw) -> Self {
        // The Option<Vec<u8>> distinction is load-bearing here: a
        // present-but-empty `daysOfWeek: []` is the user's explicit
        // intent ("no days configured") and must win over any legacy
        // dayOfWeek that might co-exist in the JSON. Only when
        // daysOfWeek is genuinely absent do we promote the legacy
        // dayOfWeek into a single-element vec. Earlier shape collapsed
        // both states via .unwrap_or_default() + .is_empty() and
        // silently overrode `[]` with the legacy value.
        let days = match raw.days_of_week {
            Some(v) => v,
            None => raw
                .day_of_week
                .filter(|d| *d <= 6)
                .map(|d| vec![d])
                .unwrap_or_default(),
        };
        Self {
            enabled: raw.enabled,
            days_of_week: days,
            hour: raw.hour,
            minute: raw.minute,
        }
    }
}

impl Default for ReminderSettings {
    fn default() -> Self {
        // Friday at 4pm — the end-of-week reflection slot recommended in docs.
        Self {
            enabled: false,
            days_of_week: vec![4],
            hour: 16,
            minute: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Mail-send preferences (Phase 2.9b)
// ---------------------------------------------------------------------------
//
// Three send paths the user can choose between for "Send to manager":
//   - Gmail        — opens a Gmail compose URL in the browser (default)
//   - NativeMail   — drives Mac Mail.app via AppleScript (osascript)
//   - Outlook      — opens Outlook web compose (Business or Personal)
//
// All three travel through compose_weekly_email and the preview modal in
// later slices; this slice just stores the preferences.

/// Which send path the "Send to manager" button uses. Gmail is the
/// recommended default because it doesn't need accessibility permissions
/// and works on any machine where the user has a Gmail tab open.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum MailSendMode {
    Gmail,
    NativeMail,
    Outlook,
}

impl Default for MailSendMode {
    fn default() -> Self {
        MailSendMode::Gmail
    }
}

/// Body format used by Gmail and Outlook (which only support plaintext),
/// and as the non-HTML option for Native Mac Mail. CleanText strips
/// markdown markers for a manager-readable plaintext message;
/// MarkdownSource preserves the raw `**bold**` / `- bullet` markup for
/// users who want the source visible in their sent folder.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum MailBodyFormat {
    CleanText,
    MarkdownSource,
}

impl Default for MailBodyFormat {
    fn default() -> Self {
        MailBodyFormat::CleanText
    }
}

/// Outlook web has two distinct hosts depending on whether the account is
/// a Microsoft 365 work/school tenant or a consumer outlook.com address.
/// We can't autodetect from the user_email reliably (custom domains route
/// through 365), so let the user pick.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum OutlookFlavor {
    Business,
    Personal,
}

impl Default for OutlookFlavor {
    fn default() -> Self {
        OutlookFlavor::Business
    }
}

/// How the Send button delivers the body to the chosen mail client.
///
/// - `Prefilled`: body is rendered to plaintext and embedded in the
///   Gmail/Outlook URL or AppleScript `content:` property. Current
///   behavior. Fastest (one click) but the recipient sees the body
///   without rich formatting.
/// - `ClipboardPaste`: an empty draft is opened in the chosen client
///   (Gmail / Outlook web / Mac Mail) and the rich-HTML version of the
///   body is written to the system clipboard. The user presses Cmd+V
///   in the compose body to paste, then Send. One extra keystroke,
///   but the recipient gets a fully formatted message regardless of
///   which client we're routing through.
///
/// This setting is GLOBAL (orthogonal to send mode) — picking it once
/// applies to all three clients. The Native Mac "styled HTML .eml"
/// toggle is an independent peer override on top of this choice.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum MailBodyDelivery {
    Prefilled,
    ClipboardPaste,
}

impl Default for MailBodyDelivery {
    fn default() -> Self {
        MailBodyDelivery::Prefilled
    }
}

/// User-tunable display preferences for the landing-page task list.
/// Wired into `list_tasks` on the frontend — the backend command
/// itself returns every task in the current week's Plans body; these
/// flags only affect what the UI shows and how it orders them.
///
/// Defaults are picked so an upgrade from a pre-Slice-4 settings.json
/// doesn't surprise the user: `show_completed = true` (nothing
/// disappears), `open_tasks_first = true` (the natural read order),
/// `show_completed_timestamp = false` (extra chrome only if the user
/// opts in).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TaskListSettings {
    /// When false, the landing-page task list hides `[x]` rows.
    pub show_completed: bool,
    /// When true, open tasks are sorted above completed ones. When
    /// false, tasks appear in source-file order (matches the
    /// Plans-and-priorities section as written).
    pub open_tasks_first: bool,
    /// When true, completed rows render a relative-time label
    /// ("checked 2h ago"). Uses the `completed_at` sidecar value.
    /// Off by default to keep the list visually tight.
    pub show_completed_timestamp: bool,
    /// When true, the entire task-list section is hidden from the
    /// landing page. Escape hatch for users who don't use the
    /// feature — everything else in the app keeps working.
    #[serde(default)]
    pub hide_task_list: bool,
    /// Phase 3c Slice 5 — when true, incomplete tasks from the
    /// previous ISO week are automatically copied forward into the
    /// current week's Plans section on landing-page mount / focus /
    /// week-transition. When false, no rollover ever fires; users
    /// manage their carry-forward manually via the summary editor.
    /// Default on: rollover is the whole point of the feature; off is
    /// a kill switch for anyone who prefers a fresh slate each week.
    #[serde(default = "default_true")]
    pub auto_rollover_enabled: bool,
}

/// Serde-default helper: `#[serde(default)]` uses `Default::default()`
/// on the field's type, which for `bool` is `false`. `auto_rollover`
/// wants `true` as its missing-field fallback so upgrading from a
/// pre-Slice-5 settings.json turns rollover ON, matching the fresh-
/// install default.
fn default_true() -> bool {
    true
}

impl Default for TaskListSettings {
    fn default() -> Self {
        Self {
            show_completed: true,
            open_tasks_first: true,
            show_completed_timestamp: false,
            hide_task_list: false,
            auto_rollover_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JournalSettings {
    pub version: u32,
    pub user_name: Option<String>,
    /// The user's own email address. In Gmail mode it pins the
    /// `/mail/u/{address}` slot so multi-account users always land in the
    /// right Gmail; in Native Mac Mail mode it's set as the outgoing
    /// `sender` so the message goes from the right account. Never shown
    /// to the manager. Optional. `#[serde(default)]` so older
    /// settings.json files written before this field existed still parse.
    #[serde(default)]
    pub user_email: Option<String>,
    pub reminder: ReminderSettings,
    /// Manager's email address — used by the "Send weekly summary to manager"
    /// flow on /summary. Optional; an empty/missing value just disables the
    /// Send button. `#[serde(default)]` so older settings.json files written
    /// before this field existed still parse cleanly.
    #[serde(default)]
    pub manager_email: Option<String>,
    /// Manager's first name (or whatever the user prefers to address them as).
    /// Used purely to personalize the email greeting ("Hello {name},"); the
    /// send still works without it (greeting falls back to a plain "Hello,").
    /// Kept separate from manager_email because the email is the routing
    /// information and the name is presentation.
    #[serde(default)]
    pub manager_name: Option<String>,
    /// Job title as it appears in BambooHR — surfaced in the eventual Send-to-
    /// Manager email signature and reused by other "who am I" places that
    /// want a one-line role descriptor. Optional. `#[serde(default)]` so
    /// older settings.json files parse cleanly.
    #[serde(default)]
    pub bamboo_title: Option<String>,
    /// Jira project keys the user is associated with (e.g. `["MAGE", "LIVE"]`).
    /// Stored as a Vec of all-caps tokens; the wizard accepts a comma-
    /// separated string and the backend normalizes (upper-case, strip
    /// whitespace, drop empties). Optional, defaults to empty vec.
    #[serde(default)]
    pub jira_project_keys: Vec<String>,
    /// Which Send-to-manager path to use. Defaults to Gmail.
    #[serde(default)]
    pub mail_send_mode: MailSendMode,
    /// Body format used when the chosen send mode emits a plaintext body
    /// (Gmail and Outlook always; Native Mac Mail when `mail_native_html`
    /// is false). Defaults to CleanText.
    #[serde(default)]
    pub mail_body_format: MailBodyFormat,
    /// Native Mac Mail only — when true, AppleScript drives a styled
    /// rich-HTML message instead of plaintext. Ignored in Gmail/Outlook
    /// modes (their web compose endpoints don't accept HTML bodies).
    #[serde(default)]
    pub mail_native_html: bool,
    /// Outlook only — Business (Microsoft 365) vs Personal (outlook.com).
    /// Picks which compose host the URL points at.
    #[serde(default)]
    pub mail_outlook_flavor: OutlookFlavor,
    /// How the body reaches the compose window — embedded in the URL
    /// as plaintext (Prefilled, default) or written to the clipboard
    /// for the user to paste with Cmd+V (ClipboardPaste). Applies to
    /// all three send modes.
    #[serde(default)]
    pub mail_body_delivery: MailBodyDelivery,
    /// Phase 2.8+ "Colorful Labels": when true, label chips render with
    /// their per-label color (either the persisted hex override on the
    /// `LabelEntry` or a hash-derived fallback) instead of the flat
    /// theme accent. When false, persisted hex overrides remain on disk
    /// untouched — the renderer just ignores them. Default false so an
    /// existing journal upgrades without a visual surprise; the user
    /// opts in from Settings > Theme.
    #[serde(default)]
    pub colorful_labels: bool,
    /// Phase 3c Slice 4 — display preferences for the landing-page
    /// task list. `#[serde(default)]` so a pre-Slice-4 settings.json
    /// upgrades cleanly with the defaults documented on
    /// [`TaskListSettings`].
    #[serde(default)]
    pub task_list: TaskListSettings,
}

impl Default for JournalSettings {
    fn default() -> Self {
        Self {
            version: CURRENT_VERSION,
            user_name: None,
            user_email: None,
            reminder: ReminderSettings::default(),
            manager_email: None,
            manager_name: None,
            bamboo_title: None,
            jira_project_keys: Vec::new(),
            mail_send_mode: MailSendMode::default(),
            mail_body_format: MailBodyFormat::default(),
            mail_native_html: false,
            mail_outlook_flavor: OutlookFlavor::default(),
            mail_body_delivery: MailBodyDelivery::default(),
            colorful_labels: false,
            task_list: TaskListSettings::default(),
        }
    }
}

impl JournalSettings {
    /// Load journal settings. Returns the default if the file doesn't exist.
    pub async fn load<B: StorageBackend + ?Sized>(backend: &B) -> StorageResult<Self> {
        match backend.read_metadata(JOURNAL_SETTINGS_FILENAME).await? {
            Some(content) => serde_json::from_str(&content)
                .map_err(|e| StorageError::Serde(e.to_string())),
            None => Ok(Self::default()),
        }
    }

    pub async fn save<B: StorageBackend + ?Sized>(&self, backend: &B) -> StorageResult<()> {
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| StorageError::Serde(e.to_string()))?;
        backend
            .write_metadata(JOURNAL_SETTINGS_FILENAME, &content)
            .await
    }
}

// ---------------------------------------------------------------------------
// Default journal_root
// ---------------------------------------------------------------------------

/// `~/Documents/CaptainsLog/` on macOS/Linux; `%USERPROFILE%/Documents/CaptainsLog/`
/// on Windows. Used as the suggested journal root in the first-run picker.
pub fn default_journal_root() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join("Documents").join("CaptainsLog")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::LocalFilesystem;
    use tempfile::TempDir;

    #[tokio::test]
    async fn app_settings_missing_returns_none() {
        let dir = TempDir::new().unwrap();
        let loaded = AppSettings::load(dir.path()).await.unwrap();
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn app_settings_roundtrip() {
        let dir = TempDir::new().unwrap();
        // Use a subdir to verify create_dir_all is called.
        let app_dir = dir.path().join("nested/app-data");

        let original = AppSettings {
            version: CURRENT_VERSION,
            journal_root: PathBuf::from("/Users/test/MyJournal"),
            theme: Theme::Light,
            custom_theme: None,
        };
        original.save(&app_dir).await.unwrap();

        let loaded = AppSettings::load(&app_dir).await.unwrap().unwrap();
        assert_eq!(loaded.version, original.version);
        assert_eq!(loaded.journal_root, original.journal_root);
        assert_eq!(loaded.theme, Theme::Light);
        assert!(loaded.custom_theme.is_none());
    }

    #[tokio::test]
    async fn app_settings_legacy_without_theme_defaults_to_dark() {
        // Simulate an app-settings.json written before the theme field existed
        // (e.g. anyone who ran the wizard on yesterday's build). Serde's
        // #[default] should fill it in transparently.
        let dir = TempDir::new().unwrap();
        let app_dir = dir.path();
        let legacy_json = r#"{
          "version": 1,
          "journalRoot": "/Users/test/MyJournal"
        }"#;
        tokio::fs::write(app_dir.join(APP_SETTINGS_FILENAME), legacy_json)
            .await
            .unwrap();

        let loaded = AppSettings::load(app_dir).await.unwrap().unwrap();
        assert_eq!(loaded.theme, Theme::Dark);
    }

    #[test]
    fn theme_serializes_lowercase() {
        let json = serde_json::to_string(&Theme::Dark).unwrap();
        assert_eq!(json, r#""dark""#);
        let json = serde_json::to_string(&Theme::Light).unwrap();
        assert_eq!(json, r#""light""#);
        let json = serde_json::to_string(&Theme::Custom).unwrap();
        assert_eq!(json, r#""custom""#);
    }

    // ---------- Phase 2.8 Custom theme persistence ----------

    /// 12-token fixture mirroring SHIPPING_DARK_PRIMARIES from theme.ts —
    /// makes the round-trip assertion concrete and forces the field order
    /// to stay in lockstep with the frontend type.
    fn fixture_custom_theme() -> CustomTheme {
        CustomTheme {
            bg_base: "#2b2420".to_string(),
            bg_surface: "#36302c".to_string(),
            bg_elevated: "#3d3936".to_string(),
            text_primary: "#f6e7d7".to_string(),
            text_secondary: "#d2b094".to_string(),
            text_muted: "#a89784".to_string(),
            // border_* are user-facing solid colors in this slice;
            // rgba() wrapping is a derivation-engine concern, not persistence.
            border_structural: "#241e1a".to_string(),
            border_decorative: "#4a2418".to_string(),
            accent_primary: "#ff5c08".to_string(),
            accent_green: "#95c13b".to_string(),
            accent_pink: "#eb018b".to_string(),
            btn_sapphire: "#3a82c8".to_string(),
        }
    }

    #[test]
    fn app_settings_default_via_legacy_load_is_light_and_no_custom() {
        // AppSettings has no Default impl (journal_root has no sensible
        // default — first-run decides). Instead pin the "fresh install"
        // shape via the legacy-load path: a minimal JSON with neither
        // theme nor customTheme should parse with theme = default (Dark)
        // and custom_theme = None. This is the spec the wizard relies on.
        let json = r#"{
          "version": 1,
          "journalRoot": "/tmp/x"
        }"#;
        let loaded: AppSettings = serde_json::from_str(json).unwrap();
        assert_eq!(loaded.theme, Theme::Dark);
        assert!(loaded.custom_theme.is_none());
    }

    #[tokio::test]
    async fn app_settings_custom_theme_round_trip() {
        // Save with Theme::Custom + a full 12-token payload, reload, and
        // verify every field comes back identically. Catches accidental
        // serde rename mismatches between snake_case Rust fields and the
        // camelCase wire format the frontend uses.
        let dir = TempDir::new().unwrap();
        let app_dir = dir.path();
        let custom = fixture_custom_theme();
        let original = AppSettings {
            version: CURRENT_VERSION,
            journal_root: PathBuf::from("/Users/test/MyJournal"),
            theme: Theme::Custom,
            custom_theme: Some(custom.clone()),
        };
        original.save(app_dir).await.unwrap();
        let loaded = AppSettings::load(app_dir).await.unwrap().unwrap();
        assert_eq!(loaded.theme, Theme::Custom);
        assert_eq!(loaded.custom_theme.as_ref(), Some(&custom));
    }

    #[test]
    fn app_settings_malformed_hex_rejected_with_clear_error() {
        // A bad hex value in the payload must surface as a serde error
        // mentioning the offending value, not silently coerce to black or
        // get dropped. The frontend should never send malformed hex —
        // if it does, the user sees a hard failure rather than a blank
        // theme on next launch.
        let json = r##"{
          "version": 1,
          "journalRoot": "/tmp/x",
          "theme": "custom",
          "customTheme": {
            "bgBase": "not-a-color",
            "bgSurface": "#36302c",
            "bgElevated": "#3d3936",
            "textPrimary": "#f6e7d7",
            "textSecondary": "#d2b094",
            "textMuted": "#a89784",
            "borderStructural": "#241e1a",
            "borderDecorative": "#4a2418",
            "accentPrimary": "#ff5c08",
            "accentGreen": "#95c13b",
            "accentPink": "#eb018b",
            "btnSapphire": "#3a82c8"
          }
        }"##;
        let err = serde_json::from_str::<AppSettings>(json).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("hex color") && msg.contains("not-a-color"),
            "expected the error to name the bad value, got: {msg}"
        );
    }

    #[test]
    fn app_settings_rejects_three_digit_hex_shorthand() {
        // CSS allows #f00; we don't, because the derivation engine works
        // off the canonical 6-digit form. Reject shorthand explicitly so
        // a UI that forgets to expand never sneaks into the wire payload.
        let json = r##"{
          "version": 1,
          "journalRoot": "/tmp/x",
          "theme": "custom",
          "customTheme": {
            "bgBase": "#f00",
            "bgSurface": "#36302c",
            "bgElevated": "#3d3936",
            "textPrimary": "#f6e7d7",
            "textSecondary": "#d2b094",
            "textMuted": "#a89784",
            "borderStructural": "#241e1a",
            "borderDecorative": "#4a2418",
            "accentPrimary": "#ff5c08",
            "accentGreen": "#95c13b",
            "accentPink": "#eb018b",
            "btnSapphire": "#3a82c8"
          }
        }"##;
        assert!(serde_json::from_str::<AppSettings>(json).is_err());
    }

    #[tokio::test]
    async fn app_settings_legacy_without_custom_theme_loads_with_none() {
        // Older app-settings.json (no customTheme field) must still parse.
        // serde(default) on the field is responsible — this test pins the
        // contract so a future refactor that removes #[serde(default)]
        // surfaces immediately.
        let dir = TempDir::new().unwrap();
        let app_dir = dir.path();
        let legacy_json = r#"{
          "version": 1,
          "journalRoot": "/Users/test/MyJournal",
          "theme": "light"
        }"#;
        tokio::fs::write(app_dir.join(APP_SETTINGS_FILENAME), legacy_json)
            .await
            .unwrap();

        let loaded = AppSettings::load(app_dir).await.unwrap().unwrap();
        assert_eq!(loaded.theme, Theme::Light);
        assert!(loaded.custom_theme.is_none());
    }

    #[tokio::test]
    async fn journal_settings_missing_returns_default() {
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        let loaded = JournalSettings::load(&backend).await.unwrap();
        assert_eq!(loaded.user_name, None);
        assert!(!loaded.reminder.enabled);
        assert_eq!(loaded.reminder.days_of_week, vec![4]);
        assert_eq!(loaded.reminder.hour, 16);
    }

    #[tokio::test]
    async fn journal_settings_roundtrip() {
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());

        let original = JournalSettings {
            version: CURRENT_VERSION,
            user_name: Some("Chris".to_string()),
            user_email: Some("chris.carpenter@prodigygame.com".to_string()),
            reminder: ReminderSettings {
                enabled: true,
                days_of_week: vec![4],
                hour: 16,
                minute: 30,
            },
            manager_email: Some("chris.manager@prodigygame.com".to_string()),
            manager_name: Some("Pat".to_string()),
            bamboo_title: Some("Staff QA Analyst".to_string()),
            jira_project_keys: vec!["MAGE".to_string(), "LIVE".to_string()],
            ..JournalSettings::default()
        };
        original.save(&backend).await.unwrap();

        let loaded = JournalSettings::load(&backend).await.unwrap();
        assert_eq!(loaded.user_name, Some("Chris".to_string()));
        assert_eq!(
            loaded.user_email,
            Some("chris.carpenter@prodigygame.com".to_string())
        );
        assert!(loaded.reminder.enabled);
        assert_eq!(loaded.reminder.minute, 30);
        assert_eq!(
            loaded.manager_email,
            Some("chris.manager@prodigygame.com".to_string())
        );
        assert_eq!(loaded.manager_name, Some("Pat".to_string()));
        assert_eq!(loaded.bamboo_title, Some("Staff QA Analyst".to_string()));
        assert_eq!(
            loaded.jira_project_keys,
            vec!["MAGE".to_string(), "LIVE".to_string()]
        );
    }

    #[tokio::test]
    async fn journal_settings_legacy_without_manager_email_parses() {
        // Simulate a settings.json written before the managerEmail field
        // existed. The #[serde(default)] attribute should fill it in as None
        // rather than erroring on the missing field. The Phase 2.7 fields
        // (bambooTitle, jiraProjectKeys) get the same treatment.
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        let legacy_json = r#"{
          "version": 1,
          "userName": "Chris",
          "reminder": { "enabled": false, "dayOfWeek": 4, "hour": 16, "minute": 0 }
        }"#;
        // Write directly through the backend's metadata writer to bypass the
        // serializer (which would always emit managerEmail: null).
        backend
            .write_metadata(JOURNAL_SETTINGS_FILENAME, legacy_json)
            .await
            .unwrap();

        let loaded = JournalSettings::load(&backend).await.unwrap();
        assert_eq!(loaded.manager_email, None);
        assert_eq!(loaded.manager_name, None);
        assert_eq!(loaded.user_name, Some("Chris".to_string()));
        assert_eq!(loaded.user_email, None);
        assert_eq!(loaded.bamboo_title, None);
        assert!(loaded.jira_project_keys.is_empty());
        // The legacy single-day reminder rolls into the multi-day vec.
        assert_eq!(loaded.reminder.days_of_week, vec![4]);
    }

    #[tokio::test]
    async fn journal_settings_user_email_round_trip_with_and_without() {
        // Phase 2.9b: `userEmail` joined JournalSettings. A settings.json
        // written by older builds (no `userEmail` field) must still load
        // — serde(default) takes care of that; this test pins it.
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());

        // 1. Legacy file with no userEmail → user_email == None.
        let legacy_json = r#"{
          "version": 1,
          "userName": "Chris",
          "reminder": { "enabled": false, "daysOfWeek": [4], "hour": 16, "minute": 0 }
        }"#;
        backend
            .write_metadata(JOURNAL_SETTINGS_FILENAME, legacy_json)
            .await
            .unwrap();
        let loaded = JournalSettings::load(&backend).await.unwrap();
        assert_eq!(loaded.user_email, None);

        // 2. Save with a value → reload → see the same value.
        let with_email = JournalSettings {
            user_email: Some("chris.carpenter@prodigygame.com".to_string()),
            ..loaded
        };
        with_email.save(&backend).await.unwrap();
        let reloaded = JournalSettings::load(&backend).await.unwrap();
        assert_eq!(
            reloaded.user_email,
            Some("chris.carpenter@prodigygame.com".to_string())
        );
    }

    #[tokio::test]
    async fn reminder_legacy_day_of_week_promotes_to_single_element_vec() {
        // A Phase 2.0-era settings.json wrote `dayOfWeek: 3` (Thursday).
        // The Phase 2.7 multi-day shape expects `daysOfWeek: [3]`. Verify
        // the load path bridges the two without losing the day.
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        let legacy_json = r#"{
          "version": 1,
          "userName": "Chris",
          "reminder": { "enabled": true, "dayOfWeek": 3, "hour": 9, "minute": 30 }
        }"#;
        backend
            .write_metadata(JOURNAL_SETTINGS_FILENAME, legacy_json)
            .await
            .unwrap();

        let loaded = JournalSettings::load(&backend).await.unwrap();
        assert!(loaded.reminder.enabled);
        assert_eq!(loaded.reminder.days_of_week, vec![3]);
        assert_eq!(loaded.reminder.hour, 9);
        assert_eq!(loaded.reminder.minute, 30);
    }

    #[tokio::test]
    async fn reminder_new_days_of_week_takes_precedence_over_legacy() {
        // If a settings.json somehow has BOTH fields (e.g. a downgrade
        // path or hand-edit), the new field wins — it's the forward-
        // compatible shape and the one the in-app writer emits.
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        let mixed_json = r#"{
          "version": 1,
          "userName": "Chris",
          "reminder": {
            "enabled": true,
            "dayOfWeek": 4,
            "daysOfWeek": [0, 2, 4],
            "hour": 9,
            "minute": 0
          }
        }"#;
        backend
            .write_metadata(JOURNAL_SETTINGS_FILENAME, mixed_json)
            .await
            .unwrap();

        let loaded = JournalSettings::load(&backend).await.unwrap();
        assert_eq!(loaded.reminder.days_of_week, vec![0, 2, 4]);
    }

    #[tokio::test]
    async fn reminder_days_of_week_round_trips_through_save_and_load() {
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        let original = JournalSettings {
            version: CURRENT_VERSION,
            user_name: None,
            user_email: None,
            reminder: ReminderSettings {
                enabled: true,
                days_of_week: vec![0, 2, 4, 6], // Mon/Wed/Fri/Sun
                hour: 14,
                minute: 45,
            },
            manager_email: None,
            manager_name: None,
            bamboo_title: None,
            jira_project_keys: vec![],
            ..JournalSettings::default()
        };
        original.save(&backend).await.unwrap();
        let loaded = JournalSettings::load(&backend).await.unwrap();
        assert_eq!(loaded.reminder.days_of_week, vec![0, 2, 4, 6]);
    }

    #[tokio::test]
    async fn reminder_days_of_week_dedupes_and_sorts() {
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        let messy_json = r#"{
          "version": 1,
          "userName": "Chris",
          "reminder": { "enabled": true, "daysOfWeek": [4, 1, 4, 0, 1], "hour": 16, "minute": 0 }
        }"#;
        backend
            .write_metadata(JOURNAL_SETTINGS_FILENAME, messy_json)
            .await
            .unwrap();

        let loaded = JournalSettings::load(&backend).await.unwrap();
        assert_eq!(loaded.reminder.days_of_week, vec![0, 1, 4]);
    }

    #[tokio::test]
    async fn reminder_empty_days_array_is_preserved_over_legacy_day_of_week() {
        // If a user (or hand-edit, or migration tool) writes an explicit
        // empty array AND leaves a legacy single-day value, the explicit
        // empty MUST win — that's the user's intent. Otherwise the legacy
        // value silently overrides the new one.
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        let mixed_json = r#"{
          "version": 1,
          "userName": "Chris",
          "reminder": { "enabled": true, "dayOfWeek": 3, "daysOfWeek": [], "hour": 16, "minute": 0 }
        }"#;
        backend
            .write_metadata(JOURNAL_SETTINGS_FILENAME, mixed_json)
            .await
            .unwrap();

        let loaded = JournalSettings::load(&backend).await.unwrap();
        assert!(loaded.reminder.days_of_week.is_empty());
    }

    #[tokio::test]
    async fn reminder_empty_days_array_alone_is_preserved() {
        // Sanity check: an explicit empty array without any legacy field
        // should also stay empty. The previous shape would have fallen
        // through to dayOfWeek = None and ended up empty anyway, so this
        // is mostly a guard against regressions in the From impl.
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        let json = r#"{
          "version": 1,
          "userName": "Chris",
          "reminder": { "enabled": true, "daysOfWeek": [], "hour": 16, "minute": 0 }
        }"#;
        backend
            .write_metadata(JOURNAL_SETTINGS_FILENAME, json)
            .await
            .unwrap();

        let loaded = JournalSettings::load(&backend).await.unwrap();
        assert!(loaded.reminder.days_of_week.is_empty());
    }

    #[tokio::test]
    async fn reminder_days_of_week_drops_out_of_range_entries() {
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        let messy_json = r#"{
          "version": 1,
          "userName": "Chris",
          "reminder": { "enabled": true, "daysOfWeek": [2, 99, 7, 4], "hour": 16, "minute": 0 }
        }"#;
        backend
            .write_metadata(JOURNAL_SETTINGS_FILENAME, messy_json)
            .await
            .unwrap();

        let loaded = JournalSettings::load(&backend).await.unwrap();
        // 7 and 99 silently dropped; 2 and 4 kept and sorted.
        assert_eq!(loaded.reminder.days_of_week, vec![2, 4]);
    }

    // ---------- Phase 2.9b mail-send preferences ----------

    #[test]
    fn journal_settings_default_mail_prefs() {
        // The default JournalSettings must promise Gmail + clean text +
        // Business + plaintext-native. These are the choices the UI
        // assumes for a brand-new install with no settings.json edits.
        let s = JournalSettings::default();
        assert_eq!(s.mail_send_mode, MailSendMode::Gmail);
        assert_eq!(s.mail_body_format, MailBodyFormat::CleanText);
        assert_eq!(s.mail_outlook_flavor, OutlookFlavor::Business);
        assert!(!s.mail_native_html);
        assert_eq!(s.mail_body_delivery, MailBodyDelivery::Prefilled);
    }

    #[test]
    fn mail_body_delivery_serializes_kebab_case() {
        // The TypeScript union 'prefilled' | 'clipboard-paste' must match
        // Rust's wire format exactly or the Tauri serde boundary silently
        // drops the field and the user's choice never crosses over.
        let json = serde_json::to_string(&MailBodyDelivery::ClipboardPaste).unwrap();
        assert_eq!(json, "\"clipboard-paste\"");
        let parsed: MailBodyDelivery = serde_json::from_str("\"prefilled\"").unwrap();
        assert_eq!(parsed, MailBodyDelivery::Prefilled);
    }

    #[tokio::test]
    async fn journal_settings_legacy_without_mail_fields_loads_with_defaults() {
        // A settings.json written by a pre-2.9b build has none of the
        // mail* fields. serde(default) on each field must fill them in
        // rather than refusing to load the whole file.
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        let legacy_json = r#"{
          "version": 1,
          "userName": "Chris",
          "reminder": { "enabled": false, "daysOfWeek": [4], "hour": 16, "minute": 0 }
        }"#;
        backend
            .write_metadata(JOURNAL_SETTINGS_FILENAME, legacy_json)
            .await
            .unwrap();

        let loaded = JournalSettings::load(&backend).await.unwrap();
        assert_eq!(loaded.mail_send_mode, MailSendMode::Gmail);
        assert_eq!(loaded.mail_body_format, MailBodyFormat::CleanText);
        assert_eq!(loaded.mail_outlook_flavor, OutlookFlavor::Business);
        assert!(!loaded.mail_native_html);
    }

    #[tokio::test]
    async fn journal_settings_mail_prefs_round_trip_outlook_personal_markdown() {
        // Save with non-default values across all four new fields and
        // confirm reload returns the same shape. Catches accidental
        // serde rename mismatches (kebab-case enum vs camelCase field).
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        let original = JournalSettings {
            mail_send_mode: MailSendMode::Outlook,
            mail_body_format: MailBodyFormat::MarkdownSource,
            mail_native_html: true,
            mail_outlook_flavor: OutlookFlavor::Personal,
            ..JournalSettings::default()
        };
        original.save(&backend).await.unwrap();
        let loaded = JournalSettings::load(&backend).await.unwrap();
        assert_eq!(loaded.mail_send_mode, MailSendMode::Outlook);
        assert_eq!(loaded.mail_body_format, MailBodyFormat::MarkdownSource);
        assert_eq!(loaded.mail_outlook_flavor, OutlookFlavor::Personal);
        assert!(loaded.mail_native_html);
    }

    #[test]
    fn mail_send_mode_serializes_kebab_case() {
        // The serde rename is load-bearing for the frontend — it expects
        // 'gmail' / 'native-mail' / 'outlook'. A silent rename change
        // (e.g. to PascalCase) would break the settings dropdown.
        assert_eq!(
            serde_json::to_string(&MailSendMode::Gmail).unwrap(),
            r#""gmail""#
        );
        assert_eq!(
            serde_json::to_string(&MailSendMode::NativeMail).unwrap(),
            r#""native-mail""#
        );
        assert_eq!(
            serde_json::to_string(&MailSendMode::Outlook).unwrap(),
            r#""outlook""#
        );
    }

    #[test]
    fn mail_body_format_and_outlook_flavor_serialize_kebab_case() {
        assert_eq!(
            serde_json::to_string(&MailBodyFormat::CleanText).unwrap(),
            r#""clean-text""#
        );
        assert_eq!(
            serde_json::to_string(&MailBodyFormat::MarkdownSource).unwrap(),
            r#""markdown-source""#
        );
        assert_eq!(
            serde_json::to_string(&OutlookFlavor::Business).unwrap(),
            r#""business""#
        );
        assert_eq!(
            serde_json::to_string(&OutlookFlavor::Personal).unwrap(),
            r#""personal""#
        );
    }

    // ---------- Phase 2.8+ Colorful Labels toggle ----------

    #[test]
    fn journal_settings_default_colorful_labels_is_false() {
        // Default off so an existing journal upgrades without surprise
        // (every chip suddenly recoloring). The user opts in from
        // Settings > Theme.
        let s = JournalSettings::default();
        assert!(!s.colorful_labels);
    }

    #[tokio::test]
    async fn journal_settings_legacy_without_colorful_labels_loads_with_false() {
        // A settings.json written before this field existed must still
        // load — serde(default) is responsible. This test pins the
        // contract so a future refactor that drops the attribute fails
        // here instead of silently rejecting every legacy file.
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        let legacy_json = r#"{
          "version": 1,
          "userName": "Chris",
          "reminder": { "enabled": false, "daysOfWeek": [4], "hour": 16, "minute": 0 }
        }"#;
        backend
            .write_metadata(JOURNAL_SETTINGS_FILENAME, legacy_json)
            .await
            .unwrap();

        let loaded = JournalSettings::load(&backend).await.unwrap();
        assert!(!loaded.colorful_labels);
    }

    #[test]
    fn task_list_settings_defaults_match_documented_shape() {
        // Locks in the Slice 4/5 defaults so a future refactor that
        // flips one silently fails here rather than shipping.
        let s = TaskListSettings::default();
        assert!(s.show_completed);
        assert!(s.open_tasks_first);
        assert!(!s.show_completed_timestamp);
        assert!(!s.hide_task_list);
        assert!(s.auto_rollover_enabled);
    }

    #[tokio::test]
    async fn journal_settings_legacy_without_task_list_loads_with_defaults() {
        // Pre-Slice-4 settings.json files have no `task_list` field.
        // serde(default) on JournalSettings.task_list is responsible
        // for filling in TaskListSettings::default() when the field is
        // absent. This test pins that contract so a future refactor
        // that drops the attribute fails here.
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        let legacy_json = r#"{
          "version": 1,
          "userName": "Chris",
          "reminder": { "enabled": false, "daysOfWeek": [4], "hour": 16, "minute": 0 }
        }"#;
        backend
            .write_metadata(JOURNAL_SETTINGS_FILENAME, legacy_json)
            .await
            .unwrap();

        let loaded = JournalSettings::load(&backend).await.unwrap();
        assert!(loaded.task_list.show_completed);
        assert!(loaded.task_list.open_tasks_first);
        assert!(!loaded.task_list.show_completed_timestamp);
        assert!(!loaded.task_list.hide_task_list);
        assert!(loaded.task_list.auto_rollover_enabled);
    }

    #[tokio::test]
    async fn journal_settings_partial_task_list_loads_with_field_defaults() {
        // A task_list block that predates the `hide_task_list` field
        // must still parse — `#[serde(default)]` on the individual
        // field carries the default when the JSON key is absent.
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        let json_missing_hide = r#"{
          "version": 1,
          "userName": "Chris",
          "reminder": { "enabled": false, "daysOfWeek": [4], "hour": 16, "minute": 0 },
          "taskList": {
            "showCompleted": true,
            "openTasksFirst": true,
            "showCompletedTimestamp": false
          }
        }"#;
        backend
            .write_metadata(JOURNAL_SETTINGS_FILENAME, json_missing_hide)
            .await
            .unwrap();

        let loaded = JournalSettings::load(&backend).await.unwrap();
        assert!(!loaded.task_list.hide_task_list);
        // Missing `autoRolloverEnabled` in a partial task_list block
        // must upgrade to the default (true), not to serde's bool
        // default (false). Guard against accidental removal of the
        // `#[serde(default = "default_true")]` attribute.
        assert!(loaded.task_list.auto_rollover_enabled);
    }

    #[tokio::test]
    async fn journal_settings_task_list_round_trips() {
        // Saving with non-default toggles and reloading must preserve
        // them — catches accidental snake_case vs camelCase drift on
        // the nested struct's fields.
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        let original = JournalSettings {
            task_list: TaskListSettings {
                show_completed: false,
                open_tasks_first: false,
                show_completed_timestamp: true,
                hide_task_list: true,
                auto_rollover_enabled: false,
            },
            ..JournalSettings::default()
        };
        original.save(&backend).await.unwrap();
        let loaded = JournalSettings::load(&backend).await.unwrap();
        assert!(!loaded.task_list.show_completed);
        assert!(!loaded.task_list.open_tasks_first);
        assert!(loaded.task_list.show_completed_timestamp);
        assert!(loaded.task_list.hide_task_list);
        assert!(!loaded.task_list.auto_rollover_enabled);
    }

    #[tokio::test]
    async fn journal_settings_colorful_labels_round_trips_true() {
        // Saving with the toggle on and reloading must preserve it —
        // catches accidental serde rename mismatches (snake_case Rust
        // field vs camelCase wire format) on this specific field.
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        let original = JournalSettings {
            colorful_labels: true,
            ..JournalSettings::default()
        };
        original.save(&backend).await.unwrap();
        let loaded = JournalSettings::load(&backend).await.unwrap();
        assert!(loaded.colorful_labels);
    }

    #[test]
    fn default_root_uses_documents_subdir() {
        let root = default_journal_root();
        let s = root.to_string_lossy();
        assert!(s.ends_with("Documents/CaptainsLog") || s.ends_with("Documents\\CaptainsLog"));
    }
}
