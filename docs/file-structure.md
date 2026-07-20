# File Structure

How journal data is laid out on disk.

## Tree

```
<user-chosen-root>/                       # Default: ~/Documents/CaptainsLog/
├── .metadata/
│   ├── labels.json                       # Label index (see label-system.md)
│   ├── settings.json                     # Journal-level settings (see data-format.md)
│   ├── sent-log.json                     # Per-week send-to-manager history (Phase 2.6)
│   ├── capture-draft.json                # In-flight quick-capture draft (Phase 2 auto-save)
│   ├── task-completions.json             # Phase 3c task completion timestamps
│   ├── task-due-dates.json               # Phase 3e task due dates
│   ├── rollover-log.json                 # Phase 3c rollover provenance
│   ├── auto-import-log.json              # Phase 3c/Slice 6c auto-import trigger log
│   ├── link-cache.json                   # Phase 4 URL → enrichment metadata cache
│   └── pre-slice6-backups/               # Pre-6a byte-identical weekly-file backups
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

## Task-related sidecars

`task-completions.json` stores per-task completion timestamps + state; `rollover-log.json` tracks which tasks were rolled forward from prior weeks with provenance; `auto-import-log.json` logs when auto-import triggers to prevent duplicate processing; `task-due-dates.json` maps tasks to optional local due dates (Phase 3e).

Tasks now live in a dedicated `### Tasks` section within the Weekly Summary (Phase 3c+), not in Plans and priorities. See data-format.md for the updated weekly file structure.

## Link-chip sidecar

`link-cache.json` maps URLs to their fetched enrichment metadata (title, siteName, favicon data URI) for the Phase 4 inline link-chip widget. Populated lazily by the `enrich_link` Tauri command; the frontend reads back cached entries so re-rendering a doc doesn't re-hit the network. Auth-gated URLs (Jira, Slack, private GitHub, etc.) cache an entry with null metadata fields and render as a hostname/globe fallback chip. See data-format.md for the full schema.

## Pre-Slice-6 backups

`pre-slice6-backups/` holds byte-identical snapshots of legacy weekly files taken just before the Slice-6a task-section migration first touches them. Users do not interact with this directory directly; each file is created once and never overwritten, so a hand-recovery is always possible if the migration output looks wrong.

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

## Cross-device sync

Cross-device sync was dropped from the roadmap on 2026-07-16 (see ROADMAP.md). Captain's Log is single-machine by design; users who want sync can point the journal root at a folder inside iCloud Drive / Dropbox / Google Drive at their own risk.

## Backup

For v1, backup is the user's responsibility. Recommended approaches:

- macOS Time Machine (covers `~/Documents` by default)
- `git init` in the journal root (markdown files are perfect for git)
- Manual cloud sync of the folder
