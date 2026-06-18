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

In active development. Currently in **Phase 0 — Planning & Scaffolding**.

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
- [DESIGN.md](DESIGN.md) — architecture, tech stack, data model
- [DEVELOPMENT-JOURNAL.md](DEVELOPMENT-JOURNAL.md) — running log of decisions and progress
- [docs/](docs/) — detailed design specs

## Tech stack (planned)

- **App framework:** Tauri 2.0 (Rust backend + web frontend)
- **Frontend:** TypeScript + Svelte 5
- **Editor:** CodeMirror 6 or similar
- **Storage:** Plain markdown files on disk

## Brand & voice

This is a Prodigy internal product. UI copy follows Prodigy's brand voice:

> Progressive, imaginative, zealous, galvanizing. Sage-Outlaw persona. Conversational yet professional. Pronouns (we, our, you), contractions, concise and friendly. Direct but not blunt, educational but not verbose.

Colors are defined in [STYLE-GUIDE.md](STYLE-GUIDE.md). Dark mode is the default. Typography and component standards are still being pulled together — see ROADMAP "Deferred / TBD."
