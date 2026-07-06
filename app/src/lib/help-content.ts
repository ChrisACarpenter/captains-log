/**
 * HTML body content for the Help and Nerds Only popups. Authored by the
 * help-popups-discovery workflow (parallel agents drafted content from
 * a survey of actual keyboard shortcuts + clickable affordances in the
 * codebase; synthesizer plugged in the real shortcut list). Updated
 * 2026-07-06 to cover Phase 2.6–2.8c additions (Send-to-manager, Custom
 * themes, Colorful labels, multi-day reminders, Rust module layout).
 *
 * The strings are HTML, not markdown — rendered via Svelte's {@html ...}
 * inside HelpButtons.svelte's popup container. Update them here when
 * shortcuts change, when new buttons land, when the tech stack moves.
 */

export const HELP_HTML = `<h3>What is Captain's Log?</h3>
<p>A private, local-first journal for capturing your work as it happens — so performance reviews write themselves.</p>

<h3>The three surfaces</h3>
<ul>
  <li><strong>/capture</strong> — a fast jot box for in-the-moment notes; whatever you type lands in this week's journal.</li>
  <li><strong>/summary</strong> — a structured weekly form (key accomplishments, plans, challenges, anything else on your mind) that you can mail to your manager.</li>
  <li><strong>/journal</strong> — the full chronological log, browsable and editable by ISO week.</li>
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
  <li><kbd>Cmd</kbd>+<kbd>K</kbd> — link</li>
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
</ul>

<h3>Things you can click that aren't obvious</h3>
<ul>
  <li>The thin orange bar at the very top is a week-progress indicator — it fills from Monday to Sunday. Not clickable, just a quiet status line.</li>
  <li>Rendered task checkboxes toggle when clicked.</li>
  <li>Any <code>YYYY-MM-DD</code> in your prose becomes a clickable date chip with a picker.</li>
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
</ul>

<h3>Custom themes + the OKLCH walker</h3>
<p>Custom themes take 12 user-picked primary colors and derive ~23 dependent tokens — hover states, focus rings, subtle borders, error tints. The engine walks in <a href="https://oklch.com" target="_blank" rel="noopener">OKLCH</a> color space using <a href="https://culorijs.org" target="_blank" rel="noopener">culori</a> and iterates the L axis on each token until it hits WCAG AA contrast against its host surface (4.5:1 for text, 3:1 for UI). Non-convergent tokens fall back to better-of-black-or-white, so nothing paints below AA. Palettes serialize to a small <code>.captheme.json</code> you can share, version-control, or drop into Settings → Theme on a new machine.</p>

<h3>Storage</h3>
<p>Each ISO week is one <a href="https://commonmark.org" target="_blank" rel="noopener">CommonMark</a> + <a href="https://github.github.com/gfm/" target="_blank" rel="noopener">GFM</a> file at <code>YYYY/YYYY-Wnn.md</code> under your journal root. Alongside them a <code>.metadata/</code> folder holds the label index, journal-level settings, a sent-log for the manager-email flow, and any in-flight capture draft — all JSON, all rebuildable from the markdown if you ever delete them. No database, no proprietary format, no lock-in — you can <code>cat</code> a year of journal from the terminal, grep it, sync it via iCloud or Git, or open it in any editor on earth. If Captain's Log disappears tomorrow, your data is fine.</p>

<h3>Look and feel</h3>
<p>Body type is <a href="https://fonts.google.com/specimen/ABeeZee" target="_blank" rel="noopener">ABeeZee</a>, a friendly humanist sans. Display headings use <a href="https://fonts.google.com/specimen/Paytone+One" target="_blank" rel="noopener">Paytone One</a>. Code is rendered in the OS monospace stack (SF Mono on macOS). Icons live in a small <code>Icon.svelte</code> component that inlines hand-picked <a href="https://lucide.dev" target="_blank" rel="noopener">Lucide</a>-derived SVG paths — no icon-font runtime dependency.</p>

<h3>Source</h3>
<p>Repo at <a href="https://github.com/ChrisACarpenter/captains-log" target="_blank" rel="noopener">github.com/ChrisACarpenter/captains-log</a>. Bug reports welcome; PRs accepted from anyone who wants to contribute.</p>`;
