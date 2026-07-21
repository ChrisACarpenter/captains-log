<p align="center">
  <img src="https://capsule-render.vercel.app/api?type=waving&height=170&color=0:2b2420,45:ff5c08,100:6c1e38&text=Captain%27s%20Log&fontColor=f6e7d7&fontAlignY=35&fontSize=42&desc=Weekly%20work%20journal%20for%20painless%20self-reviews&descAlignY=57&descSize=16" alt="Captain's Log — weekly work journal for painless self-reviews" />
</p>

# 🧭 Captain's Log

> **"Capture what you did while you still remember it. Your future self, mid-review, will thank you."**

A small, native Mac app for keeping a weekly work journal. Built to make self-reviews and performance reviews painless — you write a line or two as things happen; six months later, an LLM turns it into first-draft answers to your review questions.

## 🧠 The problem

Self-reviews take an enormous amount of time because remembering what you actually did over six months is hard. You end up digging through Slack, Jira, Confluence, and your own memory to reconstruct events.

Captain's Log fixes this by capturing work as it happens, organized weekly, in a format that's ready to hand to an LLM for a first-draft review.

## 🚀 Quick Start

1. Grab the latest `.dmg` from [Releases](https://github.com/SMARTeacher/captains-log/releases) and drop **Captain's Log** into `/Applications`.
2. Launch it — the first-run wizard picks your journal folder and reminder preferences.
3. Click the book icon in your menu bar (or open the main window) to capture your first Note.
4. On Friday, hit **Write Weekly Summary**. On review day, hit **Prep Self Review**.

That's the whole loop. Everything else is optional.

## ✨ What it does

- **Quick-capture Notes** in two clicks from the menu bar — timestamped, filed straight into the current week's markdown file.
- **Weekly Summary** template modeled on the Lattice 4-question format for teams that use it.
- **Tasks as first-class citizens** — aggregated on the landing page, with optional due dates and OS-notification reminders that roll forward automatically across weeks.
- **Full-text search** across every weekly file with a `Cmd+K` shortcut, jump-to-position from the results.
- **Prep Self Review wizard** — bundles a date range of Notes + Summaries into a markdown doc you can hand to Claude (or any LLM) with review questions.
- **Plain markdown on disk** — you own your data. `cat` it, grep it, sync it via iCloud or Git.

## 📍 Status

**1.0 ready.** Phases 1–5 shipped, the Pre-1.0 arc (Polish Sweep, MkDocs research, Style System Finalization, Fontsource migration, Final Documentation Pass) is complete. See [ROADMAP.md](ROADMAP.md) for the phase-by-phase record.

## 📖 Project vocabulary

| Term | Meaning |
|---|---|
| **Note** | A single timestamped entry. Created via quick-capture or in the journal window. |
| **Weekly Summary** | The optional 4-question structured summary for a given week, modeled on the Lattice template. |
| **Weekly Notes** | The section of a weekly markdown file that holds all the Notes for that week. |
| **Label** | A tag attached to a Note (or Summary). Lives in both the dedicated Labels field and inline as `#hashtags` in body text. |
| **Task** | A `- [ ]` checkbox item in the week's `### Tasks` section — aggregated on the landing page, sidecar-backed for completions, due dates, and rollover state. |
| **Period** | A date range used by the Prep Self Review module to bundle Notes for export. |

## 📚 Key documents

- [ROADMAP.md](ROADMAP.md) — phased feature plan and phase-by-phase receipts
- [ARCHITECTURE.md](ARCHITECTURE.md) — system architecture (storage, live-preview, IPC, task pipeline, link chips)
- [DESIGN.md](DESIGN.md) — design rationale + product decisions
- [STYLE-GUIDE.md](STYLE-GUIDE.md) — visual design language + brand
- [DEVELOPMENT-JOURNAL.md](DEVELOPMENT-JOURNAL.md) — append-only running log of decisions and progress
- [docs/components.md](docs/components.md) — shared Svelte component library index
- [docs/](docs/) — detailed design specs (data format, file structure, label system, UX flows, first-run setup)

## 🧰 Tech stack

- **App shell:** Tauri 2.x (Rust core + WKWebView on macOS)
- **Frontend:** TypeScript + Svelte 5 (runes) + SvelteKit static adapter
- **Editor:** CodeMirror 6 + Lezer with live-preview decorations, custom widgets (date chips, task checkboxes, link chips), and a bespoke markdown toolbar
- **HTML rendering:** pulldown-cmark + ammonia (for send-to-manager, task rendering, and other markdown→HTML surfaces)
- **Fonts:** Self-hosted Paytone One + ABeeZee — WOFF2 files ship in [`app/static/fonts/`](app/static/fonts/), original sources from [Fontsource](https://fontsource.org/) (OFL). No CDN dependency at runtime.
- **Storage:** Plain markdown files on disk + sidecar `.metadata/` JSON (task completions, due dates, rollover log, link cache)

## 🎨 Brand & voice

Captain's Log is a homebrew personal project that ended up adopted internally at Prodigy. UI copy follows Prodigy's brand voice:

> Progressive, imaginative, zealous, galvanizing. Sage-Outlaw persona. Conversational yet professional. Pronouns (we, our, you), contractions, concise and friendly. Direct but not blunt, educational but not verbose.

Brand colors, typography, iconography, and component patterns are all in [STYLE-GUIDE.md](STYLE-GUIDE.md). The app borrows its visual language from the Prodigy RPG game's design system — Paytone One + ABeeZee, warm-tinted neutrals, the signature 4px bottom drop shadow on buttons — rather than the corporate marketing brand. Dark mode is the default.

---

*Built in odd hours by a QA who's a frontend tourist. Now go log something worth remembering.*

<p align="center">
  <img src="https://capsule-render.vercel.app/api?type=waving&height=100&color=0:6c1e38,55:ff5c08,100:2b2420&section=footer" alt="" />
</p>
