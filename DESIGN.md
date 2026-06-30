# Captain's Log — Design

High-level architecture and key design decisions. Detailed specs live under [docs/](docs/).

## Tech stack

| Layer | Choice | Rationale |
|---|---|---|
| App framework | Tauri 2.0 | Lightweight (~5MB), uses native WebKit on macOS, Rust backend, cross-platform |
| Backend | Rust (via Tauri) | Memory-safe, fast, no GC overhead |
| Frontend | TypeScript + Svelte 5 | Smaller bundle than React, less boilerplate, plays well with Tauri's lightweight philosophy |
| Markdown editor | CodeMirror 6 | Mature, lightweight, good markdown support; live-preview decorations layered on top (Phase 2.5) |
| Storage (v1) | Plain markdown files on disk | Portable, future-proof, grep-able, git-friendly |
| Sync (future) | Google Drive | Everyone at Prodigy has a Google account |
| Encryption (future) | Layered on top of storage backend | Plug-in stage between content and disk/sync |

## Architecture at a glance

```
┌──────────────────────────────────────────────────┐
│  Frontend (TypeScript in WebKit, via Tauri)      │
│   Quick Capture   Journal Window   Settings      │
└──────────────────────────────────────────────────┘
                  ↕  Tauri IPC
┌──────────────────────────────────────────────────┐
│  Backend (Rust)                                  │
│   Notes API        Label Index    Notifications  │
│   Storage Layer (trait)                          │
│      ├─ LocalFilesystem  (v1)                    │
│      └─ GoogleDrive      (Phase 6)               │
└──────────────────────────────────────────────────┘
                  ↕
        Disk: <root>/YYYY/YYYY-Www.md
              + <root>/.metadata/
            (<root> defaults to ~/Documents/CaptainsLog,
             user-picked during onboarding)
```

## Key design decisions

### Storage layer abstraction

The Notes API never touches the filesystem directly. It calls a `StorageBackend` trait with implementations for `LocalFilesystem` (v1) and `GoogleDrive` (Phase 6).

Each backend implements:

- `read_week(year, week_num) -> Result<MarkdownFile>`
- `write_week(year, week_num, content) -> Result<()>`
- `list_weeks(year) -> Vec<u32>`
- `list_years() -> Vec<u32>`
- `read_metadata(name) -> Result<Json>`
- `write_metadata(name, json) -> Result<()>`

Encryption is a wrapping `EncryptedStorage<B: StorageBackend>` that transparently encrypts on write and decrypts on read. This means encryption "just works" regardless of which backend is underneath.

### Quick capture is the load-bearing UX

The single biggest predictor of journaling success is friction. Quick capture is two clicks total:

1. Click menu bar icon
2. Type → click Submit (or ⌘↩)

No date picker, no week picker, no category — it always goes to "now" in the current week's file. The user can later edit or categorize via the full journal window.

### Two label inputs, one index

See [docs/label-system.md](docs/label-system.md). Labels can come from a dedicated field OR from inline `#hashtags` in body text. Both feed `<root>/.metadata/labels.json`. The autocomplete pool is the union of all labels ever used.

### Markdown is the source of truth

Everything is markdown. The label index, settings, etc. are JSON for performance, but they're rebuildable from the markdown files. If `.metadata/` is ever deleted, the app rebuilds it from a scan.

This guarantees:

- No vendor lock-in
- Users can edit files in any external editor
- The journal travels with the user (sync, backup, migration)
- The eventual LLM bundle export is trivial — just concatenate files in the date range

## Open architectural questions

- **State management:** Probably keep it simple — Svelte stores + Tauri IPC. Avoid heavy state libraries unless we hit a real need.

## Resolved decisions

- **Editor (Phase 2.5):** CodeMirror 6 shipped — markdown stays byte-identical on disk, Slack/Typora-style live-preview decorations hide markers (`**`, `*`, `~~`, `#`, etc.) without mutating the source. WYSIWYG approaches (TipTap, Milkdown) were considered but lose source fidelity.

## Voice & brand

UI copy follows Prodigy's brand voice (per the Prodigy deck template):

- Progressive, imaginative, zealous, galvanizing
- Sage-Outlaw persona — driven by knowledge, willing to challenge defaults
- Conversational yet professional
- Pronouns (we, our, you), contractions, concise / friendly / benefit-driven
- Direct, not blunt; educational, not verbose

Colors are defined in [STYLE-GUIDE.md](STYLE-GUIDE.md). Typography is TBD — see [ROADMAP.md](ROADMAP.md) "Deferred / TBD."

The app supports **Light, Dark, and Custom** themes. Dark is the default; the picker lives in Settings → Theme. Custom themes (Phase 2.8) let the user edit 12 primary colors; the engine derives ~23 dependent tokens via OKLCH with WCAG AA contrast validation, and themes export/import as `.captheme.json`. A tray-menu "Preset Theme" submenu is the escape hatch if a Custom palette makes the in-app picker unreadable. Phase 2.8b's "Colorful Labels" toggle gives each label a per-name hue that regenerates against the active surface (no theme-burn).
