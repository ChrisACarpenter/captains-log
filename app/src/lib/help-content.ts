/**
 * HTML body content for the Help and Nerds Only popups. Authored by the
 * help-popups-discovery workflow (parallel agents drafted content from
 * a survey of actual keyboard shortcuts + clickable affordances in the
 * codebase; synthesizer plugged in the real shortcut list).
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
  <li><strong>/summary</strong> — a structured weekly form (accomplishments, plans, challenges) that you can mail to your manager.</li>
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
<p>Look for the Captain's Log icon in your menu bar (top-right of the screen). <strong>Left-click</strong> it to open the capture popup from anywhere — even when the main window is hidden. Jot, save, get back to what you were doing. Right-click the icon for "Show Captain's Log" and "Quit".</p>

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
  <li><kbd>Esc</kbd> — dismiss the Send-confirmation dialog on /summary</li>
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
  <li>The <strong>?</strong> button on the editor toolbar opens a markdown cheat sheet in your browser.</li>
</ul>

<h3>About Noot</h3>
<p>That's Noot, they're here from the RPG to help. If you've set a reminder in Settings, Noot appears under the week bar at the position of your reminder day and time. Otherwise Noot stays off-stage. Noot doesn't track you, judge your notes, or report to management.</p>

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
<p>The journal pane is <a href="https://codemirror.net" target="_blank" rel="noopener">CodeMirror 6</a> with a custom decoration layer that does the live-preview trick. The markdown grammar is <a href="https://github.com/lezer-parser/markdown" target="_blank" rel="noopener">@lezer/markdown</a> with the <a href="https://github.github.com/gfm/" target="_blank" rel="noopener">GitHub-flavored extensions</a> enabled — tables, strikethrough, autolinks, task lists, the usual suspects.</p>

<h3>Live-preview model</h3>
<p>The pattern is rich on top, canonical markdown underneath. Heading hashes, bold asterisks, link brackets — they're all still in the buffer. They're just hidden via CodeMirror's <code>Decoration.replace</code>, and the rendered styling is layered over the text via <code>Decoration.mark</code>. Move your cursor onto a line and the markers reappear so you can edit them. Move away and they vanish again. The file on disk is plain, portable markdown the whole time.</p>

<h3>Storage</h3>
<p>Each ISO week is one <a href="https://commonmark.org" target="_blank" rel="noopener">CommonMark</a> + <a href="https://github.github.com/gfm/" target="_blank" rel="noopener">GFM</a> file at <code>YYYY/YYYY-Wnn.md</code> under your journal root. That's it. No database, no proprietary format, no lock-in — you can <code>cat</code> a year of journal from the terminal, grep it, sync it via iCloud or Git, or open it in any editor on earth. If Captain's Log disappears tomorrow, your data is fine.</p>

<h3>Look and feel</h3>
<p>Body type is <a href="https://fonts.google.com/specimen/ABeeZee" target="_blank" rel="noopener">ABeeZee</a>, a friendly humanist sans. Display headings use <a href="https://fonts.google.com/specimen/Paytone+One" target="_blank" rel="noopener">Paytone One</a>. Code is rendered in the OS monospace stack (SF Mono on macOS).</p>

<h3>Source</h3>
<p>Repo at <a href="https://github.com/ChrisACarpenter/captains-log" target="_blank" rel="noopener">github.com/ChrisACarpenter/captains-log</a>. Bug reports welcome; PRs accepted from anyone who wants to contribute.</p>`;
