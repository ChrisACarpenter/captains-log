/**
 * HTML body content for the Help and Nerds Only popups. Authored by the
 * help-popups-discovery workflow (parallel agents drafted content from
 * a survey of actual keyboard shortcuts + clickable affordances in the
 * codebase; synthesizer plugged in the real shortcut list). Updated
 * 2026-07-20 to cover Phases 3a–5 + the Pre-1.0 Polish Sweep (Cmd+K
 * search, Tasks + due dates + reminders, Prep Self Review wizard, link
 * chips, hide-send-to-manager, Fontsource self-hosted fonts, task/link
 * sidecars, tasks.rs / review_prep.rs / link_enrich.rs modules).
 *
 * The strings are HTML, not markdown — rendered via Svelte's {@html ...}
 * inside HelpButtons.svelte's popup container. Update them here when
 * shortcuts change, when new buttons land, when the tech stack moves.
 */

export const HELP_HTML = `<h3>What is Captain's Log?</h3>
<p>A private, local-first journal for capturing your work as it happens — so performance reviews write themselves.</p>

<h3>The main surfaces</h3>
<ul>
  <li><strong>/capture</strong> — a fast jot box for in-the-moment notes; whatever you type lands in this week's journal.</li>
  <li><strong>/summary</strong> — a structured weekly form (key accomplishments, plans, challenges, anything else on your mind) that you can mail to your manager.</li>
  <li><strong>/journal</strong> — the full chronological log, browsable and editable by ISO week.</li>
  <li><strong>/search</strong> — full-text search across every week; filter by label, click a hit to jump to that week. Open from anywhere in the main window with <kbd>Cmd</kbd>+<kbd>K</kbd>.</li>
</ul>

<h3>What's a note?</h3>
<p>A note is anything you capture — a win, a blocker, a tiny observation. Every note rolls into the current week's journal file, timestamped automatically.</p>
<ul>
  <li>No tags or folders required — just write.</li>
  <li>Past weeks stay put; new notes always land in the current week.</li>
  <li>Edit or delete any note directly in /journal.</li>
</ul>

<h3>Quick capture from anywhere</h3>
<p>Look for the Captain's Log icon in your menu bar (top-right of the screen). <strong>Left-click</strong> it to open the capture popup from anywhere — even when the main window is hidden. Jot, save, get back to what you were doing. Right-click the icon for "Show Captain's Log", "Preset Theme" (Dark / Light escape hatch), and "Quit".</p>

<h3>Sending your weekly summary</h3>
<p>The <strong>Send</strong> button on /summary and /journal opens a draft in your default mail client. Pick which client — and how the body gets there — in <strong>Settings → Mail</strong>. Three send modes:</p>
<ul>
  <li><strong>Gmail</strong> (default) — opens a compose tab. If you set your email in Settings → General, multi-account users land in the right inbox automatically.</li>
  <li><strong>Native Mac Mail</strong> — talks to Mail.app via AppleScript. Optional "Styled HTML draft (.eml)" mode delivers a fully rendered message with no paste step.</li>
  <li><strong>Outlook</strong> — Business (Microsoft 365) or Personal (outlook.com), your pick.</li>
</ul>
<p>And two body-delivery flavors that apply to all three clients:</p>
<ul>
  <li><strong>Prefilled draft</strong> — the compose window opens with the full summary already in the body. Plaintext only (mail clients don't accept rich HTML through URL prefills).</li>
  <li><strong>Compose + paste (formatted)</strong> — compose opens empty and the fully formatted HTML lands on your clipboard. <kbd>Cmd</kbd>+<kbd>V</kbd>, then Send. Two clicks in Captain's Log, one paste, one Send.</li>
</ul>
<p>The <strong>Preview</strong> button on the Send dialog shows exactly what your manager will receive before you hand off. Captain's Log never sends the mail itself — you review and send from your real mail identity, so threading and the Sent folder work normally.</p>
<p>Don't want any of this? <strong>Settings → General → Hide Send-to-manager</strong> suppresses the Send buttons and the manager-name / manager-email fields (your saved values are kept, so re-enabling picks up where you left off).</p>

<h3>Tasks</h3>
<p>Tasks live in two places at once: aggregated on the landing page and inside each week's file under a <code>### Tasks</code> section. Use <strong>+ Add Task</strong> to append a new one; the pencil icon on a row edits it, the trash icon deletes it. Check a task and it moves under <strong>Completed</strong> in the same week's file.</p>
<ul>
  <li><strong>Rollover</strong> — incomplete tasks roll forward every Monday to the new week's file, so nothing gets lost when the week flips.</li>
  <li><strong>Auto-import</strong> — completed tasks flow into that week's Key Accomplishments once per day. Toggle it in <strong>Settings → Tasks</strong>; the <strong>Copy Completed</strong> button on /summary is the manual equivalent.</li>
</ul>

<h4>Due dates and task reminders</h4>
<p>Click the calendar icon on any task row to set (or clear) a due date. Rows show a <strong>Due</strong> chip — "Due today", "Due Fri", "Due Jul 15" — and overdue tasks surface under a red <strong>Overdue</strong> header sorted oldest-first. Rollover preserves the date, so a task that was due yesterday is still due yesterday next Monday.</p>
<p><strong>Settings → Tasks</strong> lets you enable task reminders, choose how many days before the due date to fire, and pick a time of day (default 09:00). Notifications use <strong>Noot</strong> as the icon, matching the weekly-reminder flow.</p>

<h3>Prep Self Review</h3>
<p>The <strong>Prep Self Review</strong> button on the landing page opens a wizard that assembles a markdown handoff doc for you to feed to an LLM (Claude, ChatGPT, whatever you use). Five steps:</p>
<ol>
  <li><strong>Confirm info</strong> — your name, role, and any framing you want to give.</li>
  <li><strong>Review period</strong> — pick the date range the doc should cover.</li>
  <li><strong>Questions</strong> — paste the self-review prompts you're being asked to answer.</li>
  <li><strong>OKRs</strong> — paste your current OKRs / goals.</li>
  <li><strong>Generate</strong> — the app stitches everything together, optionally including full Weekly Notes for the period (toggle on step 5), and gives you <strong>Save to Desktop</strong> and <strong>Copy to clipboard</strong> outputs.</li>
</ol>
<p>Captain's Log assembles the source material — it does not write your review. Hand the generated doc to your LLM of choice and iterate from there.</p>

<h3>Weekly reminders (and Noot)</h3>
<p><strong>Settings → Reminders</strong> lets you pick day(s) of the week (multi-select pills) and a time. macOS posts a native notification when the time arrives — click it to jump straight to /summary. Reminders automatically suppress for weeks you've already written a summary. The Reminders tab has a tip about switching macOS to "Persistent" Alert Style if you want the notification's action buttons to stay visible.</p>
<p>When a reminder is set, that's <strong>Noot</strong> — the little mascot under the week bar — parked at your reminder day and time so you can see at a glance when the nudge is coming. Noot's on loan from the Prodigy RPG. They don't track you, judge your notes, or report to management.</p>

<h3>Themes and colors</h3>
<p>Captain's Log ships with Light and Dark themes. <strong>Settings → Theme</strong> also lets you build a <strong>Custom</strong> palette — pick 12 primary colors (backgrounds, text, borders, accents) and the app derives the rest with WCAG AA contrast validation. Export your palette as a <code>.captheme.json</code> file to share with teammates or move between machines.</p>
<p><strong>Colorful Labels</strong> (a toggle in the Theme tab) gives each label its own auto-generated hue — same label always gets the same color, but the hue adapts to the active theme so a Dark-tuned color doesn't get baked into disk and become invisible under Light. You can override any label's color individually from the Labels tab.</p>
<p>If a Custom palette ever makes the in-app picker unreadable, right-click the menu-bar icon → <strong>Preset Theme</strong> → Dark or Light to escape.</p>

<h3>Keyboard shortcuts</h3>

<h4>Formatting (inside any editor)</h4>
<ul>
  <li><kbd>Cmd</kbd>+<kbd>B</kbd> — bold</li>
  <li><kbd>Cmd</kbd>+<kbd>I</kbd> — italic</li>
  <li><kbd>Cmd</kbd>+<kbd>Shift</kbd>+<kbd>X</kbd> — strikethrough</li>
  <li><kbd>Cmd</kbd>+<kbd>K</kbd> — link (inside an editor; outside an editor the same shortcut opens /search — see Navigation)</li>
  <li><kbd>Cmd</kbd>+<kbd>E</kbd> — inline code / fenced code block</li>
  <li><kbd>Cmd</kbd>+<kbd>Alt</kbd>+<kbd>0</kbd> — cycle heading level (none → H1 → H2 → H3)</li>
  <li><kbd>Cmd</kbd>+<kbd>Shift</kbd>+<kbd>8</kbd> — bullet list</li>
  <li><kbd>Cmd</kbd>+<kbd>Shift</kbd>+<kbd>7</kbd> — numbered list</li>
  <li><kbd>Cmd</kbd>+<kbd>Shift</kbd>+<kbd>L</kbd> — task list</li>
  <li><kbd>Cmd</kbd>+<kbd>Shift</kbd>+<kbd>9</kbd> — blockquote</li>
  <li><kbd>Cmd</kbd>+<kbd>;</kbd> — insert today's date</li>
</ul>

<h4>Editing</h4>
<ul>
  <li><kbd>Cmd</kbd>+<kbd>S</kbd> — save now (works on /journal and /summary)</li>
  <li><kbd>Cmd</kbd>+<kbd>Enter</kbd> — save now (/summary only)</li>
  <li><kbd>Esc</kbd> — dismisses whichever popup is on top (Send confirm, Preview, Label details, any confirmation) without touching the ones underneath</li>
  <li>Type three backticks on a line, then <kbd>Enter</kbd> — auto-expand a fenced code block</li>
</ul>

<h4>Navigation</h4>
<ul>
  <li><kbd>Cmd</kbd>+<kbd>Shift</kbd>+<kbd>S</kbd> — toggle Preview / Source mode (/journal only)</li>
  <li><kbd>Cmd</kbd>+<kbd>K</kbd> — open /search (main window only; inside an editor <kbd>Cmd</kbd>+<kbd>K</kbd> still inserts a link — the shortcut is contextual)</li>
</ul>

<h3>Things you can click that aren't obvious</h3>
<ul>
  <li>The thin orange bar at the very top is a week-progress indicator — it fills from Monday to Sunday. Not clickable, just a quiet status line.</li>
  <li>Rendered task checkboxes toggle when clicked.</li>
  <li>Any <code>YYYY-MM-DD</code> in your prose becomes a clickable date chip with a picker.</li>
  <li>Markdown links render as favicon+label <strong>link chips</strong>. Plain click opens the URL in your browser; <kbd>Alt</kbd>+click puts the <code>[text]</code> label in edit mode so you can rename it without touching the URL.</li>
  <li>Paste a URL <em>over a selection</em> and it wraps the selection as a link. Paste a URL <em>on its own</em> and it inserts, then async-upgrades to <code>[page title](url)</code> once the head-scrape returns. Works everywhere you can type a link — the MarkdownEditor surfaces (Notes, Summary, Journal), the + Add Task input, the inline task-edit input, and the Prep Self Review Questions / OKRs textareas.</li>
  <li>Individual label chips in the Labels tab open a details modal (rename, color override, delete).</li>
  <li>The <strong>?</strong> button on the editor toolbar opens a markdown cheat sheet in your browser.</li>
</ul>

<h3>Tips</h3>
<ul>
  <li>Capture small and often — a one-line note today beats a forgotten win in six months.</li>
  <li>Use /summary the week before a 1:1 or review cycle — it's built for exactly that moment.</li>
  <li>Everything lives locally on your machine. No cloud, no sync, no leaks.</li>
</ul>`;

export const NERDS_HTML = `<p>Captain's Log is a homebrew, single-developer project — built in odd hours by a QA who's a frontend tourist. Here's the stack, for the curious.</p>

<h3>Runtime shell</h3>
<p>The app is <a href="https://tauri.app" target="_blank" rel="noopener">Tauri 2.x</a> — a Rust core hosting a system WebView (WKWebView on macOS). Smaller than Electron, no bundled Chromium, and file I/O for your journal entries happens through Rust commands rather than a Node process. The Rust side is intentionally thin: read a file, write a file, list a directory, open a path in Finder.</p>

<h3>Frontend</h3>
<p>The UI is <a href="https://kit.svelte.dev" target="_blank" rel="noopener">SvelteKit</a> with the static adapter — no server, just a bag of HTML and JS that Tauri serves locally. State is managed with <a href="https://svelte.dev/docs/svelte/what-are-runes" target="_blank" rel="noopener">Svelte 5 runes</a> (<code>$state</code>, <code>$derived</code>, <code>$effect</code>). If you've used Svelte 4 stores, runes are the new hotness — finer-grained, less ceremony.</p>

<h3>The editor</h3>
<p>The journal pane is <a href="https://codemirror.net" target="_blank" rel="noopener">CodeMirror 6</a> with a custom decoration layer that does the live-preview trick. The markdown grammar is <a href="https://github.com/lezer-parser/markdown" target="_blank" rel="noopener">@lezer/markdown</a> with the <a href="https://github.github.com/gfm/" target="_blank" rel="noopener">GitHub-flavored extensions</a> enabled — tables, strikethrough, autolinks, task lists, the usual suspects. Setext-style headings (the <code>===</code> / <code>---</code> underline flavor) are explicitly disabled to keep a stray dash on the line below a paragraph from retroactively re-styling it as an H2.</p>
<p>The widget layer sitting on top of the buffer covers date chips, task checkboxes, bullet / ordered-list markers, and (Phase 4) link chips. Widgets use a content-only <code>eq()</code> so identical text re-uses the same DOM node across edits, and a live position-lookup so click handlers always dispatch to the right buffer range even after upstream edits shift offsets.</p>

<h3>Live-preview model</h3>
<p>The pattern is rich on top, canonical markdown underneath. Heading hashes, bold asterisks, link brackets — they're all still in the buffer. They're just hidden via CodeMirror's <code>Decoration.replace</code>, and the rendered styling is layered over the text via <code>Decoration.mark</code>. Move your cursor onto a line and the markers reappear so you can edit them. Move away and they vanish again. The file on disk is plain, portable markdown the whole time.</p>

<h3>Backend modules</h3>
<p>The Rust side (under <code>src-tauri/src/</code>) is organized by concern rather than by page:</p>
<ul>
  <li><code>storage.rs</code> — the <code>StorageBackend</code> trait plus a <code>LocalFilesystem</code> implementation. Every read and write to the journal flows through this trait, so a future Google Drive backend can slot in without touching callers.</li>
  <li><code>settings.rs</code> — the app-level and journal-level settings structs. <code>#[serde(default)]</code> on every field, so older settings files upgrade cleanly when new fields land.</li>
  <li><code>labels.rs</code> — parses inline <code>#labels</code> and explicit <code>**Labels:**</code> lines out of markdown, maintains the label index, and honors word-boundary rules that keep URLs (<code>#section</code>) from false-positiving.</li>
  <li><code>email.rs</code> + <code>email_html.rs</code> — build the send-to-manager payloads. Gmail and Outlook get URL-encoded compose URLs; Native Mac Mail gets an AppleScript block piped through <code>osascript</code>; the Styled HTML path emits a multipart <code>.eml</code>.</li>
  <li><code>reminders.rs</code> — the <a href="https://tokio.rs" target="_blank" rel="noopener">tokio</a> scheduler that fires weekly reminders. Re-derives the next fire instant from <code>chrono::Local::now()</code> on every wake so macOS hibernation can't strand it in the past.</li>
  <li><code>tasks.rs</code> — parses the <code>### Tasks</code> section out of each week file, handles the Monday rollover of incomplete tasks, owns the task sidecars (completions, due-dates, rollover-log, auto-import-log), and runs a second tokio scheduler for task reminders (N days before due at time Y).</li>
  <li><code>review_prep.rs</code> — assembles the Prep Self Review markdown handoff doc. Takes the wizard payload (info, review period, questions, OKRs, optional weekly notes) and produces the string that the frontend saves to Desktop or copies to the clipboard.</li>
  <li><code>link_enrich.rs</code> — the head-scrape service behind link chips. <a href="https://docs.rs/reqwest" target="_blank" rel="noopener">reqwest</a> fetches the target, <a href="https://docs.rs/scraper" target="_blank" rel="noopener">scraper</a> pulls <code>&lt;title&gt;</code> / <code>og:title</code> / favicon, and results are cached in <code>.metadata/link-cache.json</code> so a second paste of the same URL is instant and offline-safe.</li>
</ul>

<h3>Custom themes + the OKLCH walker</h3>
<p>Custom themes take 12 user-picked primary colors and derive ~23 dependent tokens — hover states, focus rings, subtle borders, error tints. The engine walks in <a href="https://oklch.com" target="_blank" rel="noopener">OKLCH</a> color space using <a href="https://culorijs.org" target="_blank" rel="noopener">culori</a> and iterates the L axis on each token until it hits WCAG AA contrast against its host surface (4.5:1 for text, 3:1 for UI). Non-convergent tokens fall back to better-of-black-or-white, so nothing paints below AA. Palettes serialize to a small <code>.captheme.json</code> you can share, version-control, or drop into Settings → Theme on a new machine.</p>

<h3>Storage</h3>
<p>Each ISO week is one <a href="https://commonmark.org" target="_blank" rel="noopener">CommonMark</a> + <a href="https://github.github.com/gfm/" target="_blank" rel="noopener">GFM</a> file at <code>YYYY/YYYY-Wnn.md</code> under your journal root. Alongside them a <code>.metadata/</code> folder holds:</p>
<ul>
  <li>the label index and journal-level settings;</li>
  <li>a sent-log for the manager-email flow and any in-flight capture draft;</li>
  <li>the task sidecars — <code>task-completions.json</code>, <code>task-due-dates.json</code>, <code>rollover-log.json</code>, <code>auto-import-log.json</code>;</li>
  <li><code>link-cache.json</code>, populated by <code>link_enrich.rs</code>;</li>
  <li><code>pre-slice6-backups/</code>, the pre-migration snapshots the Phase 3c task-restructure took of every week file before it rewrote them.</li>
</ul>
<p>All JSON, all rebuildable from the markdown if you ever delete them — the task sidecars re-derive from a fresh parse via <strong>Settings → Tasks → Rebuild task index</strong>, and the link cache repopulates itself from the next paste (or manual refresh). No database, no proprietary format, no lock-in — you can <code>cat</code> a year of journal from the terminal, grep it, sync it via iCloud or Git, or open it in any editor on earth. If Captain's Log disappears tomorrow, your data is fine.</p>
<p>Email HTML and inline task text share a small Markdown→HTML pipeline in Rust: <a href="https://github.com/pulldown-cmark/pulldown-cmark" target="_blank" rel="noopener">pulldown-cmark</a> renders the markdown, then <a href="https://github.com/rust-ammonia/ammonia" target="_blank" rel="noopener">ammonia</a> sanitizes the output so nothing user-authored can smuggle scripts or dangerous attributes into a rendered surface.</p>

<h3>Look and feel</h3>
<p>Body type is ABeeZee, a friendly humanist sans. Display headings use Paytone One. Both are self-hosted via <a href="https://fontsource.org" target="_blank" rel="noopener">@fontsource</a> npm packages (Paytone One + ABeeZee, <code>latin-ext-400</code> plus italic), so the app has no Google Fonts CDN dependency at runtime — everything ships inside the bundle. Code is rendered in the OS monospace stack (SF Mono on macOS). Icons live in a small <code>Icon.svelte</code> component that inlines hand-picked <a href="https://lucide.dev" target="_blank" rel="noopener">Lucide</a>-derived SVG paths — no icon-font runtime dependency.</p>

<h3>Rough edges</h3>
<p>Small quirks that haven't hit any real user yet, documented so you're not surprised if you do. All are one-liner fixes if they ever become a real complaint.</p>
<ul>
  <li><strong>Editor cursor near fenced code:</strong> Cmd+Home / Cmd+End / Cmd+F landing directly on a triple-backtick fence line and then arrowing can confuse the cursor-skip filter. Move a line up or down and it recovers.</li>
  <li><strong>IME composition + body-line-start backspace:</strong> the widget layer bails on this edge case rather than trying to handle every input-method state machine.</li>
  <li><strong>Multi-cursor + widget commands:</strong> most widget-aware commands (task toggle, chip commit) bail on multi-selection rather than dispatch per range. Regular typing / paste / delete work fine with multiple cursors.</li>
  <li><strong>Setext headings (<code>===</code> / <code>---</code> underlines):</strong> the live-preview active-state check doesn't detect them. The parser does, so on-disk markdown is still correct — only the "unhide markers when your cursor is on the line" cue is missing. ATX headings (<code>#</code> prefix) work fully.</li>
  <li><strong>Double-digit ordered-list markers:</strong> at 10+ items in an ordered list the marker column visually overlaps the content by a hair under the hang-indent layout. Fine at nine and below.</li>
  <li><strong>External writes during our own save:</strong> if another app modifies a week file at the exact millisecond Captain's Log is mid-save, one write wins and the other is dropped. Two-writer race inherent to plain files; the cross-route invalidation event mechanism can trigger a refresh but can't coordinate a merge.</li>
  <li><strong>Orphan task sidecar entries:</strong> if you hand-edit a week file outside the app to remove a task, its <code>task-completions.json</code> / <code>task-due-dates.json</code> entries linger. <strong>Settings → Tasks → Rebuild task index</strong> cleans them up.</li>
  <li><strong>Hand-typed content in <code>### Tasks</code>:</strong> the section is owned by the app — free-form notes typed directly inside it (as opposed to actual <code>- [ ]</code> task lines) can be clobbered by the next task write. Put freehand notes anywhere else in the week file.</li>
</ul>

<h3>Source</h3>
<p>Repo at <a href="https://github.com/SMARTeacher/captains-log" target="_blank" rel="noopener">github.com/SMARTeacher/captains-log</a>. Bug reports welcome; PRs accepted from anyone who wants to contribute.</p>`;
