# File Structure

How journal data is laid out on disk.

## Tree

```
<user-chosen-root>/                       # Default: ~/Documents/CaptainsLog/
├── .metadata/
│   ├── labels.json                       # Label index (see label-system.md)
│   ├── settings.json                     # Journal-level settings (see data-format.md)
│   ├── sent-log.json                     # Per-week send-to-manager history (Phase 2.6)
│   └── capture-draft.json                # In-flight quick-capture draft (Phase 2 auto-save)
├── 2026/
│   ├── 2026-W01.md
│   ├── 2026-W02.md
│   ├── ...
│   └── 2026-W52.md
└── 2027/
    └── ...
```

## Conventions

- **Root location:** user-chosen during first-run setup. Default suggestion: `~/Documents/CaptainsLog/`.
- **Year folders:** `YYYY/` (4 digits).
- **Weekly files:** `YYYY-Www.md` (ISO 8601 week number).
- **Metadata folder:** `.metadata/` (dotfile = hidden in Finder, signals "internal" / "app-managed").

## Why a dot-metadata folder?

- Hidden by default in Finder (clean for the user)
- Easy to grep across journals (skip `.metadata` in scans)
- Makes the JSON files obviously "app-managed" vs the markdown which is "user-managed"

## Why year folders?

Without year folders, after a few years of weekly files you have hundreds of files in one directory. Year folders mean:

- Up to 53 files per year folder (bounded)
- Easy manual browsing
- Easy file-system tooling (sync, backup, archive a year)

## What if the user moves the root folder?

The user picks a new location from Settings. The app:

1. Stops writing to the old location
2. Updates `settings.json` (in the new location)
3. Offers to move existing files (rsync-equivalent)
4. On confirm, copies everything to the new location and (optionally) cleans up the old folder

## Cross-device safety (Phase 6)

When Google Drive sync exists, the same root folder is shared across machines. Some settings are per-device (notification preferences, window positions) and some are per-journal (user name, label index).

This means `settings.json` may eventually split:

- `settings.json` in `.metadata/` (synced, journal-wide)
- App-support directory equivalent (per-machine, device-specific)

Cross that bridge when we build sync.

## Backup

For v1, backup is the user's responsibility. Recommended approaches:

- macOS Time Machine (covers `~/Documents` by default)
- `git init` in the journal root (markdown files are perfect for git)
- Manual cloud sync of the folder
