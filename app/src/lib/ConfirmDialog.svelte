<!--
  ConfirmDialog — yes/no confirmation modal.

  Thin wrapper over `$lib/Modal.svelte` with `zLayer="nested"` so it stacks
  cleanly above an existing details/picker modal (e.g. the LabelDetails-
  Modal's Rename + Delete confirms). Inherits Modal's backdrop dim + blur,
  body-scroll lock, focus-on-open, topmost-only-Escape, and a11y wiring.

  Caller controls visibility with an {#if open} wrapper. The component
  doesn't manage its own open flag — callers handle that alongside their
  own state (showDeleteConfirm, etc.).

  Snippet API for the body so callers can include inline `<strong>` for
  emphasized identifiers ("Delete **foo**? Used **3 times**.") without
  string-formatting hacks.

  Props:
    title          — string headline. Renders as Modal's standard header.
    body           — snippet rendering the body paragraph(s). Wrap in <p>
                     and use <strong> for emphasis.
    confirmLabel   — string. Default "Confirm".
    confirmVariant — 'emerald' | 'ruby'. Drives the Confirm button class.
                     Default 'emerald' for affirmative actions (rename,
                     save); use 'ruby' for destructive (delete).
    cancelLabel    — string. Default "Cancel".
    cancelVariant  — 'marble' | 'ruby'. Default 'marble' (neutral).
    onConfirm      — async callback. Called when the user clicks Confirm.
    onCancel       — sync callback. Called for backdrop click + Escape +
                     the Cancel button click. Callers flip their open
                     flag false here.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import Modal from '$lib/Modal.svelte';

  type Variant = 'emerald' | 'ruby' | 'marble';

  type Props = {
    title: string;
    body: Snippet;
    confirmLabel?: string;
    confirmVariant?: Extract<Variant, 'emerald' | 'ruby'>;
    cancelLabel?: string;
    cancelVariant?: Extract<Variant, 'marble' | 'ruby'>;
    onConfirm: () => void | Promise<void>;
    onCancel: () => void;
  };

  let {
    title,
    body,
    confirmLabel = 'Confirm',
    confirmVariant = 'emerald',
    cancelLabel = 'Cancel',
    cancelVariant = 'marble',
    onConfirm,
    onCancel,
  }: Props = $props();
</script>

<Modal
  open={true}
  onClose={onCancel}
  {title}
  zLayer="nested"
  maxWidth="min(420px, calc(100vw - 32px))"
>
  <div class="confirm-body">
    {@render body()}
  </div>
  <div class="confirm-actions">
    <button class="btn btn-{confirmVariant} btn-sm" onclick={() => void onConfirm()}>
      {confirmLabel}
    </button>
    <button class="btn btn-{cancelVariant} btn-sm" onclick={onCancel}>
      {cancelLabel}
    </button>
  </div>
</Modal>

<style>
  .confirm-body {
    margin: 0 0 var(--space-4);
    color: var(--text-secondary);
    line-height: var(--text-body-lh);
  }

  .confirm-body :global(p) {
    margin: 0 0 var(--space-3);
  }

  .confirm-body :global(p:last-child) {
    margin-bottom: 0;
  }

  .confirm-body :global(strong) {
    color: var(--text-primary);
    font-weight: normal;
    font-family: var(--font-display);
  }

  .confirm-actions {
    display: flex;
    gap: var(--space-3);
    justify-content: flex-end;
  }
</style>
