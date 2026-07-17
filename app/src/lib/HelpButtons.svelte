<!--
  Help + Nerds Only popup buttons anchored in the lower-LEFT corner.

  Two small chrome-light buttons; clicking either opens a modal-style
  popup with the relevant content. The HTML content lives in
  ./help-content.ts so the discovery workflow's drafts can be updated
  there without touching the rendering shell.

  Placement is bottom-LEFT (was bottom-right in an earlier draft) so
  the scrollbar appearing on long routes can't shift the buttons.

  ## Layering
  - Fixed at bottom: 12px, left: 12px.
  - z-index: 60 — below WeekStripe + Noot (z=100) but above page
    content (default 0) and above the cat (z=50).
  - Each popup uses a full-viewport backdrop (z=200) so it can't be
    obscured by other floaters.

  ## Dismissal
  - Click backdrop → close.
  - Esc key → close.
  - Close (×) button in popup top-right → close.

  ## A11y
  - Buttons have aria-haspopup="dialog" + aria-expanded reflecting
    open state.
  - Popup container has role="dialog" + aria-modal="true" + a
    labelled name via aria-labelledby on the popup's h2 (mounted with
    a deterministic id).
  - Focus moves to the popup on open; restores to the trigger button
    on close.
-->
<script lang="ts">
  import { onDestroy } from 'svelte';
  import { HELP_HTML, NERDS_HTML } from './help-content';
  import Modal from '$lib/Modal.svelte';

  type PopupKind = 'help' | 'nerds' | null;

  let openKind = $state<PopupKind>(null);
  let lastTriggerBtn: HTMLButtonElement | null = null;

  function openPopup(kind: 'help' | 'nerds', triggerBtn: HTMLButtonElement): void {
    lastTriggerBtn = triggerBtn;
    openKind = kind;
    // Modal handles focus on open via its own queueMicrotask hop.
  }

  function closePopup(): void {
    openKind = null;
    // Restore focus to the trigger so keyboard users land back where
    // they were rather than at the top of the document.
    lastTriggerBtn?.focus();
    lastTriggerBtn = null;
  }

  // Escape + click-backdrop both flow through Modal's onClose now —
  // no manual window keydown listener needed.

  onDestroy(() => {
    lastTriggerBtn = null;
  });

  const popupTitle = $derived(openKind === 'help' ? 'Help' : 'Nerds Only');
  const popupBody = $derived(openKind === 'help' ? HELP_HTML : NERDS_HTML);
</script>

<div class="help-buttons" role="group" aria-label="Help and About">
  <button
    type="button"
    class="help-btn"
    aria-haspopup="dialog"
    aria-expanded={openKind === 'help'}
    onclick={(e) => openPopup('help', e.currentTarget)}
  >Help</button>
  <button
    type="button"
    class="help-btn help-btn-nerds"
    aria-haspopup="dialog"
    aria-expanded={openKind === 'nerds'}
    onclick={(e) => openPopup('nerds', e.currentTarget)}
  >Nerds Only</button>
</div>

{#if openKind}
  <Modal open={true} onClose={closePopup} title={popupTitle}>
    <div class="help-popup-body">
      {@html popupBody}
    </div>
  </Modal>
{/if}

<style>
  .help-buttons {
    position: fixed;
    bottom: 12px;
    /* Anchored to the LEFT edge so a vertical scrollbar appearing/
     * disappearing on the page doesn't shift the buttons. Cat is at
     * top-left; help buttons are at bottom-left — different vertical
     * positions, no overlap. */
    left: 12px;
    z-index: 60;
    display: inline-flex;
    gap: 6px;
    align-items: center;
  }

  .help-btn {
    appearance: none;
    background: var(--bg-elevated);
    border: 1px solid var(--border-structural);
    border-radius: var(--radius-pill);
    /* text-muted on bg-elevated at 11px = 3.83:1, fails AA. text-secondary
       clears 5.41:1 while still reading as quieter chrome than primary. */
    color: var(--text-secondary);
    font: inherit;
    font-size: 11px;
    font-weight: 500;
    padding: 4px 10px;
    cursor: pointer;
    transition:
      background var(--duration-fast) var(--ease-standard),
      color var(--duration-fast) var(--ease-standard),
      border-color var(--duration-fast) var(--ease-standard);
  }
  .help-btn:hover {
    background: var(--bg-surface);
    color: var(--text-primary);
    border-color: var(--accent-primary);
  }
  .help-btn:focus-visible {
    outline: none;
    box-shadow: 0 0 0 2px var(--focus-glow);
    border-color: var(--accent-primary);
    color: var(--text-primary);
  }

  /* The Nerds variant gets a faint italic tint so the two buttons read
   * as a pair-of-different-things rather than a single segmented control. */
  .help-btn-nerds {
    font-style: italic;
  }

  /* Backdrop + popup chrome + header bar + close button live in
   * $lib/Modal.svelte now — this file only styles the body content
   * (.help-popup-body below). */

  .help-popup-body {
    padding: var(--space-4);
    overflow-y: auto;
    line-height: 1.55;
    font-size: 14px;
  }

  /* The body content is generated HTML — style child elements via
   * :global so the rules reach the {@html ...} subtree. */
  .help-popup-body :global(h3) {
    margin: var(--space-4) 0 var(--space-2);
    font-size: 14px;
    font-weight: 700;
    color: var(--text-primary);
  }
  .help-popup-body :global(h3:first-child) {
    margin-top: 0;
  }
  .help-popup-body :global(h4) {
    margin: var(--space-3) 0 var(--space-1);
    font-size: 13px;
    font-weight: 600;
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .help-popup-body :global(p) {
    margin: 0 0 var(--space-2);
  }
  .help-popup-body :global(ul) {
    margin: 0 0 var(--space-2);
    padding-left: 18px;
  }
  .help-popup-body :global(li) {
    margin: 2px 0;
  }
  .help-popup-body :global(a) {
    /* Contrast-safe variant — raw accent-primary on bg-elevated at 14px
       only hits 3.77:1, failing AA for normal text. */
    color: var(--accent-primary-text);
    text-decoration: underline;
  }
  .help-popup-body :global(a:hover) {
    text-decoration: none;
  }
  .help-popup-body :global(code) {
    font-family: ui-monospace, "SF Mono", SFMono-Regular, Menlo, monospace;
    font-size: 0.9em;
    background: var(--bg-surface);
    padding: 1px 4px;
    border-radius: 3px;
  }
  .help-popup-body :global(strong) {
    font-weight: 600;
  }
</style>
