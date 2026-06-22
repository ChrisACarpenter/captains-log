# Captain's Log — Roadmap

## Current phase: 2 — Polish (in progress)

Phase 1 MVP is complete and verified end-to-end. Phase 2 is partially done — first-run + settings backend, the reminder scheduler, and the dedicated capture popup window are all shipped. Remaining: journal browser (read past notes), label autocomplete, settings panel, theme toggle.

---

## Phase 0 — Planning & Scaffolding ✅

- [x] Project folder + docs (README, ROADMAP, DESIGN, DEVELOPMENT-JOURNAL, STYLE-GUIDE, docs/*)
- [x] Tech stack locked (Tauri 2.0 + Svelte 5 + TypeScript + SvelteKit static adapter)
- [x] Brand styling pulled from RPG game (Paytone One + ABeeZee fonts, Lucide icons, 4px grid, signature drop-shadow button)
- [x] Git initialized; private repo at github.com/ChrisACarpenter/captains-log

## Phase 1 — MVP: "Can I capture a Note?" ✅

- [x] Tauri app shell that runs on macOS
- [x] Menu bar icon (Lucide book template image, macOS-native dark/light adaptive)
- [x] Quick-capture popup (one click from menu bar → one click to submit) *(dedicated popup window landed in Phase 2)*
- [x] Writes Notes into the current week's markdown file at `journals/YYYY/YYYY-Www.md`
- [x] Creates the weekly file (with empty Summary scaffold) if it doesn't exist
- [x] Basic Labels field with manual entry (no autocomplete yet)
- [x] Body text supports inline `#hashtags` as labels (basic parsing, no autocomplete yet)
- [x] All labels write to `journals/.metadata/labels.json`
- [x] First-run setup: name, journal location *(shipped in Phase 2 Option B)*
- [x] Theme infrastructure (CSS variables for dark + light; dark hardcoded for v1)

**Success criterion met:** captured multiple notes across an afternoon; markdown files contain frontmatter, Weekly Summary scaffold, Weekly Notes section, timestamped notes with labels and inline `#hashtags`.

## Phase 2 — Polish: "Can I actually use this daily?"

### Done

- [x] Dedicated quick-capture popup window (label `capture`, 460×460, hidden by default, opens via tray)
- [x] Dock icon in addition to menu bar — both shipped (clicking either reaches the app)
- [x] First-run wizard (4 steps: welcome → name → location → reminder)
- [x] Two-tier settings — app-level (`~/Library/Application Support/.../app-settings.json`) + journal-level (`<root>/.metadata/settings.json`)
- [x] Folder picker via `tauri-plugin-dialog`
- [x] Optional weekly reminder notification (real macOS notifications via `tauri-plugin-notification`)
- [x] In-process scheduler restart on settings save (no double-restart needed)
- [x] RPG petbook icon as the app icon (Dock, Finder, Cmd+Tab)
- [x] RPG scroll icon as the reminder notification icon

### Remaining

- [ ] Full journal window with year/week tree sidebar
- [ ] Open and edit past Notes
- [ ] Markdown editor with rich text rendering (CodeMirror 6 or similar)
- [ ] Label autocomplete (JIRA-style dropdown, fed by `labels.json`)
- [ ] Inline `#` autocomplete in body text
- [ ] Weekly Summary UI (the 4-field Lattice template)
- [ ] Settings panel — `/settings` route to edit name, journal location, reminder *after* first-run (uses scheduler-restart-on-save we already shipped)
- [ ] Light/dark theme toggle in Settings
- [ ] macOS system spell-check on inputs (mostly the editor)
- [ ] `labels.json` schema normalized to camelCase (currently `first_used`/`last_used` — predates the camelCase sweep)
- [ ] Hot-swap `LocalFilesystem` when journal_root changes (currently still asks for restart on root change; reminder config already hot-swaps)

**Success:** Captain's Log has replaced any other journaling system I was using.

## Phase 3 — Search & Navigation

- [ ] Full-text search across all weekly files
- [ ] Filter by label, date range, file
- [ ] Click search result → opens correct week, scrolls to correct Note
- [ ] Year/week tree handles many years gracefully

## Phase 4 — Link Enrichment

- [ ] Detect URLs in Notes (Jira, GitHub, Slack, Confluence to start)
- [ ] Fetch metadata via MCP connectors
- [ ] Store enriched metadata inline or in a `.metadata/links/` cache
- [ ] Display enriched cards in the rendered view (status, title, last update)
- [ ] Room to grow to other systems beyond the initial four

## Phase 5 — Performance Review Module

The reason this app exists.

- [ ] Date range picker (calendar UI)
- [ ] "Bundle this range as a single markdown file" export
- [ ] Bundle includes Notes + Summaries + enriched link metadata
- [ ] Bundle prepends a configurable instruction block for the LLM
- [ ] Configurable review-question templates (e.g. the 8-question Prodigy mid-year template)
- [ ] One-click "draft my review" flow that produces editable first-draft answers

## Phase 6 — Sync & Sharing

- [ ] Google Drive sync option (everyone at Prodigy has a Google account)
- [ ] Conflict resolution for multi-device edits
- [ ] At-rest encryption (so synced files aren't readable without the app)

## Phase 7 — Cross-Platform

- [ ] Validated Windows build
- [ ] Validated Linux build
- [ ] CI for all three platforms

---

## Deferred / TBD

- [ ] **Higher-resolution petbook source** — current app icon is upscaled from a 96×96 source PNG. Larger sizes (256/512/1024) are softer than they could be. Replace `src-tauri/icons/source-petbook.png` and re-run `npx @tauri-apps/cli icon …` if a higher-res asset surfaces.
- [ ] **Spacing, motion, and component library finalization** — colors, typography, iconography, and core component patterns are locked in [STYLE-GUIDE.md](STYLE-GUIDE.md). Still TBD: final spacing scale tokens, animation/transition spec, complete reusable component spec library. Address as we build screens in Phase 2.
- [ ] **Bulk label management UI** — rename/merge/delete labels across all files. Phase 2 if it becomes a pain point; later if not.
- [ ] **Plugin / extension API** — let other tools read/write Captain's Log data.
- [ ] **iOS/Android companion app** — flagged but probably not worth doing soon.
- [ ] **Multi-user / team features** — flagged but likely never.
