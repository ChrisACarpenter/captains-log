//! Storage abstraction for journal data.
//!
//! The [`StorageBackend`] trait is the single point of contact between the
//! rest of the app and "where the data lives." Implementations:
//!
//! - [`LocalFilesystem`] — plain files on disk (v1, MVP)
//! - `GoogleDriveStorage` — cloud sync (Phase 6, not yet implemented)
//! - `EncryptedStorage<B>` — wraps another backend with at-rest encryption (Phase 6)
//!
//! ## On-disk layout (per `docs/file-structure.md`)
//!
//! ```text
//! <root>/
//! ├── .metadata/
//! │   ├── labels.json
//! │   └── settings.json
//! ├── 2026/
//! │   ├── 2026-W01.md
//! │   └── ...
//! └── 2027/
//!     └── ...
//! ```
//!
//! Week numbers use ISO 8601 (1-53). Each weekly file holds frontmatter,
//! an optional Weekly Summary, and zero or more Notes. The trait only deals
//! with raw strings — parsing/serialization lives in the `notes` module.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use thiserror::Error;

/// Errors surfaced from any storage backend.
#[derive(Debug, Error)]
pub enum StorageError {
    #[error("i/o error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("invalid filename: {0}")]
    InvalidFilename(String),

    #[error("invalid week number: year={year} week={week}")]
    InvalidWeek { year: u32, week: u32 },

    #[error("serialization error: {0}")]
    Serde(String),
}

/// Convenience alias used throughout the storage layer.
pub type StorageResult<T> = Result<T, StorageError>;

/// Source of truth for journal persistence.
///
/// All paths are abstracted away — callers only pass `year`, `week`, and
/// metadata `name`. Implementations decide where bytes actually go.
#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// Read a weekly file's raw markdown.
    ///
    /// Returns `Ok(None)` if the file does not exist — callers can treat
    /// "no file" as "no entries yet" without an explicit `exists()` check.
    async fn read_week(&self, year: u32, week: u32) -> StorageResult<Option<String>>;

    /// Write a weekly file. Creates parent directories as needed.
    async fn write_week(&self, year: u32, week: u32, content: &str) -> StorageResult<()>;

    /// List week numbers present for a given year, sorted ascending.
    /// Returns an empty Vec if the year folder doesn't exist.
    async fn list_weeks(&self, year: u32) -> StorageResult<Vec<u32>>;

    /// List years for which any weekly files exist, sorted ascending.
    /// Returns an empty Vec if the root folder doesn't exist.
    async fn list_years(&self) -> StorageResult<Vec<u32>>;

    /// Read a named metadata file (e.g., `"labels.json"`, `"settings.json"`).
    /// Returns `Ok(None)` if the file does not exist.
    async fn read_metadata(&self, name: &str) -> StorageResult<Option<String>>;

    /// Write a named metadata file. Creates `.metadata/` as needed.
    async fn write_metadata(&self, name: &str, content: &str) -> StorageResult<()>;
}

// ---------------------------------------------------------------------------
// LocalFilesystem
// ---------------------------------------------------------------------------

/// Stores journal data as plain files on the local disk.
pub struct LocalFilesystem {
    root: PathBuf,
}

impl LocalFilesystem {
    /// Construct a backend rooted at `root`. The directory is not created
    /// here — directories are created lazily on first write.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Public root accessor, mostly for debugging / settings UIs.
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn week_path(&self, year: u32, week: u32) -> StorageResult<PathBuf> {
        if !(1..=53).contains(&week) {
            return Err(StorageError::InvalidWeek { year, week });
        }
        Ok(self
            .root
            .join(year.to_string())
            .join(format!("{:04}-W{:02}.md", year, week)))
    }

    fn metadata_path(&self, name: &str) -> PathBuf {
        self.root.join(".metadata").join(name)
    }
}

/// Map a `std::io::Error` to `StorageError::Io` while remembering the path.
fn io(path: impl Into<PathBuf>, source: std::io::Error) -> StorageError {
    StorageError::Io {
        path: path.into(),
        source,
    }
}

#[async_trait]
impl StorageBackend for LocalFilesystem {
    async fn read_week(&self, year: u32, week: u32) -> StorageResult<Option<String>> {
        let path = self.week_path(year, week)?;
        match tokio::fs::read_to_string(&path).await {
            Ok(content) => Ok(Some(content)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(io(path, e)),
        }
    }

    async fn write_week(&self, year: u32, week: u32, content: &str) -> StorageResult<()> {
        let path = self.week_path(year, week)?;
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| io(parent, e))?;
        }
        tokio::fs::write(&path, content)
            .await
            .map_err(|e| io(&path, e))
    }

    async fn list_weeks(&self, year: u32) -> StorageResult<Vec<u32>> {
        let year_dir = self.root.join(year.to_string());
        let mut entries = match tokio::fs::read_dir(&year_dir).await {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(io(year_dir, e)),
        };

        let mut weeks = Vec::new();
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| io(&year_dir, e))?
        {
            let name = entry.file_name();
            if let Some(s) = name.to_str() {
                if let Some(week) = parse_week_filename(s, year) {
                    weeks.push(week);
                }
            }
        }

        weeks.sort();
        Ok(weeks)
    }

    async fn list_years(&self) -> StorageResult<Vec<u32>> {
        let mut entries = match tokio::fs::read_dir(&self.root).await {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(io(&self.root, e)),
        };

        let mut years = Vec::new();
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| io(&self.root, e))?
        {
            let file_type = entry
                .file_type()
                .await
                .map_err(|e| io(entry.path(), e))?;
            if !file_type.is_dir() {
                continue;
            }
            if let Some(s) = entry.file_name().to_str() {
                if let Ok(year) = s.parse::<u32>() {
                    if (1900..=3000).contains(&year) {
                        years.push(year);
                    }
                }
            }
        }

        years.sort();
        Ok(years)
    }

    async fn read_metadata(&self, name: &str) -> StorageResult<Option<String>> {
        let path = self.metadata_path(name);
        match tokio::fs::read_to_string(&path).await {
            Ok(content) => Ok(Some(content)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(io(path, e)),
        }
    }

    async fn write_metadata(&self, name: &str, content: &str) -> StorageResult<()> {
        let path = self.metadata_path(name);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| io(parent, e))?;
        }
        tokio::fs::write(&path, content)
            .await
            .map_err(|e| io(&path, e))
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Parse a weekly filename of the form `YYYY-Www.md` and return the week
/// number, or `None` if the format doesn't match the expected year.
fn parse_week_filename(name: &str, expected_year: u32) -> Option<u32> {
    let expected_prefix = format!("{:04}-W", expected_year);
    let week_str = name
        .strip_prefix(&expected_prefix)?
        .strip_suffix(".md")?;
    let week: u32 = week_str.parse().ok()?;
    if (1..=53).contains(&week) {
        Some(week)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_backend() -> (TempDir, LocalFilesystem) {
        let dir = TempDir::new().expect("tempdir");
        let backend = LocalFilesystem::new(dir.path());
        (dir, backend)
    }

    #[tokio::test]
    async fn read_missing_week_returns_none() {
        let (_dir, backend) = make_backend();
        let result = backend.read_week(2026, 25).await.unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn write_then_read_week_roundtrips() {
        let (_dir, backend) = make_backend();
        let content = "# Week of June 15 - June 21, 2026\n\nHello.";
        backend.write_week(2026, 25, content).await.unwrap();
        let read_back = backend.read_week(2026, 25).await.unwrap();
        assert_eq!(read_back, Some(content.to_string()));
    }

    #[tokio::test]
    async fn write_week_creates_year_folder() {
        let (dir, backend) = make_backend();
        backend.write_week(2026, 1, "content").await.unwrap();
        let year_dir = dir.path().join("2026");
        assert!(year_dir.is_dir(), "year folder should exist");
        let file_path = year_dir.join("2026-W01.md");
        assert!(file_path.is_file(), "weekly file should exist");
    }

    #[tokio::test]
    async fn list_weeks_returns_sorted_weeks() {
        let (_dir, backend) = make_backend();
        backend.write_week(2026, 5, "a").await.unwrap();
        backend.write_week(2026, 1, "b").await.unwrap();
        backend.write_week(2026, 23, "c").await.unwrap();

        let weeks = backend.list_weeks(2026).await.unwrap();
        assert_eq!(weeks, vec![1, 5, 23]);
    }

    #[tokio::test]
    async fn list_weeks_for_missing_year_returns_empty() {
        let (_dir, backend) = make_backend();
        let weeks = backend.list_weeks(1999).await.unwrap();
        assert!(weeks.is_empty());
    }

    #[tokio::test]
    async fn list_weeks_ignores_non_matching_files() {
        let (dir, backend) = make_backend();
        // Create a year folder with both valid and noise files.
        tokio::fs::create_dir_all(dir.path().join("2026"))
            .await
            .unwrap();
        tokio::fs::write(dir.path().join("2026/2026-W10.md"), "x")
            .await
            .unwrap();
        tokio::fs::write(dir.path().join("2026/notes.txt"), "x")
            .await
            .unwrap();
        tokio::fs::write(dir.path().join("2026/2025-W30.md"), "x") // wrong year
            .await
            .unwrap();

        let weeks = backend.list_weeks(2026).await.unwrap();
        assert_eq!(weeks, vec![10]);
    }

    #[tokio::test]
    async fn list_years_returns_sorted_years() {
        let (_dir, backend) = make_backend();
        backend.write_week(2027, 1, "a").await.unwrap();
        backend.write_week(2024, 1, "b").await.unwrap();
        backend.write_week(2026, 1, "c").await.unwrap();

        let years = backend.list_years().await.unwrap();
        assert_eq!(years, vec![2024, 2026, 2027]);
    }

    #[tokio::test]
    async fn list_years_ignores_non_numeric_dirs() {
        let (dir, backend) = make_backend();
        tokio::fs::create_dir_all(dir.path().join("2026"))
            .await
            .unwrap();
        tokio::fs::create_dir_all(dir.path().join(".metadata"))
            .await
            .unwrap();
        tokio::fs::create_dir_all(dir.path().join("not-a-year"))
            .await
            .unwrap();

        let years = backend.list_years().await.unwrap();
        assert_eq!(years, vec![2026]);
    }

    #[tokio::test]
    async fn metadata_roundtrips() {
        let (_dir, backend) = make_backend();
        backend
            .write_metadata("labels.json", r#"{"labels":[]}"#)
            .await
            .unwrap();
        let read = backend.read_metadata("labels.json").await.unwrap();
        assert_eq!(read, Some(r#"{"labels":[]}"#.to_string()));
    }

    #[tokio::test]
    async fn read_missing_metadata_returns_none() {
        let (_dir, backend) = make_backend();
        let read = backend.read_metadata("settings.json").await.unwrap();
        assert_eq!(read, None);
    }

    #[tokio::test]
    async fn invalid_week_number_is_rejected_on_read() {
        let (_dir, backend) = make_backend();
        let err = backend.read_week(2026, 0).await.unwrap_err();
        matches!(err, StorageError::InvalidWeek { .. });
        let err = backend.read_week(2026, 54).await.unwrap_err();
        matches!(err, StorageError::InvalidWeek { .. });
    }

    #[test]
    fn parse_week_filename_happy_path() {
        assert_eq!(parse_week_filename("2026-W25.md", 2026), Some(25));
        assert_eq!(parse_week_filename("2026-W01.md", 2026), Some(1));
        assert_eq!(parse_week_filename("2026-W53.md", 2026), Some(53));
    }

    #[test]
    fn parse_week_filename_rejects_mismatched_year() {
        assert_eq!(parse_week_filename("2025-W25.md", 2026), None);
    }

    #[test]
    fn parse_week_filename_rejects_out_of_range_week() {
        assert_eq!(parse_week_filename("2026-W00.md", 2026), None);
        assert_eq!(parse_week_filename("2026-W54.md", 2026), None);
    }

    #[test]
    fn parse_week_filename_rejects_non_markdown() {
        assert_eq!(parse_week_filename("2026-W25.txt", 2026), None);
        assert_eq!(parse_week_filename("2026-W25", 2026), None);
    }
}
