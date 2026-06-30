<!--
  PathPickerField — label + path input + "Browse…" button + hint
  trio, used wherever the app needs the user to pick a folder on
  their machine (onboarding step 4, /settings journal-location, and
  future backup/export destinations).

  Extracted out of StepSettings.svelte during the slice-4 cleanup so
  the markup + Tauri dialog plumbing live in one place. The component
  owns:

  - the .path-row flex layout (input grows, Browse button hugs right)
  - the @tauri-apps/plugin-dialog folder-picker invocation
  - hint + warning (error) microcopy below the row

  Visual treatment piggy-backs on the shared .text-input utility class
  in app.css (so it honors the per-context --input-bg variable, same
  as InputField does).

  ## Props

      id                   — DOM id. Forwarded to the <input> AND used
                             by the <label for=…> linkage.
      label                — visible label text above the input.
      value                — bindable. Two-way via `bind:value`. Set
                             to the chosen path when Browse succeeds.
      placeholder?         — input placeholder.
      hint?                — helper microcopy below the input.
      browseLabel?         — button text. Defaults to "Browse…".
      dialogTitle?         — passed to the Tauri open dialog as title.
      dialogDefaultPath?   — passed to the Tauri open dialog as the
                             starting directory. Falls back to the
                             current `value` when omitted.
      error?               — optional error/warning text shown below
                             the hint in the contrast-safe pink.
                             Cleared automatically when Browse picks
                             a folder.
-->
<script lang="ts">
  import { open as openDialog } from '@tauri-apps/plugin-dialog';

  type Props = {
    id: string;
    label: string;
    value: string;
    placeholder?: string;
    hint?: string;
    browseLabel?: string;
    dialogTitle?: string;
    dialogDefaultPath?: string;
    error?: string;
  };

  let {
    id,
    label,
    value = $bindable(),
    placeholder = '',
    hint,
    browseLabel = 'Browse…',
    dialogTitle,
    dialogDefaultPath,
    error,
  }: Props = $props();

  // Internal error from a failed Tauri dialog invocation. Kept separate
  // from the `error` prop so a caller's externally-driven error (e.g.
  // form-validation result) can coexist with — and not be clobbered by
  // — picker plumbing failures. Internal error wins when both are set.
  let dialogError = $state('');
  const displayError = $derived(dialogError || error || '');

  async function browse(): Promise<void> {
    // Clear any prior dialog error before re-attempting; if the dialog
    // throws we re-set it in the catch below.
    dialogError = '';
    try {
      const result = await openDialog({
        directory: true,
        multiple: false,
        defaultPath: dialogDefaultPath ?? value ?? undefined,
        title: dialogTitle,
      });
      if (typeof result === 'string' && result.length > 0) {
        value = result;
      }
      // User cancelled (null) — leave value as-is, error already cleared.
    } catch (err) {
      dialogError = String(err);
    }
  }
</script>

<div class="path-field">
  <label for={id}>{label}</label>
  <div class="path-row">
    <input
      {id}
      {placeholder}
      class="text-input path-input"
      type="text"
      spellcheck="false"
      autocomplete="off"
      bind:value
    />
    <button type="button" class="btn btn-marble btn-sm" onclick={browse}>
      {browseLabel}
    </button>
  </div>
  {#if hint}
    <p class="hint">{hint}</p>
  {/if}
  {#if displayError}
    <p class="hint hint-warning">{displayError}</p>
  {/if}
</div>

<style>
  /* Outer wrapper matches InputField's .field shape so PathPickerField
     slots into the same flex/gap rhythm the form steps use. */
  .path-field {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  label {
    font-family: var(--font-display);
    font-size: var(--text-button);
    color: var(--text-primary);
  }

  /* Input grows, Browse button hugs the right edge. */
  .path-row {
    display: flex;
    gap: var(--space-3);
    align-items: center;
  }
  .path-input {
    flex: 1;
    font-size: var(--text-caption);
  }

  .hint {
    margin: 0;
    font-size: var(--text-caption);
    line-height: var(--text-caption-lh);
    /* text-secondary (not text-muted) so the tone matches InputField hints.
     * Slice-5 verdict flagged the inconsistency as a polish item. */
    color: var(--text-secondary);
  }
  .hint-warning {
    color: var(--accent-pink-text);
  }
</style>
