# Captain's Log — Roadmap

## Current phase: 0 — Planning & Scaffolding

Capturing decisions, drafting design docs, locking the architecture before any code.

---

## Phase 1 — MVP: "Can I capture a Note?"

The smallest useful thing. Confirms the data model and core capture flow work.

- [ ] Tauri app shell that runs on macOS
- [ ] Menu bar icon
- [ ] Quick-capture popup (one click from menu bar → one click to submit)
- [ ] Writes Notes into the current week's markdown file at `journals/YYYY/YYYY-Www.md`
- [ ] Creates the weekly file (with empty Summary scaffold) if it doesn't exist
- [ ] Basic Labels field with manual entry (no autocomplete yet)
- [ ] Body text supports inline `#hashtags` as labels (basic parsing, no autocomplete yet)
- [ ] All labels write to `journals/.metadata/labels.json`
- [ ] First-run setup: name, journal location
- [ ] Theme infrastructure (CSS variables for dark + light; dark hardcoded for v1)

**Success:** I can quick-capture five Notes across a few days, and the markdown files look right.

## Phase 2 — Polish: "Can I actually use this daily?"

- [ ] Full journal window with year/week tree sidebar
- [ ] Open and edit past Notes
- [ ] Markdown editor with rich text rendering (CodeMirror 6 or similar)
- [ ] Label autocomplete (JIRA-style, fed by `labels.json`)
- [ ] Inline `#` autocomplete in body text
- [ ] Weekly Summary UI (the 4-field Lattice template)
- [ ] Dock icon (in addition to menu bar) — clicking opens the journal window
- [ ] macOS system spell-check enabled
- [ ] Settings popup: journal location, reminder on/off + time, name
- [ ] Optional weekly reminder notification
- [ ] Light/dark theme toggle in Settings

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

- [ ] **Spacing, motion, and component library finalization** — colors, typography (Paytone One + ABeeZee), iconography (Lucide + selected RPG assets), and core component patterns are all locked in [STYLE-GUIDE.md](STYLE-GUIDE.md). Still TBD: final spacing scale tokens, animation/transition spec, complete reusable component spec library. Address as we build screens in Phase 2.
- [ ] **Bulk label management UI** — rename/merge/delete labels across all files. Phase 2 if it becomes a pain point; later if not.
- [ ] **Plugin / extension API** — let other tools read/write Captain's Log data.
- [ ] **iOS/Android companion app** — flagged but probably not worth doing soon.
- [ ] **Multi-user / team features** — flagged but likely never.
