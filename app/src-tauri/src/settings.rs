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
/// the `<html>` element.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Dark,
    Light,
}

impl Default for Theme {
    fn default() -> Self {
        Theme::Dark
    }
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderSettings {
    pub enabled: bool,
    /// 0 = Monday … 6 = Sunday (ISO weekday convention)
    pub day_of_week: u8,
    pub hour: u8,
    pub minute: u8,
}

impl Default for ReminderSettings {
    fn default() -> Self {
        // Friday at 4pm — the end-of-week reflection slot recommended in docs.
        Self {
            enabled: false,
            day_of_week: 4,
            hour: 16,
            minute: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JournalSettings {
    pub version: u32,
    pub user_name: Option<String>,
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
}

impl Default for JournalSettings {
    fn default() -> Self {
        Self {
            version: CURRENT_VERSION,
            user_name: None,
            reminder: ReminderSettings::default(),
            manager_email: None,
            manager_name: None,
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
        };
        original.save(&app_dir).await.unwrap();

        let loaded = AppSettings::load(&app_dir).await.unwrap().unwrap();
        assert_eq!(loaded.version, original.version);
        assert_eq!(loaded.journal_root, original.journal_root);
        assert_eq!(loaded.theme, Theme::Light);
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
    }

    #[tokio::test]
    async fn journal_settings_missing_returns_default() {
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());
        let loaded = JournalSettings::load(&backend).await.unwrap();
        assert_eq!(loaded.user_name, None);
        assert!(!loaded.reminder.enabled);
        assert_eq!(loaded.reminder.day_of_week, 4);
        assert_eq!(loaded.reminder.hour, 16);
    }

    #[tokio::test]
    async fn journal_settings_roundtrip() {
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystem::new(dir.path());

        let original = JournalSettings {
            version: CURRENT_VERSION,
            user_name: Some("Chris".to_string()),
            reminder: ReminderSettings {
                enabled: true,
                day_of_week: 4,
                hour: 16,
                minute: 30,
            },
            manager_email: Some("chris.manager@prodigygame.com".to_string()),
            manager_name: Some("Pat".to_string()),
        };
        original.save(&backend).await.unwrap();

        let loaded = JournalSettings::load(&backend).await.unwrap();
        assert_eq!(loaded.user_name, Some("Chris".to_string()));
        assert!(loaded.reminder.enabled);
        assert_eq!(loaded.reminder.minute, 30);
        assert_eq!(
            loaded.manager_email,
            Some("chris.manager@prodigygame.com".to_string())
        );
        assert_eq!(loaded.manager_name, Some("Pat".to_string()));
    }

    #[tokio::test]
    async fn journal_settings_legacy_without_manager_email_parses() {
        // Simulate a settings.json written before the managerEmail field
        // existed. The #[serde(default)] attribute should fill it in as None
        // rather than erroring on the missing field.
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
    }

    #[test]
    fn default_root_uses_documents_subdir() {
        let root = default_journal_root();
        let s = root.to_string_lossy();
        assert!(s.ends_with("Documents/CaptainsLog") || s.ends_with("Documents\\CaptainsLog"));
    }
}
