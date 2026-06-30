# Captain's Log

A weekly work journal for Prodigy employees, built to make self-reviews painless.

## The problem

Self-reviews and performance reviews take an enormous amount of time because remembering what you actually did over six months is hard. You end up digging through Slack, Jira, Confluence, and your own memory to reconstruct events.

Captain's Log fixes this by capturing work as it happens, organized weekly, in a format ready to feed to an LLM for first-draft review answers.

## What it is

A small, native Mac (cross-platform later) app that:

- Captures timestamped Notes throughout the week via a quick-capture flow (2 clicks)
- Organizes Notes into weekly markdown files
- Supports an optional structured Weekly Summary (the 4-question Lattice template)
- Stores everything as plain markdown on disk, fully portable
- Powers a Performance Review module that bundles a date range of Notes + Summaries and feeds them to an LLM along with review questions

## Status

In active development. Phase 2.8c shipped 2026-06-30 — see [ROADMAP.md](ROADMAP.md) for the full phase history. Next up: **Phase 3a — Label library viewer + bulk management**.

## Project vocabulary

| Term | Meaning |
|---|---|
| **Note** | A single timestamped entry. Created via quick-capture or in the journal window. |
| **Weekly Summary** | The optional 4-question structured summary for a given week, modeled on the Lattice template. |
| **Weekly Notes** | The section of a weekly markdown file that holds all the Notes for that week. |
| **Label** | A tag attached to a Note (or Summary). Lives in both the dedicated Labels field and inline as `#hashtags` in body text. |
| **Period** | A date range used by the Performance Review module to bundle Notes for export. |

## Key documents

- [ROADMAP.md](ROADMAP.md) — phased feature plan
- [ARCHITECTURE.md](ARCHITECTURE.md) — system architecture (storage, live-preview, IPC, etc.)
- [DESIGN.md](DESIGN.md) — design rationale + product decisions
- [STYLE-GUIDE.md](STYLE-GUIDE.md) — visual design language + brand
- [DEVELOPMENT-JOURNAL.md](DEVELOPMENT-JOURNAL.md) — running log of decisions and progress
- [docs/components.md](docs/components.md) — shared Svelte component library index
- [docs/](docs/) — detailed design specs (data format, file structure, label system, UX flows, first-run setup)

## Tech stack

- **App framework:** Tauri 2.x (Rust backend + WebKit frontend)
- **Frontend:** TypeScript + Svelte 5 + SvelteKit (static adapter)
- **Editor:** CodeMirror 6 with live-preview decorations + custom markdown extensions
- **Storage:** Plain markdown files on disk + sidecar `.metadata/` JSON

## Brand & voice

This is a Prodigy internal product. UI copy follows Prodigy's brand voice:

> Progressive, imaginative, zealous, galvanizing. Sage-Outlaw persona. Conversational yet professional. Pronouns (we, our, you), contractions, concise and friendly. Direct but not blunt, educational but not verbose.

Brand colors, typography, iconography, and component patterns are all defined in [STYLE-GUIDE.md](STYLE-GUIDE.md). Dark mode is the default. The app borrows its visual language from the Prodigy RPG game's design system (Paytone One + ABeeZee fonts, the signature 4px bottom drop shadow, etc.) rather than the corporate marketing brand.
