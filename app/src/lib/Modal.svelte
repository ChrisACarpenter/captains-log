<!--
  Modal — the canonical popup shell for the whole app.

  Wraps every dim-blur backdrop + centered card pattern we've been
  re-implementing in each component (HelpButtons, LabelDetailsModal,
  SendToManagerButton's confirms, the Custom theme editor's import-pending
  guard, etc.). One file, one place to evolve the popup look + a11y.

  Mounting and dismissal:
    - Caller controls visibility with `open` boolean. When false this
      component renders absolutely nothing — no idle DOM, no leaked
      listeners.
    - Backdrop click → onClose.
    - Escape → onClose (window-level listener, attached only while open).
    - Body scroll locked while open so the page underneath doesn't shift
      when the user scrolls inside the popup or on the backdrop.

  Layering:
    - `zLayer="default"` (z-index 100) for the standard popup.
    - `zLayer="nested"` (z-index 200) for confirm-on-top-of-popup cases
      (ConfirmDialog uses this). Backdrop sits one rung lower than the
      card so the underlying popup stays partially visible but
      non-interactive while the inner confirm is up.

  Backdrop styling matches the Help / Nerds Only popup conventions Chris
  asked us to standardize on: rgba(0, 0, 0, 0.32) tint + 2px backdrop-
  filter blur. The card uses bg-elevated + structural border + a soft drop
  shadow, with a flex-column body the caller fills via the children snippet.

  Header is optional. Pass `title` to get the standard header bar with a
  close button; omit it to render a header-less card (ConfirmDialog goes
  this route — its title is part of the body).

  Accessibility:
    - role="dialog" + aria-modal="true" on the card
    - aria-labelledby points at the header's id when title is set; caller
      can supply its own labelledby via `ariaLabelledBy` if it renders a
      custom header inside the body
    - tabindex="-1" + auto-focus on mount so keyboard users land inside
    - Escape closes (standard dialog UX)

  Caller-controlled focus trap is intentionally NOT included — Tauri's
  webview is single-page and tab-cycling-out-of-the-popup is acceptable
  for the kinds of confirmations we ship. If we ever need a true trap,
  Svelte 5's bind:this + onMount makes it a one-component upgrade.

  Props:
    open                — boolean visibility gate.
    onClose             — called on backdrop click + Escape + (if title) the
                          close button. Caller flips `open` false here.
    title?              — header text. When set, renders the header bar
                          with title + × close button. Omit for a header-
                          less card (ConfirmDialog provides its own h2).
    ariaLabelledBy?     — id of an element the card should reference via
                          aria-labelledby. Defaults to the auto-generated
                          header id when title is set.
    zLayer?             — 'default' (z 100) | 'nested' (z 200). Default 'default'.
    maxWidth?           — CSS for max-width of the card. Default
                          `min(520px, calc(100vw - 32px))`.
    children            — body content snippet.
-->
<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import type { Snippet } from 'svelte';

  type Props = {
    open: boolean;
    onClose: () => void;
    title?: string;
    ariaLabelledBy?: string;
    zLayer?: 'default' | 'nested';
    maxWidth?: string;
    /**
     * When true, Escape / backdrop click / header close-button are all
     * no-ops. Callers set this during an in-flight IPC so the user can't
     * dismiss the modal while state is mid-transition. Distinct from
     * simply setting `open` false — the caller keeps the modal mounted;
     * we just refuse to *request* a close via any of the standard
     * dismissal affordances.
     */
    blockDismissal?: boolean;
    /**
     * When true, initial focus goes to the first enabled form control
     * (`<input>`, `<textarea>`, or `<select>`) inside the body instead
     * of the card itself. Use this for modals that exist to collect
     * input — otherwise the user has to Tab into the field before
     * typing. Default false preserves the "focus the card" behavior
     * ConfirmDialog and LabelDetailsModal already rely on.
     */
    focusFirstInput?: boolean;
    children: Snippet;
  };

  let {
    open,
    onClose,
    title,
    ariaLabelledBy,
    zLayer = 'default',
    maxWidth = 'min(520px, calc(100vw - 32px))',
    blockDismissal = false,
    focusFirstInput = false,
    children,
  }: Props = $props();

  // Auto-generated header id so multiple Modals on the page can each
  // reference their own title without callers having to coordinate ids.
  const myId = ++instanceCounter;
  const headerId = `modal-title-${myId}`;

  // Body scroll lock — applied via inline style on document.body so multiple
  // simultaneously-open modals don't fight each other (each pushes the lock
  // count up; lock releases when count returns to 0). Tracks with a module-
  // level WeakRef-ish counter rather than a class so SSR doesn't blow up.
  function applyScrollLock(): void {
    bodyLockCount += 1;
    if (bodyLockCount === 1 && typeof document !== 'undefined') {
      document.body.style.overflow = 'hidden';
    }
  }
  function releaseScrollLock(): void {
    bodyLockCount = Math.max(0, bodyLockCount - 1);
    if (bodyLockCount === 0 && typeof document !== 'undefined') {
      document.body.style.overflow = '';
    }
  }

  // Escape handler — registered only while open so a closed Modal doesn't
  // intercept Escape meant for something else. Only the topmost open
  // Modal in the stack handles Escape, so a nested ConfirmDialog dismisses
  // the inner confirm without also closing the parent.
  function onKeydown(e: KeyboardEvent): void {
    if (e.key !== 'Escape') return;
    if (modalStack[modalStack.length - 1] !== myId) return;
    if (blockDismissal) return;
    e.stopPropagation();
    onClose();
  }

  // Guarded dismiss entry point used by the backdrop + header close-×.
  // Escape has its own guard above (needs early-return before
  // stopPropagation).
  function requestClose(): void {
    if (blockDismissal) return;
    onClose();
  }

  let cardEl = $state<HTMLDivElement | null>(null);

  // Element that was focused right before the modal opened. On close
  // we return focus here so keyboard users don't lose their place in
  // the tab order — required by WCAG 2.1 SC 3.2.1 (On Focus) for
  // dialogs.
  let previouslyFocusedEl: HTMLElement | null = null;

  // Mount/unmount side effects keyed on `open`. Using $effect so the lock
  // + listener cycle correctly when callers toggle `open` repeatedly.
  $effect(() => {
    if (open) {
      previouslyFocusedEl =
        typeof document !== 'undefined' &&
        document.activeElement instanceof HTMLElement
          ? document.activeElement
          : null;
      applyScrollLock();
      modalStack.push(myId);
      window.addEventListener('keydown', onKeydown, true);
      // Defer focus to next microtask so the card is rendered before we
      // ask the browser to focus it. When `focusFirstInput` is set,
      // prefer the first enabled form control in the body — that's
      // what a data-entry modal wants (no Tab required to start
      // typing). Otherwise focus the card itself (matches the prior
      // behavior ConfirmDialog / LabelDetailsModal rely on).
      queueMicrotask(() => {
        if (focusFirstInput && cardEl) {
          const firstInput = cardEl.querySelector<HTMLElement>(
            'input:not([disabled]), textarea:not([disabled]), select:not([disabled])',
          );
          if (firstInput) {
            firstInput.focus();
            return;
          }
        }
        cardEl?.focus();
      });
      return () => {
        window.removeEventListener('keydown', onKeydown, true);
        const idx = modalStack.indexOf(myId);
        if (idx >= 0) modalStack.splice(idx, 1);
        releaseScrollLock();
        // Restore focus. Guard against the element having been removed
        // from the DOM between open + close (e.g. route change, parent
        // v-if flip) — in that case, silently drop.
        const target = previouslyFocusedEl;
        previouslyFocusedEl = null;
        if (target && document.contains(target)) {
          target.focus();
        }
      };
    }
  });

  onDestroy(() => {
    // Defensive: if the component is torn down while still open (route
    // change, parent v-if flips), make sure we don't leak the lock.
    if (open) releaseScrollLock();
  });
</script>

<script lang="ts" module>
  let instanceCounter = 0;
  let bodyLockCount = 0;
  // Stack of currently-open Modal ids in mount order. Only the topmost
  // handles Escape, so nested Modals (e.g. ConfirmDialog opened from
  // inside LabelDetailsModal) dismiss the inner one first.
  const modalStack: number[] = [];
</script>

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="modal-backdrop modal-{zLayer}"
    onclick={requestClose}
  ></div>
  <div
    class="modal-card modal-{zLayer}"
    bind:this={cardEl}
    role="dialog"
    aria-modal="true"
    aria-labelledby={ariaLabelledBy ?? (title ? headerId : undefined)}
    tabindex="-1"
    style:max-width={maxWidth}
  >
    {#if title}
      <header class="modal-header">
        <h2 id={headerId}>{title}</h2>
        <button
          type="button"
          class="modal-close"
          onclick={requestClose}
          aria-label="Close"
          disabled={blockDismissal}
        >×</button>
      </header>
    {/if}
    <div class="modal-body">
      {@render children()}
    </div>
  </div>
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.32);
    backdrop-filter: blur(2px);
  }

  /* Default + nested layers. Card sits one rung above its own backdrop so
   * a click on the backdrop dismisses, and a nested modal's backdrop dims
   * the parent popup (which sits at z=100). */
  .modal-default.modal-backdrop {
    z-index: 90;
  }
  .modal-default.modal-card {
    z-index: 100;
  }
  .modal-nested.modal-backdrop {
    z-index: 190;
  }
  .modal-nested.modal-card {
    z-index: 200;
  }

  .modal-card {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: 100%;
    max-height: calc(100vh - 64px);
    background: var(--bg-elevated);
    border: 1px solid var(--border-structural);
    border-radius: var(--radius-md);
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.28);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    outline: none;
    font-family: var(--font-body);
    color: var(--text-primary);
  }

  .modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--space-3) var(--space-4);
    border-bottom: 1px solid var(--border-structural);
    background: var(--bg-surface);
    flex-shrink: 0;
  }
  .modal-header h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 16px;
    font-weight: 600;
  }

  .modal-close {
    appearance: none;
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    font-size: 22px;
    line-height: 1;
    width: 28px;
    height: 28px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: background var(--transition-base), color var(--transition-base);
  }
  .modal-close:hover {
    background: var(--bg-elevated);
    color: var(--text-primary);
  }
  .modal-close:focus-visible {
    outline: 2px solid var(--focus-ring);
    outline-offset: 1px;
  }

  .modal-body {
    padding: var(--space-4);
    overflow-y: auto;
    color: var(--text-primary);
  }
</style>
