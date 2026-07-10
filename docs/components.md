# Captain's Log — Component Library

Index of the shared Svelte components under `app/src/lib/` and `app/src/lib/onboarding/`.

**This doc is a map, not the territory.** The canonical API contract for every component lives in the `<!-- ... -->` header comment at the top of each `.svelte` file — those headers are kept current as part of the component's normal maintenance. This index summarizes purpose + props + a usage example so a developer (or future Claude session) can find the right component without grep-fishing.

When the index and the in-file header disagree, **trust the header**. Open an issue or update the entry here.

## Quick reference

| Component | Category | Purpose |
|---|---|---|
| [ConfirmDialog](#confirmdialog) | Chrome | Yes/no confirmation modal that stacks cleanly above other modals without managing its own open state |
| [ExternalUpdateBanner](#externalupdatebanner) | Chrome | Render an unmissable banner warning users when a markdown file they're editing has been modified externally, with options to reload or dismiss |
| [HelpButtons](#helpbuttons) | Chrome | A pair of chrome-light buttons (Help, Nerds Only) fixed in the lower-left corner that open modal popups with help content when clicked |
| [LoadingOverlay](#loadingoverlay) | Chrome | Provides a reusable spinner overlay with message for long-running operations like saving or loading |
| [Modal](#modal) | Chrome | The canonical popup shell for the app—wraps every dim-blur backdrop + centered card pattern into one reusable, accessible dialog component |
| [PointerFinger](#pointerfinger) | Chrome | Onboarding guide that points to the next unfilled field with a gentle bobbing animation to draw user attention |
| [TipBubble](#tipbubble) | Chrome | A styled callout bubble that delivers in-context guidance with a consistent visual signal (info icon + bold heading + caption body) |
| [DatePickerPopover](#datepickerpopover) | Form | A hand-rolled date picker popover that anchors to a chip element, handles month/year navigation and keyboard input, and commits selected dates back to a CodeMirror 6 editor extension |
| [Checkbox](#checkbox) | Form | Canonical checkbox control with inline (glyph + one-line label) and card (heading + description) variants; source of truth for every checkbox in the app |
| [InputField](#inputfield) | Form | Standard "label + text input + hint" field component used across settings, capture, and onboarding steps |
| [LabelInput](#labelinput) | Form | Autocomplete tag/label input with chip display, filtering against a persisted label index, and optional per-label hex coloring |
| [PathPickerField](#pathpickerfield) | Form | A labeled path input field with a folder-picker button and optional hint/error messaging for directory selection tasks |
| [MarkdownEditor](#markdowneditor) | Editor | CodeMirror 6 wrapper for markdown editing that preserves source formatting byte-for-byte, with optional live preview and formatting toolbar |
| [MarkdownToolbar](#markdowntoolbar) | Editor | Formatting toolbar for MarkdownEditor that renders icon buttons for text styles (bold, italic, lists, etc.) and dispatches CodeMirror transactions |
| [RolloverReceipt](#rolloverreceipt) | Status | Transient pill announcing "Rolled over N tasks from last week" with polite aria-live and auto-dismiss after 5s |
| [SaveStatus](#savestatus) | Status | Small italic status indicator that surfaces the autosave state next to a Save or Done button |
| [WeekStripe](#weekstripe) | Status | A 4px fixed progress meter pinned to the top of the main window that displays the week elapsed and optional Noot mascot reminders |
| [Icon](#icon) | Atom | Render a themed inline SVG icon from a curated set of text-formatting, UI, and informational icons |
| [LabelDetailsModal](#labeldetailsmodal) | Feature | Modal popup for viewing and editing individual label details including usage stats, color customization, renaming, and deletion |
| [SendToManagerButton](#sendtomanagerbutton) | Feature | Send-to-Manager button + confirmation modal + sent-status display for sharing weekly summaries to the default mail client |
| [TaskMetaChip](#taskmetachip) | Feature | Pill chip for task-row metadata — provenance (origin), completed-at time (time), due date (due), or maroon overdue-due variant |
| [TaskRowActionButton](#taskrowactionbutton) | Feature | Inline pencil/trash/calendar action button on task rows, with variant-driven hover tint and bindable button element for popover anchoring |
| [StepHeader](#stepheader) | Onboarding | Renders a shared header block (with heading and optional lead text) for onboarding step pages |
| [Wizard](#wizard) | Onboarding | First-run onboarding wizard with five-step flow for capturing user info, manager details, and journal settings |
| [WizardFrame](#wizardframe) | Onboarding | Renders the visual chrome (card frame, Ed character, progress dots) shared across every onboarding wizard step |

## Chrome

_Popups, overlays, banners, and other UI chrome that wraps the rest of the app._

### ConfirmDialog

**Source:** [`ConfirmDialog.svelte`](app/src/lib/ConfirmDialog.svelte)

Yes/no confirmation modal that stacks cleanly above other modals without managing its own open state.

**When to use.** Use this when you need a confirmation dialog for user actions (e.g., delete, rename, destructive operations). Caller controls visibility with an {#if open} wrapper and manages the open flag alongside their own state.

**Props**

| Prop | Type | Required | Description |
|---|---|---|---|
| `title` | `string` | **yes** | Headline rendered as Modal's standard header. |
| `body` | `Snippet` | **yes** | Snippet rendering the body paragraph(s). Wrap in <p> and use <strong> for emphasis of identifiers. |
| `confirmLabel` | `string` | no | Text on the confirm button. Default: 'Confirm'. |
| `confirmVariant` | `'emerald' \| 'ruby'` | no | Button style. Use 'emerald' for affirmative (rename, save); 'ruby' for destructive (delete). Default: 'emerald'. |
| `cancelLabel` | `string` | no | Text on the cancel button. Default: 'Cancel'. |
| `cancelVariant` | `'marble' \| 'ruby'` | no | Button style. Default: 'marble' (neutral). |
| `onConfirm` | `() => void \| Promise<void>` | **yes** | Async callback fired when user clicks Confirm. |
| `onCancel` | `() => void` | **yes** | Sync callback fired on backdrop click, Escape key, or Cancel button click. Caller flips their open flag false here. |

**Usage**

```svelte
{#if showDeleteConfirm}
  <ConfirmDialog
    title="Delete item?"
    confirmLabel="Delete"
    confirmVariant="ruby"
    onConfirm={() => void deleteItem(itemId)}
    onCancel={() => (showDeleteConfirm = false)}
    body={deleteBody}
  />
{/if}

{#snippet deleteBody()}
  <p>Delete <strong>{itemName}</strong>? This cannot be undone.</p>
{/snippet}
```

**Related:** [Modal](#modal)

**Notes**

Thin wrapper over Modal with zLayer="nested" to stack cleanly above existing details/picker modals. Inherits Modal's backdrop dim+blur, body-scroll lock, focus-on-open, topmost-only-Escape, and a11y wiring. Body snippet allows inline HTML (strong tags for emphasis) without string-formatting hacks. Caller must manage visibility and open state externally — the component does not manage its own open flag.

### ExternalUpdateBanner

**Source:** [`ExternalUpdateBanner.svelte`](app/src/lib/ExternalUpdateBanner.svelte)

Render an unmissable banner warning users when a markdown file they're editing has been modified externally, with options to reload or dismiss.

**When to use.** Use this when the in-memory copy of a document is dirty and external changes have been detected, to prevent the user from accidentally overwriting those changes on their next save.

**Props**

| Prop | Type | Required | Description |
|---|---|---|---|
| `onReload` | `() => void` | **yes** | Callback fired when the user clicks the 'Reload (lose my edits)' button. |
| `onDismiss` | `() => void` | **yes** | Callback fired when the user clicks the dismiss (×) button. |
| `children` | `Snippet` | **yes** | The banner message body, rendered as default slot. Message wording differs per route (e.g., 'modified outside this view' or 'modified outside this view (likely from /journal or a quick-capture note)'). |

**Usage**

```svelte
<ExternalUpdateBanner
  onReload={() => location.reload()}
  onDismiss={() => dismissBanner = false}
>
  This file has been modified outside this view (likely from
  /journal or a quick-capture note). Reload to sync changes, or
  dismiss this warning.
</ExternalUpdateBanner>
```

**Notes**

A11y: Component includes role="status" and aria-live="polite" so screen readers announce the banner politely without interrupting the user. The dismiss button has aria-label="Dismiss warning" for proper screen reader context since the × glyph alone doesn't read. Earlier versions of this pattern were duplicated ~120 lines across /journal and /summary routes; this component consolidates them for maintainability.

### HelpButtons

**Source:** [`HelpButtons.svelte`](app/src/lib/HelpButtons.svelte)

A pair of chrome-light buttons (Help, Nerds Only) fixed in the lower-left corner that open modal popups with help content when clicked.

**When to use.** Use this when you need to provide contextual help and Easter egg (nerds-only) content accessible from any page via persistent fixed buttons with proper focus management and keyboard dismissal.

**Usage**

```svelte
<help-buttons />
```

**Related:** [Modal](#modal)

**Notes**

No props — fully self-contained. Anchored bottom-LEFT (not bottom-right) so scrollbar appearance on long routes can't shift the buttons. Content is pulled from `help-content.ts` at build time. Focus restored to the trigger button when the popup closes. Popup uses a full-viewport backdrop (z=200) so it can't be obscured by other floaters.

### LoadingOverlay

**Source:** [`LoadingOverlay.svelte`](app/src/lib/LoadingOverlay.svelte)

Provides a reusable spinner overlay with message for long-running operations like saving or loading.

**When to use.** Use this when you need to block interaction on a specific panel during a long-running operation (rebuild, save, import, etc.) while keeping the rest of the page interactive.

**Props**

| Prop | Type | Required | Description |
|---|---|---|---|
| `message` | `string` | no | Text shown below the spinner. Default: 'Loading…' |

**Usage**

```svelte
<div style="position: relative; height: 400px;">
  {#if isSaving}
    <LoadingOverlay message="Saving changes…" />
  {/if}
  <!-- rest of panel content -->
</div>
```

**Notes**

Positions absolutely within the nearest positioned ancestor (give the parent `position: relative`). Covers only its positioned container, not the whole page. Uses modal-overlay styling conventions (85% bg-elevated with 4px backdrop-blur, accent-orange spinner, decorative border). Accessible via role="status" + aria-live="polite"; spinner marked aria-hidden. Z-index defaults to 1 within its stacking context.

### Modal

**Source:** [`Modal.svelte`](app/src/lib/Modal.svelte)

The canonical popup shell for the app—wraps every dim-blur backdrop + centered card pattern into one reusable, accessible dialog component.

**When to use.** Use this when you need to display a modal dialog with a semi-transparent backdrop, centered card layout, optional header with close button, and automatic Escape/backdrop-click dismissal. It handles scroll locking and stacking for nested modals automatically.

**Props**

| Prop | Type | Required | Description |
|---|---|---|---|
| `open` | `boolean` | **yes** | Visibility gate. When false, renders nothing (no idle DOM, no leaked listeners). |
| `onClose` | `() => void` | **yes** | Called on backdrop click, Escape key, or close button click. Caller should flip `open` false here. |
| `title` | `string \| undefined` | no | Optional header text. When set, renders the header bar with title + × close button. Omit for a header-less card (e.g., ConfirmDialog which provides its own h2). |
| `ariaLabelledBy` | `string \| undefined` | no | Id of an element the card should reference via aria-labelledby. Defaults to the auto-generated header id when title is set. |
| `zLayer` | `'default' \| 'nested'` | no | Z-layer control. 'default' (z-index 100) for standard popups; 'nested' (z-index 200) for confirm-on-top-of-popup cases. Default 'default'. |
| `maxWidth` | `string` | no | CSS max-width of the card. Default 'min(520px, calc(100vw - 32px))'. |
| `children` | `Snippet` | **yes** | Body content snippet. The flex-column container the caller fills. |

**Usage**

```svelte
<Modal
  open={showPreview}
  onClose={closePreview}
  title="Weekly summary preview"
  maxWidth="min(640px, calc(100vw - 32px))"
>
  <p>Preview body content here…</p>
  <div class="modal-actions">
    <button class="btn btn-ruby" onclick={closePreview}>Close</button>
    <button class="btn btn-marble" onclick={copy}>Copy To Clipboard</button>
  </div>
</Modal>
```

**Related:** [ConfirmDialog](#confirmdialog)

**Notes**

Body scroll is automatically locked while open to prevent page shift; multiple open modals (stacked) do not fight each other. Escape dismisses only the topmost modal via a stack mechanism—nested modals close innermost-first. Focus automatically lands on the card on mount via queueMicrotask. Caller-controlled focus trap is intentionally NOT included (single-page Tauri webview context makes tab-out acceptable). Header id is auto-generated per instance, so multiple Modals can coexist on the page without id coordination.

### PointerFinger

**Source:** [`PointerFinger.svelte`](app/src/lib/PointerFinger.svelte)

Onboarding guide that points to the next unfilled field with a gentle bobbing animation to draw user attention.

**When to use.** Use this when you need to visually guide users during onboarding to highlight an unfilled form field or input that requires attention.

**Props**

| Prop | Type | Required | Description |
|---|---|---|---|
| `hidden` | `boolean` | no | When true, renders an empty spacer to keep flex layout stable without showing the pointer. Default false. |

**Usage**

```svelte
<div class="guide-row">
  <PointerFinger hidden={!isFieldEmpty} />
  <InputField id="name" label="Your name" bind:value={name} />
</div>
```

**Notes**

The component uses aria-hidden="true" since it's purely decorative. The hand asset at /branded/guide-hand.png ships pointing down-right and is rotated -15deg to point right. Animation is a 1.6s bobbing motion that shifts the element 4px horizontally at the midpoint. Callers are responsible for managing visibility logic based on their per-step form state (the component just renders or hides based on the hidden prop).

### TipBubble

**Source:** [`TipBubble.svelte`](app/src/lib/onboarding/TipBubble.svelte)

A styled callout bubble that delivers in-context guidance with a consistent visual signal (info icon + bold heading + caption body).

**When to use.** Use this when you need to surface non-blocking guidance, tips, or explanations within the app's page flow. Every tip should look and read identically—use this component to ensure consistency. If the guidance needs action buttons, hard-error severity, or different colors, use ExternalUpdateBanner instead.

**Props**

| Prop | Type | Required | Description |
|---|---|---|---|
| `heading` | `string` | no | The bold display-font label for the tip. Defaults to 'Heads up'. The component adds the trailing colon; pass the bare word. |
| `children` | `Snippet` | **yes** | The tip body content. Renders at caption size in secondary color. Supports <strong> (display-font emphasis, no bold), <a> and <button.link-button> (accent-primary-text, underlined), <code> (mono pill), and <em> (italic). |

**Usage**

```svelte
<TipBubble>
  Switch <em>Body delivery</em> above to
  <strong>Compose + paste</strong> for one-step paste-in.
</TipBubble>

<TipBubble heading="How delete works">
  Removes this label from every Note and Weekly Summary's labels
  list. Inline <code>#hashtag</code> text in note bodies is left
  alone — clean those up by hand if you want to.
</TipBubble>
```

**Related:** [ExternalUpdateBanner](#externalupdatebanner)

**Notes**

Body content rendering follows strict rules to ensure consistency across all tips in the app: <strong> renders in display font at normal weight (not bold) with primary text color; <a> and <button.link-button> use accent-primary-text (contrast-safe orange) with underline; <code> renders in mono on a subtle pill background; <em> uses italic at the caption size. These rules are enforced via :global() selectors in the component's style block, so callers cannot override font-size from outside the slot. Left border accent is 3px solid accent-primary.

## Form

_Form-field primitives — label + input + hint trios, chip-based label inputs, date pickers._

### Checkbox

**Source:** [`Checkbox.svelte`](app/src/lib/Checkbox.svelte)

Canonical checkbox control with inline (glyph + one-line label) and card (heading + description) variants; source of truth for every checkbox in the app.

**When to use.** Use this whenever you need a boolean toggle in a form or settings pane. The inline variant is the default and matches the `.checkbox-square` glyph used by CodeMirror task widgets and the landing-page task list. Opt into the card variant by supplying both `label` and `description` — it mirrors the radio cards on the Theme and Mail tabs so preference-heavy settings read as one control language.

**Props**

| Prop | Type | Required | Description |
|---|---|---|---|
| `checked` | `boolean` | **yes** | Bindable via bind:checked. Source of truth for the on/off state. |
| `onchange` | `(checked: boolean) => void` | no | Fires after each user toggle with the new value. Use for Set-based state where bind:checked can't round-trip through your data structure. |
| `disabled` | `boolean` | no | When true, clicks and Space/Enter are no-ops and the row dims. Defaults to false. |
| `ariaLabel` | `string` | no | Accessible name when no visible label text is provided (glyph-only usage like the task-row checkbox). |
| `label` | `string` | no | Heading text for the card variant, or a plain-text alternative to the `children` snippet in the inline variant. |
| `description` | `string` | no | Descriptive paragraph rendered under `label`. Presence of `description` is what switches the component into the card variant; without it, only the inline variant renders. |
| `children` | `Snippet` | no | Content rendered next to the square in the inline variant (allows inline markup like `<strong>` or `<em>`). Ignored in card variant. |

**Usage**

```svelte
<!-- Inline with bindable state: -->
<Checkbox bind:checked={reminderEnabled}>
  Send me a weekly reminder to fill in the Weekly Summary
</Checkbox>

<!-- Inline driven by external Set-based state: -->
<Checkbox
  checked={selectedNames.has(name)}
  onchange={() => toggleSelection(name)}
>
  {name}
</Checkbox>

<!-- Card variant — Settings > Tasks: -->
<Checkbox
  bind:checked={showCompleted}
  label="Show completed tasks"
  description="Keep finished tasks in view. Turn off to focus on what's left."
/>

<!-- Glyph-only (task row): -->
<Checkbox
  bind:checked={done}
  ariaLabel={done ? 'Mark not done' : 'Mark done'}
/>
```

**Notes**

Clicking anywhere on the row toggles state; focus lands on the button so Space/Enter work for keyboard users. The card variant is triggered by presence of `description` — `label` alone keeps the inline variant and just replaces the `children` snippet. Focus indicator hugs the inner `.checkbox-square` rather than the whole row so the ring reads consistently against both the task list and the padded card. Uses `role="checkbox"` + `aria-checked` on a `<button>` rather than a native `<input type="checkbox">` so the visual matches the CodeMirror task widget and landing-page task list — Chris flagged the drift between native inputs and the custom `.checkbox-square` glyph, and this component is the single destination every checkbox migrates to.

### DatePickerPopover

**Source:** [`DatePickerPopover.svelte`](app/src/lib/DatePickerPopover.svelte)

A hand-rolled date picker popover that anchors to a chip element, handles month/year navigation and keyboard input, and commits selected dates back to a CodeMirror 6 editor extension.

**When to use.** Use this when you need a calendar picker that integrates with CM6 editor extensions and want complete control over styling without external dependencies. The component opens anchored to a chip, auto-flips above if it would overflow the viewport, and supports full keyboard navigation.

**Props**

| Prop | Type | Required | Description |
|---|---|---|---|
| `iso` | `string` | **yes** | ISO YYYY-MM-DD date string currently shown by the chip. The picker opens pre-selected on this date. |
| `from` | `number` | **yes** | Document start position of the date token in the source. Passed back in onCommit so the editor extension knows what range to replace. |
| `to` | `number` | **yes** | Document end position of the date token in the source. Passed back in onCommit so the editor extension knows what range to replace. |
| `anchorEl` | `HTMLElement` | **yes** | The chip's DOM element. Used for positioning the popover via getBoundingClientRect() and for outside-click detection. |
| `onCommit` | `(newIso: string) => void` | **yes** | Callback fired when the user picks a day. The parent dispatches the editor transaction in response. |
| `onClose` | `() => void` | **yes** | Callback fired when the picker should close (outside-click, Escape key, or successful day pick). |

**Usage**

```svelte
{#if datePickerOpen}
  <DatePickerPopover
    iso="2026-06-30"
    from={42}
    to={52}
    anchorEl={chipElement}
    onCommit={(newIso) => applyDateEdit(newIso)}
    onClose={() => datePickerOpen = false}
  />
{/if}
```

**Notes**

The popover mounts fresh each time the user clicks a chip (gated by parent {#if}), so the initial `iso` is captured at mount-time and does not reactively follow the chip's source-of-truth while picking. Uses fixed positioning anchored via getBoundingClientRect(); if the popover would overflow the viewport bottom, it flips above the chip. In very tight viewports (e.g. Quick Capture popup), the popover may overlap the chip rather than render off-screen. Keyboard navigation includes arrow keys (day-by-day), PageUp/PageDown (month-by-month), Shift+PageUp/PageDown (year-by-year), and arrow navigation auto-spills over month boundaries. Outside-click detection uses `document.mousedown` (not click) to avoid race conditions with focus management. The component manages its own grid construction, month/year navigation, and today highlighting — no slots or children prop.

### InputField

**Source:** [`InputField.svelte`](app/src/lib/InputField.svelte)

Standard "label + text input + hint" field component used across settings, capture, and onboarding steps.

**When to use.** Use this when you need a complete form field with a label, text input, and optional hint or warning text below. It consolidates the ~15 verbatim repetitions of that shape into one reusable component.

**Props**

| Prop | Type | Required | Description |
|---|---|---|---|
| `id` | `string` | **yes** | DOM id forwarded to the <input> and used by the <label for=…> linkage. |
| `label` | `string` | no | Plain-text label rendered above the input. |
| `labelSnippet` | `Snippet` | no | Snippet variant of label for cases needing inline markup (e.g., links)—takes precedence over label prop. |
| `type` | `string` | no | HTML input type. Defaults to 'text'. |
| `placeholder` | `string` | no | Placeholder text. Defaults to empty string. |
| `value` | `string` | **yes** | Bindable via bind:value for two-way reactive binding. |
| `hint` | `string` | no | Optional helper microcopy below the input. |
| `hintSnippet` | `Snippet` | no | Snippet variant of hint for cases needing inline markup—takes precedence over hint prop when present. |
| `warning` | `string` | no | Optional warning text rendered in contrast-safe pink—when present, replaces hint and takes visual precedence. |
| `autocomplete` | `HTMLInputAttributes['autocomplete']` | no | HTML autocomplete hint. Defaults to 'off'. |
| `spellcheck` | `boolean` | no | Defaults to false (most form fields aren't prose). |

**Usage**

```svelte
<InputField
  id="job-title"
  label="Job title"
  placeholder="e.g. Senior Teacher"
  bind:value={jobTitle}
  hint="The job title on your employment record."
/>

<!-- With inline-markup hint via snippet: -->
{#snippet bambooHint()}
  See your <a href="https://prodigygame.bamboohr.com/">Bamboo</a> profile.
{/snippet}

<InputField
  id="job-title"
  label="Job title"
  bind:value={jobTitle}
  hintSnippet={bambooHint}
/>
```

**Related:** [TipBubble](#tipbubble)

**Notes**

Visual treatment (styling via .text-input utility class) lives in app.css and honors the per-context --input-bg CSS variable—this component only owns markup wiring and structure. The component header identifies three variants deliberately kept inline (path picker with Browse button, time input with max-width clamp, reminder day pills) because they need layout control or custom behavior that doesn't fit InputField's flow-block API. The spellcheck boolean converts to the string values 'true'/'false' for the HTML attribute. Both labelSnippet and hintSnippet take precedence over their plain-text counterparts when provided.

### LabelInput

**Source:** [`LabelInput.svelte`](app/src/lib/LabelInput.svelte)

Autocomplete tag/label input with chip display, filtering against a persisted label index, and optional per-label hex coloring.

**When to use.** Use this when you need users to add, remove, and autocomplete labels (tags) for an entry, with support for creating brand-new labels on-the-fly and a color-coded chip display.

**Props**

| Prop | Type | Required | Description |
|---|---|---|---|
| `labels` | `string[] (bindable)` | **yes** | Array of currently-added label names. Two-way binding — parent can read and write. Defaults to []. |
| `placeholder` | `string` | no | Placeholder text shown when no labels are present. Defaults to 'Add labels…'. |
| `colorfulLabels` | `boolean` | no | When true, chips and autocomplete options paint with per-label generated/persisted hex color. When false, uses the 5-color accent palette. Parent owns this so a global /settings toggle drives all callsites. Defaults to false. |

**Usage**

```svelte
<LabelInput
  bind:labels={noteLabels}
  placeholder="Add labels…"
  colorfulLabels={settings.colorfulLabels}
/>
```

**Notes**

1. Suggestions are loaded once on mount via Tauri IPC (invoke('get_labels')). New labels created in the component are appended optimistically to the local suggestions pool so subsequent typing sees them.

2. Color rendering uses a theme-change listener ('settings-changed' event) to refresh generated colors when theme switches without remounting. Theme reads (data-theme attribute, --bg-surface CSS var) are invisible to Svelte's reactivity, so a themeNonce is bumped to wire up the dependency.

3. Keyboard shortcuts: Arrow keys navigate options, Enter/Tab select, Backspace on empty input removes last chip, comma and space act as separators. Tab escapes the field naturally if no option is highlighted.

4. Label normalization: trailing/leading whitespace is trimmed, and leading # symbols are stripped. 'My Label' and '#My Label' both become 'My Label'.

5. Duplicates are excluded: the dropdown never suggests labels already in the array, and addLabel() silently rejects duplicates or empty names.

6. No component header comment — purpose and toggles inferred from code. The colorfulLabels toggle is meant to be owned by the parent (e.g., a /settings UI) so all LabelInput instances in the page respond uniformly to theme/color mode changes.

### PathPickerField

**Source:** [`PathPickerField.svelte`](app/src/lib/PathPickerField.svelte)

A labeled path input field with a folder-picker button and optional hint/error messaging for directory selection tasks.

**When to use.** Use this when you need the user to pick a folder on their machine (e.g., onboarding, settings, backup/export destination configuration).

**Props**

| Prop | Type | Required | Description |
|---|---|---|---|
| `id` | `string` | **yes** | DOM id, forwarded to the <input> and used by the <label for=…> linkage. |
| `label` | `string` | **yes** | Visible label text displayed above the input. |
| `value` | `string` | **yes** | Bindable via bind:value. Set to the chosen path when Browse succeeds. |
| `placeholder` | `string \| undefined` | no | Input placeholder text. Defaults to empty string. |
| `hint` | `string \| undefined` | no | Helper microcopy shown below the input. |
| `browseLabel` | `string` | no | Button text. Defaults to 'Browse…'. |
| `dialogTitle` | `string \| undefined` | no | Passed to the Tauri open dialog as the title. |
| `dialogDefaultPath` | `string \| undefined` | no | Passed to the Tauri open dialog as the starting directory. Falls back to the current value when omitted. |
| `error` | `string \| undefined` | no | Optional error/warning text shown below the hint in contrast-safe pink. Cleared automatically when Browse picks a folder. |

**Usage**

```svelte
<PathPickerField
  id="root"
  label="Folder"
  bind:value={journalRoot}
  hint="Plain markdown on your machine."
/>
```

**Related:** [InputField](#inputfield)

**Notes**

The component maintains separate internal error state (dialogError) from the external error prop so form-validation errors and Tauri dialog failures can coexist without clobbering each other; internal dialog error takes priority in display. The Browse button invokes @tauri-apps/plugin-dialog's openDialog in directory mode. Visual treatment reuses the shared .text-input utility class from app.css and honors the per-context --input-bg variable.

## Editor

_CodeMirror-based prose editor + its formatting toolbar._

### MarkdownEditor

**Source:** [`MarkdownEditor.svelte`](app/src/lib/MarkdownEditor.svelte)

CodeMirror 6 wrapper for markdown editing that preserves source formatting byte-for-byte, with optional live preview and formatting toolbar.

**When to use.** Use this when you need a markdown editor that maintains exact formatting on disk (no smart-quote mutations or heading-style normalization), supports keyboard formatting shortcuts and live preview, and integrates with your design system's color tokens.

**Props**

| Prop | Type | Required | Description |
|---|---|---|---|
| `value` | `string` | no | Initial markdown content. ONE-WAY binding — not $bindable because CM6 transactions own the doc. External reloads only; push changes back via onChange. |
| `onChange` | `(next: string) => void` | **yes** | Fires on every doc-changing transaction with the full current document string. Plug into consumer's existing $effect debounce for auto-save. |
| `placeholder` | `string` | no | Placeholder text shown when editor is empty. Default: empty string. |
| `class` | `string` | no | CSS class passed through to the wrapper .md-editor div. Default: empty string. |
| `style` | `string` | no | Inline styles for the wrapper. Supports CSS variables: --md-font-family, --md-font-size, --md-line-height, --md-min-height, --md-padding. Default: empty string. |
| `autofocus` | `boolean` | no | Focus the editor on mount. Default: false. |
| `id` | `string` | no | Optional DOM id forwarded to the inner .cm-content element so <label for={id}> clicks focus the editor. Default: undefined. |
| `showToolbar` | `boolean` | no | Show formatting toolbar (Bold / Italic / lists / link / etc.) above the editor. Defaults to true; set to false on /journal (raw-markdown surface where toolbar would be off-message). |
| `livePreview` | `boolean` | no | Slack/Typora-style live preview — hides markdown markers (**, *, ~~, #, -, >, etc.) so user sees rendered rich text. Buffer on disk stays canonical markdown. Defaults to false (source mode). Phase 2.5 Architecture B opts /capture and /summary into this; /journal stays raw-source. |

**Usage**

```svelte
<MarkdownEditor
  value={body}
  onChange={(next) => (body = next)}
  placeholder="What did you just do?"
  autofocus
  livePreview
  id="note-body"
/>
```

**Related:** [MarkdownToolbar](#markdowntoolbar), [DatePickerPopover](#datepickerpopover)

**Notes**

value is ONE-WAY only — do not use $bindable. CM6 transactions own the document buffer; re-setting on every keystroke fights the transaction model and resets cursor. Echo-loop guard in $effect prevents onChange from triggering parent state updates that reflip value. The component imports GFM (GitHub Flavored Markdown) with Setext headings disabled to avoid list-vs-heading parse collisions. Spell-check is on by default (WebKit native, no IPC). Task checkboxes, inline date chips, and code blocks render as live-preview decorations (hidden in source mode). CSS selector reach-across via :global() to CodeMirror's internal DOM tree (.cm-editor, .cm-scroller, .cm-content, etc.) for theming. Focus ring, caret color, and selection background use design-system accent colors.

### MarkdownToolbar

**Source:** [`MarkdownToolbar.svelte`](app/src/lib/MarkdownToolbar.svelte)

Formatting toolbar for MarkdownEditor that renders icon buttons for text styles (bold, italic, lists, etc.) and dispatches CodeMirror transactions.

**When to use.** Use this when you need a per-editor markdown formatting toolbar above a CodeMirror instance. Each button shares the same command implementation as the keyboard shortcuts, so toolbar clicks and Cmd+B/I/K/E/etc. have no drift.

**Props**

| Prop | Type | Required | Description |
|---|---|---|---|
| `view` | `EditorView \| undefined` | **yes** | The CodeMirror EditorView this toolbar dispatches formatting commands into. Owned by the parent MarkdownEditor; passed down once it mounts. Undefined before mount. |
| `updateTick` | `number` | **yes** | A monotonic counter bumped by MarkdownEditor on every cursor move or document change. Acts as a Svelte $derived reactivity dependency so the pressed-state classes (is-active) stay in sync with the syntax tree at the cursor. |

**Usage**

```svelte
<!-- Normally rendered by MarkdownEditor when showToolbar is true; if you
     ever consume it directly, pass the CodeMirror EditorView reference
     and the updateTick counter the parent bumps on every transaction. -->
<MarkdownToolbar {view} {updateTick} />
```

**Related:** [MarkdownEditor](#markdowneditor), [Icon](#icon)

**Notes**

Per-editor design (not floating/shared) is intentional—multi-editor pages (four editors on /summary) need clear target coupling. The `activeFormats` derived state pulls from the CodeMirror syntax tree on every updateTick, so buttons light up (.is-active) immediately as the cursor enters formatted regions. The help button is visually separated from formatting buttons by a spacer and opens the markdown guide in the system browser. Roving-tabindex a11y pattern is deferred; v1 has plain tab order. Every command calls view.focus() internally to reclaim editor focus after clicks.

## Status

_State indicators that surface where the user is or what just happened._

### RolloverReceipt

**Source:** [`RolloverReceipt.svelte`](app/src/lib/RolloverReceipt.svelte)

Transient pill announcing "Rolled over N tasks from last week" with polite aria-live and auto-dismiss after 5s.

**When to use.** Use this to acknowledge an automatic task rollover the moment it completes. Mount it conditionally when `tasksCopied > 0` — the receipt does not gate itself on the count. Pair it with a parent that clears its own state in `onDismiss` so the receipt doesn't re-mount if the trigger fires again in the same session.

**Props**

| Prop | Type | Required | Description |
|---|---|---|---|
| `tasksCopied` | `number` | **yes** | How many tasks moved. Rendered as a pluralized count ("1 task" / "2 tasks"). Zero means "don't render at all" — parent should conditional-mount rather than pass 0. |
| `sourceLabel` | `string` | **yes** | Human-readable name for where the tasks came from ("last week", "Week 27, 2026"). Kept as a plain string so the parent can localize or vary the format. |
| `onDismiss` | `() => void` | no | Called on manual close OR the 5s auto-timeout. Parent clears its own trigger state here. |

**Usage**

```svelte
{#if rolloverCount > 0}
  <RolloverReceipt
    tasksCopied={rolloverCount}
    sourceLabel="last week"
    onDismiss={() => (rolloverCount = 0)}
  />
{/if}
```

**Notes**

Uses `role="status"` + `aria-live="polite"` + `aria-atomic="true"` so screen readers announce the count and tail as one utterance without interrupting the user. Auto-dismiss timer is a 5s `setTimeout` cleared on manual close or destroy; the effect only runs once since the props that would retrigger it (`tasksCopied`, `sourceLabel`) don't change during the receipt's lifetime — the parent remounts on new rollovers. The numeric count renders in `--accent-primary-text` to echo the "checked N ago" chip elsewhere, signaling quantitative info without leaning on color alone. Border uses `--border-structural` rather than the orange-tinted `--border-decorative` so the pill reads against `--bg-elevated` in both themes.

### SaveStatus

**Source:** [`SaveStatus.svelte`](app/src/lib/SaveStatus.svelte)

Small italic status indicator that surfaces the autosave state next to a Save or Done button.

**When to use.** Use this when you need to display the current autosave state (idle, dirty, saving, saved, or error) to the user, typically positioned near save/submit controls in forms or editors.

**Props**

| Prop | Type | Required | Description |
|---|---|---|---|
| `status` | `'idle' \| 'dirty' \| 'saving' \| 'saved' \| 'error'` | **yes** | The current save state. idle renders nothing; dirty shows 'Unsaved changes'; saving shows savingText; saved shows timestamp; error shows errorText. |
| `lastSavedAt` | `Date \| null` | **yes** | Date object for the last successful save. Used to format the 'saved' text as 'Saved HH:MM AM/PM'. Pass null when there's no recent save. |
| `onRetry` | `(() => void) \| undefined` | no | Optional click handler. When set, the error state renders as a clickable button; when omitted, error renders as plain text. |
| `savingText` | `string` | no | Text to display in saving state. Default: 'Saving…'. Capture overrides to 'Saving draft…'. |
| `savedPrefix` | `string` | no | Prefix for the saved state text. Default: 'Saved'. Capture overrides to 'Draft saved'. |
| `errorText` | `string` | no | Text to display in error state. Default: 'Couldn't save — retry?'. Capture overrides to 'Couldn't save draft'. |

**Usage**

```svelte
<SaveStatus
  status={saveStatus}
  lastSavedAt={savedAt}
  onRetry={() => void saveNow()}
/>

<!-- Capture's draft-flavored copy: -->
<SaveStatus
  status={draftStatus}
  lastSavedAt={draftSavedAt}
  savingText="Saving draft…"
  savedPrefix="Draft saved"
  errorText="Couldn't save draft"
/>
```

**Notes**

The idle state renders nothing (empty). The error state only becomes clickable if onRetry is provided; otherwise it degrades to a non-interactive span. Time is formatted in user's locale (en-US) as 'h:MM AM/PM'. WCAG AA contrast compliance: error color uses --accent-pink-text instead of raw --accent-pink due to insufficient contrast at 13px on bg-base. This component consolidated three near-identical local .save-status blocks previously scattered across journal, summary, and capture routes.

### WeekStripe

**Source:** [`WeekStripe.svelte`](app/src/lib/WeekStripe.svelte)

A 4px fixed progress meter pinned to the top of the main window that displays the week elapsed and optional Noot mascot reminders.

**When to use.** Use this as a singleton header bar in the main app layout to show week progress at a glance and render reminder mascots at their scheduled times. Pair it with routes that listen for the custom 'captainslog:week-changed' event to refresh cached data when the week rolls over.

**Usage**

```svelte
<!-- Rendered once at the top of +layout.svelte for every route except
     /capture. Self-contained — no props, no slots. -->
<WeekStripe />
```

**Notes**

Component has no public props — it is entirely self-contained, retrieving settings via Tauri's invoke('get_settings') and listening to Tauri's 'settings-changed' event. Requires CSS variables --stripe-track and --stripe-fill to be defined at :root level. Updates every 60 seconds; also re-fetches when the backend emits 'settings-changed', so reminder toggles appear instantly. Dispatches a custom 'captainslog:week-changed' event at window scope when ISO week rolls over, used by routes to invalidate cached data. The fill width uses a 600ms ease-out transition for smooth visual feedback.

## Atom

_Lowest-level visual primitives._

### Icon

**Source:** [`Icon.svelte`](app/src/lib/Icon.svelte)

Render a themed inline SVG icon from a curated set of text-formatting, UI, and informational icons.

**When to use.** Use this when you need a compact icon in a formatting toolbar or UI control (like bold, italic, lists, help, or calendar). The component responds to CSS color properties, making it ideal for hover and pressed states.

**Props**

| Prop | Type | Required | Description |
|---|---|---|---|
| `name` | `'heading' \| 'bold' \| 'italic' \| 'strikethrough' \| 'list' \| 'list-ordered' \| 'list-checks' \| 'quote' \| 'link' \| 'code' \| 'calendar' \| 'help' \| 'info' \| 'search' \| 'pencil' \| 'trash' \| 'check'` | **yes** | The icon to render. Must be one of the 17 supported icon names. |
| `size` | `number` | no | Icon width and height in pixels. Default 18px (formatting toolbar size); pass 24 for Lucide-standard sizing. |

**Usage**

```svelte
<button class="md-toolbar-btn" onclick={() => run(toggleBold)}>
  <Icon name="bold" />
</button>

<!-- Larger inline icon (tip-bubble header, callouts): -->
<Icon name="info" size={16} />

<!-- Task-row action glyphs (rendered inside TaskRowActionButton at size={14}): -->
<TaskRowActionButton icon="pencil" variant="neutral" ariaLabel="Edit task" title="Edit" onclick={edit} />
<TaskRowActionButton icon="trash" variant="destructive" ariaLabel="Delete task" title="Delete" onclick={remove} />
<TaskRowActionButton icon="calendar" variant="accent" ariaLabel="Set due date" title="Set due date" onclick={openPicker} />
```

**Notes**

Uses currentColor for stroke, so icon color is controlled via CSS color property on the parent element. The component includes aria-hidden="true" since icons are typically paired with text labels. All icons use 24x24 viewBox with 2px stroke width and round caps/joins (Lucide convention) to maintain consistent visual weight. To add a new icon, add a case arm in the if/else chain with matching stroke attributes.

## Feature

_Composite components that own a whole user-facing feature._

### LabelDetailsModal

**Source:** [`LabelDetailsModal.svelte`](app/src/lib/LabelDetailsModal.svelte)

Modal popup for viewing and editing individual label details including usage stats, color customization, renaming, and deletion.

**When to use.** Use this when a user clicks a label row in Settings → Labels to open an editable details panel showing stats, color override, rename input, and a destructive delete action.

**Props**

| Prop | Type | Required | Description |
|---|---|---|---|
| `label` | `{ name: string; count: number; firstUsed: string; lastUsed: string; color?: string \| null }` | **yes** | The label to display and edit. Contains name, usage count, timestamps, and optional persisted color override. |
| `colorfulLabels` | `boolean` | **yes** | Feature flag; when true, shows the Color section (28×28 swatch, hex input, reset button). When false, section is hidden. |
| `theme` | `'light' \| 'dark' \| 'custom'` | **yes** | Current theme; used to generate the auto label color and for chip styling. |
| `bgSurface` | `string` | no | Optional background surface color (e.g., hex), passed to generateLabelColor and chipStyleFor for theme-aware color generation. |
| `onClose` | `() => void` | **yes** | Callback fired when the user closes the modal (Escape, backdrop click, or Close button). |
| `onLabelMutated` | `() => void` | **yes** | Callback fired after a successful color change, rename, or delete. Parent refreshes its label list. On delete, onClose() is called first, then this fires. |

**Usage**

```svelte
<LabelDetailsModal
  label={{ name: 'urgent', count: 42, firstUsed: '2024-01-15', lastUsed: '2024-06-20', color: '#ff0000' }}
  colorfulLabels={true}
  theme="light"
  bgSurface="#ffffff"
  onClose={() => selectedLabel = null}
  onLabelMutated={() => refreshLabels()}
/>
```

**Related:** [Modal](#modal), [ConfirmDialog](#confirmdialog), [TipBubble](#tipbubble)

**Notes**

Color input commits ON BLUR only (not keystroke) per design decision #5 — prevents half-typed values from triggering backend writes. Rename validation mirrors Rust's is_label_char rule (alphanumeric + '_' + '-'). Delete is destructive and removes label from labels arrays in Notes and Weekly Summaries; inline #hashtag text in prose is left untouched. Stats load asynchronously on mount; Color/Rename/Delete sections remain functional even if stats fail. The modal owns focus management and body-scroll lock via its parent Modal wrapper.

### SendToManagerButton

**Source:** [`SendToManagerButton.svelte`](app/src/lib/SendToManagerButton.svelte)

Send-to-Manager button + confirmation modal + sent-status display for sharing weekly summaries to the default mail client.

**When to use.** Use this when you need a one-click handoff to email for sending a weekly summary, with confirmation modal, preview capability, and sent-status tracking. Place it in an actions row; the status text line renders separately in the parent's layout.

**Props**

| Prop | Type | Required | Description |
|---|---|---|---|
| `year` | `number` | **yes** | ISO year of the week being sent. |
| `week` | `number` | **yes** | ISO week number being sent. |
| `weekLabel` | `string` | **yes** | Human-readable week label (e.g., 'Week of June 22 – June 28, 2026') used in modal copy. |
| `isDirty` | `boolean` | **yes** | Has-unsaved-edits flag from parent's form state; blocks Send until resolved. |
| `saveStatus` | `'idle' \| 'dirty' \| 'saving' \| 'saved' \| 'error'` | **yes** | Current save status; 'saving' state blocks the Send button. |
| `sentStatusText` | `string` | no | Bindable output — read-only text like 'Sent Jun 26 at 4:12 PM' or 'Last sent … (edited since)'. Parent owns placement; defaults to empty string. |
| `sentStatusIsStale` | `boolean` | no | Bindable output — true when content has been edited after sending, used to flag the status line as stale. Defaults to false. |

**Usage**

```svelte
<div class="actions-area">
  <p class="sent-status" class:is-stale={sentIsStale}>{sentText}</p>
  <div class="actions">
    <SaveStatus status={saveStatus} lastSavedAt={savedAt} />
    <SendToManagerButton
      year={iso.year}
      week={iso.week}
      weekLabel={weekLabel}
      isDirty={dirty}
      {saveStatus}
      bind:sentStatusText={sentText}
      bind:sentStatusIsStale={sentIsStale}
    />
    <button class="btn btn-emerald" onclick={() => void saveNow()}>Save</button>
  </div>
</div>
```

**Related:** [Modal](#modal), [TipBubble](#tipbubble)

**Notes**

Owns all internal gating (dirty check, save-in-progress, already-sent-unchanged, re-send detection). Re-fetches sent-record + content hash on mount and whenever year/week changes. Listens to Tauri events (weekly-file-changed, settings-changed) to invalidate state if other routes edit the same week. Supports three mail send modes (Gmail web URL, native Mac Mail with AppleScript, Outlook web) and two body-delivery strategies (prefilled vs. clipboard-paste). Preview modal renders HTML iframe or plaintext depending on send mode. Gmail URLs triggering truncation warning show a warn-and-allow flow (Send anyway / Cancel) instead of blocking. The parent is responsible for placing the sentStatusText output line wherever its layout calls for, not inside this component.

### TaskMetaChip

**Source:** [`TaskMetaChip.svelte`](app/src/lib/TaskMetaChip.svelte)

Pill chip for task-row metadata — provenance (origin), completed-at time (time), due date (due), or maroon overdue-due variant.

**When to use.** Use this on the trailing edge of a task row wherever a small piece of metadata used to live as an inline `.task-origin` / `.task-time` / `.task-due-chip` span. Four variants collapse those cases into one component; the caller picks the variant that describes what the label means.

**Props**

| Prop | Type | Required | Description |
|---|---|---|---|
| `variant` | `'origin' \| 'time' \| 'due' \| 'due-overdue'` | **yes** | Chip flavor. `origin` is italic accent-primary-text ("from W26") with no chip background — the Slice 5 provenance cue. `time` is muted with `tabular-nums` so "checked 4h ago" and "checked 11h ago" line up. `due` is accent-tinted for on-time due dates on incomplete rows. `due-overdue` is a maroon-tinted bold pill (uses the theme-adjusted `--brand-maroon-text` token) rendered when `due_date < today`. |
| `label` | `string` | **yes** | The chip text. |
| `title` | `string` | no | Native tooltip on hover. Defaults to empty string. |

**Usage**

```svelte
<!-- Slice 5 provenance on a rolled-over task: -->
<TaskMetaChip variant="origin" label="from W26" title="Rolled over from Week 26, 2026" />

<!-- Completed-at timestamp: -->
<TaskMetaChip variant="time" label="checked 2h ago" />

<!-- On-time due date: -->
<TaskMetaChip variant="due" label="Due Jul 15" />

<!-- Overdue on an incomplete row: -->
<TaskMetaChip variant="due-overdue" label="Due Jul 5" title="Overdue since Jul 5" />
```

**Related:** [TaskRowActionButton](#taskrowactionbutton)

**Notes**

All variants share `flex-shrink: 0` + `--text-caption` size + `white-space: nowrap` so a multi-chip row stays coherent as widths change. The `due-overdue` variant reinforces the "Overdue" group header on the landing page — the chip is a per-row signal, the header is the group signal, both drive the same reading. `--brand-maroon-text` is the theme-adjusted maroon (lighter red in dark theme, base maroon in light) — using the raw `--brand-maroon` here would fail contrast in dark mode.

### TaskRowActionButton

**Source:** [`TaskRowActionButton.svelte`](app/src/lib/TaskRowActionButton.svelte)

Inline pencil/trash/calendar action button on task rows, with variant-driven hover tint and bindable button element for popover anchoring.

**When to use.** Use this for any 24×24 action glyph that sits on the trailing edge of a landing-page task row. It absorbs the previously-duplicated `.task-edit-btn` / `.task-delete-btn` / `.task-due-btn` markup and CSS — three near-identical Svelte + style blocks collapse into one component. The `variant` prop drives the hover/focus tint; the icon prop picks the glyph.

**Props**

| Prop | Type | Required | Description |
|---|---|---|---|
| `icon` | `'pencil' \| 'trash' \| 'calendar'` | **yes** | The glyph to render. Must map to an [Icon](#icon) name; adding a new glyph means extending both this union and Icon's `IconName` union. |
| `variant` | `'neutral' \| 'destructive' \| 'accent'` | no | Hover/focus tint. `neutral` = muted text/edge (edit). `destructive` = maroon tint (delete). `accent` = accent-primary tint (due date). Defaults to `'neutral'`. |
| `ariaLabel` | `string` | **yes** | Accessible name for the button (glyph-only, so aria-label carries meaning). |
| `title` | `string` | **yes** | Native tooltip on hover. |
| `disabled` | `boolean` | no | When true, click and focus are no-ops and opacity drops to 0.25. Callers own the disable logic (e.g. row + other-row-editing + modal-open state combined). Defaults to false. |
| `onclick` | `(e: MouseEvent) => void` | **yes** | Click handler. Receives the DOM event so callers can `stopPropagation` from the surrounding row click. |
| `el` | `HTMLButtonElement \| null` | no | Bindable DOM handle. Required for the calendar action so the parent can anchor [DatePickerPopover](#datepickerpopover) to this button via `bind:el`. |

**Usage**

```svelte
<TaskRowActionButton
  icon="pencil"
  variant="neutral"
  ariaLabel="Edit task"
  title="Edit"
  onclick={(e) => { e.stopPropagation(); startEdit(); }}
/>

<TaskRowActionButton
  icon="trash"
  variant="destructive"
  ariaLabel="Delete task"
  title="Delete"
  disabled={otherRowEditing}
  onclick={(e) => { e.stopPropagation(); confirmDelete(); }}
/>

<!-- Calendar action anchors DatePickerPopover to its own <button>: -->
<TaskRowActionButton
  icon="calendar"
  variant="accent"
  ariaLabel="Set due date"
  title="Set due date"
  bind:el={dueBtnEl}
  onclick={openDuePicker}
/>
{#if duePickerOpen && dueBtnEl}
  <DatePickerPopover
    iso={currentIso}
    from={0}
    to={0}
    anchorEl={dueBtnEl}
    onCommit={applyDue}
    onClose={() => (duePickerOpen = false)}
  />
{/if}
```

**Related:** [Icon](#icon), [TaskMetaChip](#taskmetachip), [DatePickerPopover](#datepickerpopover)

**Notes**

Rest state is muted (opacity 0.55, no border) so a row full of chrome doesn't dominate the eye; the variant tint only paints on hover and `:focus-visible`. Callers still own the disabled logic — the button just consumes the boolean. The `el` handle is `$bindable()` because the calendar action needs a real DOM reference to anchor DatePickerPopover; without it, the popover has nothing to position against. Icons render at `size={14}` inside the 24×24 button so the glyph reads at the small scale.

## Onboarding

_First-run wizard shell + its shared step header._

### StepHeader

**Source:** [`StepHeader.svelte`](app/src/lib/onboarding/StepHeader.svelte)

Renders a shared header block (with heading and optional lead text) for onboarding step pages.

**When to use.** Use this when every onboarding step needs a consistent "<header><h1|h2/><p class='lead'/></header>" pattern with appropriate heading level and spacing — avoids duplicating the structure and CSS in each step component.

**Props**

| Prop | Type | Required | Description |
|---|---|---|---|
| `title` | `string` | **yes** | The main heading text. |
| `level` | `'h1' \| 'h2'` | no | Heading level: h1 for Intro/Complete pages (top-of-flow), h2 for form steps (default: 'h2'). |
| `lead` | `string` | no | Plain-string subtitle text, rendered in a <p class='lead'> with secondary text color. |
| `leadSnippet` | `Snippet` | no | Snippet for future steps that need inline markup in the subtitle (links, <strong>, etc.). Mutually exclusive with lead — wins if both are passed. |

**Usage**

```svelte
<!-- Intro / Complete pages (top-of-flow): -->
<StepHeader title="Welcome" level="h1" lead="Let's get you set up." />

<!-- Form steps in the middle (default h2): -->
<StepHeader title="Tell me about you" lead="A few details we'll use later." />

<!-- Inline markup in the lead via snippet: -->
{#snippet bambooLead()}
  Your <a href="https://prodigygame.bamboohr.com/">Bamboo</a> details, optional.
{/snippet}
<StepHeader title="Job details" leadSnippet={bambooLead} />
```

**Notes**

The .lead margin-bottom differs by heading level in intent (h1 uses space-4 conceptually before form steps; h2 uses space-6 before form fields), but the CSS applies space-6 uniformly to all .lead elements to maintain pre-refactor parity with the Welcome and All-set screens. Revisit spacing if a tighter h1 rhythm is desired.

### Wizard

**Source:** [`Wizard.svelte`](app/src/lib/onboarding/Wizard.svelte)

First-run onboarding wizard with five-step flow for capturing user info, manager details, and journal settings.

**When to use.** Use this when you need to guide a user through initial setup with intro, personal details, manager information, settings configuration, and a completion celebration screen.

**Props**

| Prop | Type | Required | Description |
|---|---|---|---|
| `defaultJournalRoot` | `string` | **yes** | OS-appropriate suggested journal root (fetched before wizard mounts). Pre-fills step 4's path field. |
| `onComplete` | `() => void` | **yes** | Called after the user clicks the final action button. Parent should refetch settings and rerender as normal mode. |

**Usage**

```svelte
<Wizard
  defaultJournalRoot="/Users/you/Documents"
  onComplete={() => {
    // refetch settings, exit wizard mode
    location.reload();
  }}
/>
```

**Related:** [WizardFrame](#wizardframe), [StepHeader](#stepheader). Wizard's per-step internals (`StepIntro`, `StepAboutYou`, `StepAboutManager`, `StepSettings`, `StepComplete`) live under `app/src/lib/onboarding/` but aren't reusable outside this flow, so they're not indexed here.

**Notes**

Step 4 (Settings) requires a non-empty journalRoot; all other fields are optional and freely skippable. Persistence happens on step 4's "Finish setup" button (calls complete_first_run Tauri command). The wizard stays mounted through step 5 (celebration) — parent does not refetch settings until user clicks the final action button, so firstRun doesn't flip prematurely. Saving errors display in an alert banner within the wizard.

### WizardFrame

**Source:** [`WizardFrame.svelte`](app/src/lib/onboarding/WizardFrame.svelte)

Renders the visual chrome (card frame, Ed character, progress dots) shared across every onboarding wizard step.

**When to use.** Use this when building a step in the onboarding wizard flow that needs consistent card styling, decorative Ed character positioning, and step-progress indicators. The parent Wizard component owns the Back/Continue buttons, so WizardFrame handles only the frame chrome and content container.

**Props**

| Prop | Type | Required | Description |
|---|---|---|---|
| `edImageSrc` | `string` | **yes** | Path of the Ed image to render in the bottom-left corner. Usually `/branded/ed-NN.png`. |
| `edImageAlt` | `string` | no | Alt text for the Ed image. Kept empty for purely decorative usage, but can be overridden if a step wants to surface meaning. Default: empty string. |
| `step` | `number` | **yes** | Current step number, 1-based. Used to highlight the active dot in the steps indicator. |
| `totalSteps` | `number` | **yes** | Total number of stops in the flow. Used to render the correct number of indicator dots. |
| `showIndicator` | `boolean` | no | Whether to render the steps indicator (the dot progression). Final celebration step hides it. Default: true. |
| `children` | `Snippet` | **yes** | The step content (headings, fields, tip bubbles). Rendered inside the card's content area above the steps indicator. |

**Usage**

```svelte
<WizardFrame
  edImageSrc="/branded/ed-02.png"
  edImageAlt=""
  step={2}
  totalSteps={5}
  showIndicator={true}
>
  <h2>Pick your character</h2>
  <p>Choose your favorite creature to adventure with.</p>
  <!-- form fields go here -->
</WizardFrame>
```

**Related:** [Wizard](#wizard)

**Notes**

The steps indicator is always rendered in the DOM but uses `visibility: hidden` on the final step (when `showIndicator={false}`) rather than conditional rendering. This keeps Ed's bottom-left positioning and the action-row baseline consistent across all steps. Ed's vertical center is aligned to the button row's center at `bottom: 40px`; if the indicator's footprint changed, Ed would shift vertically and misalign. The frame sets `--input-bg: var(--bg-base)` to override the shared `.text-input` utility so form fields have proper contrast against the card surface.

---

## Conventions for adding a new component

1. **Header comment first.** Every component starts with a `<!-- ... -->` block describing purpose, when-to-use, props, usage example, and any gotchas. This is the canonical contract — write it before the implementation.
2. **Add an entry here.** Drop a new section in the right category. Keep the purpose to one sentence; let the in-file header carry the depth.
3. **Cross-link.** If the new component pairs with an existing one (e.g. extends `Modal`, mirrors `InputField`), list it under **Related** on both ends.
4. **No prop-list duplication beyond this index.** If you need a richer spec, expand the in-file header — don't fork the contract across two files.

