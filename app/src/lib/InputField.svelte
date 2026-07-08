<!--
  InputField — the standard "label + text input + hint" trio used
  across /settings, /capture, and the three onboarding steps. Folds
  ~15 verbatim repetitions of that shape into one component.

  Visual treatment lives in the shared `.text-input` utility class in
  app.css (which honors the per-context --input-bg CSS variable). This
  component only owns the markup wiring + the per-field structure
  ($state binding, hint variants, optional warning).

  ## Props

      id           — DOM id. Forwarded to the <input> AND used by the
                     <label for=…> linkage.
      label        — text or Snippet rendered above the input.
      type         — HTML input type. Defaults to "text".
      placeholder  — placeholder text. Defaults to empty.
      value        — bindable. Two-way binding via `bind:value`.
      hint         — optional helper microcopy below the input.
      warning      — optional warning text. When present, replaces the
                     hint and renders in the contrast-safe pink.
      autocomplete — HTML autocomplete hint.
      spellcheck   — defaults to false (most form fields aren't prose).

  ## Composite variants the audit identified as out-of-scope

  - **Path picker** (input + Browse… button): /settings + StepSettings
    render their journal-location field as a flex .path-row with a
    paired button. The button needs to live in the parent, so those
    sites use the raw <input class="text-input"> shape directly.
  - **Time input** (max-width: 160px): /settings reminders tab uses
    type="time" with an explicit max-width to keep the native picker
    from dominating the row. The width clamp doesn't fit InputField's
    flow-block API, so that site also stays raw.
  - **Reminder day pills**: a custom row of toggle buttons, not a field.

  All three are fine to leave inline.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';

  type Props = {
    id: string;
    /** Plain-text label, OR a snippet for cases that need inline links
     *  (e.g. the "Job title — the one on Bamboo" label that links out). */
    label?: string;
    labelSnippet?: Snippet;
    type?: string;
    placeholder?: string;
    value: string;
    hint?: string;
    /** Snippet variant of hint, for cases where the helper text needs
     *  inline markup (links, <strong> tokens, etc.). When present, takes
     *  precedence over the plain-string `hint` prop. */
    hintSnippet?: Snippet;
    warning?: string;
    autocomplete?: import('svelte/elements').HTMLInputAttributes['autocomplete'];
    spellcheck?: boolean;
  };

  let {
    id,
    label,
    labelSnippet,
    type = 'text',
    placeholder = '',
    value = $bindable(),
    hint,
    hintSnippet,
    warning,
    autocomplete = 'off',
    spellcheck = false,
  }: Props = $props();
</script>

<div class="field">
  <label for={id}>
    {#if labelSnippet}{@render labelSnippet()}{:else}{label}{/if}
  </label>
  <input
    {id}
    {type}
    {placeholder}
    {autocomplete}
    spellcheck={spellcheck ? 'true' : 'false'}
    class="text-input"
    aria-describedby={warning ? `${id}-warning` : undefined}
    aria-invalid={warning ? 'true' : undefined}
    bind:value
  />
  {#if warning}
    <!--
      role="alert" + aria-live="assertive" so screen readers announce
      validation errors as soon as they render. aria-describedby on the
      input above wires the input to this element so navigating back to
      the field re-announces the error.
    -->
    <p
      id="{id}-warning"
      class="field-hint is-warning"
      role="alert"
      aria-live="assertive"
    >
      {warning}
    </p>
  {:else if hintSnippet}
    <p class="field-hint">{@render hintSnippet()}</p>
  {:else if hint}
    <p class="field-hint">{hint}</p>
  {/if}
</div>

<style>
  .field {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  /* Display-font label. Same shape /settings + onboarding both use. */
  label {
    font-family: var(--font-display);
    font-size: var(--text-button);
    color: var(--text-primary);
  }
  /* Inline links inside a label (e.g. the "Bamboo" link in StepAboutYou)
     pick up the contrast-safe orange variant. */
  label :global(a) {
    color: var(--accent-primary-text);
    text-decoration: underline;
  }
  label :global(a:hover) {
    filter: brightness(1.1);
  }

  .field-hint {
    margin: 0;
    font-size: var(--text-caption);
    line-height: var(--text-caption-lh);
    color: var(--text-secondary);
  }
  .field-hint.is-warning {
    color: var(--accent-pink-text);
  }
  /* Same display-font-on-strong trick the TipBubble uses. */
  .field-hint :global(strong) {
    font-family: var(--font-display);
    color: var(--text-primary);
    font-weight: normal;
  }
</style>
