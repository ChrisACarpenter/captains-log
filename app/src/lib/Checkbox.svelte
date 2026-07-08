<!--
  Checkbox — the canonical checkbox control across the app.

  Two visual variants share this component:

  1. **Inline** (default): full-width button with the `.checkbox-square`
     glyph followed by a single line of label text. Used everywhere a
     boolean is a one-liner — Settings > Reminders, Settings > Mail,
     Settings > Theme (label multi-select), Onboarding, etc.

  2. **Card**: padded capsule with a border, heading, and descriptive
     paragraph underneath. Matches the Theme tab's Dark/Light/Custom
     radio cards and the Mail tab's Prefilled/Compose radio cards so
     preference-heavy tabs read as a unified control language. Opt in
     by supplying BOTH `label` AND `description` props. Used by the
     Settings > Tasks tab.

  Clicking anywhere on the row toggles state. Focus lands on the
  button so keyboard users can Space/Enter. Hover / focus / checked
  styling on the inner square is driven by the parent button's state.

  ## Why not a native <input type="checkbox">?

  We already ship `.checkbox-square` in the CodeMirror task widget
  and the landing-page task list. Chris flagged the inconsistency
  between that visual and the native inputs in the Settings tabs —
  this component is the single destination every checkbox migrates
  to.

  ## Usage

  Inline with a bindable state:

      <Checkbox bind:checked={reminderEnabled}>
        Send me a weekly reminder to fill in the Weekly Summary
      </Checkbox>

  Inline driven by external state (Set / derived value):

      <Checkbox
        checked={selectedNames.has(name)}
        onchange={() => toggleSelection(name)}
      >
        {name}
      </Checkbox>

  Card variant (heading + description):

      <Checkbox
        bind:checked={showCompleted}
        label="Show completed tasks"
        description="Keep finished tasks in view. Turn off to focus on what's left."
      />

  Standalone glyph-only (aria-label carries meaning):

      <Checkbox
        bind:checked={done}
        ariaLabel={done ? 'Mark not done' : 'Mark done'}
      />

  ## Props

    checked      — bindable boolean; source of truth for state.
    onchange?    — optional callback fired after each user toggle with
                   the new boolean value. Use for Set-based state where
                   `bind:checked` can't round-trip through your data.
    disabled?    — when true, clicks + keypresses are no-ops; row dims.
    ariaLabel?   — accessible name when no visible label text is
                   provided.
    label?       — the heading text (card variant) or a plain text
                   alternative to the `children` snippet (inline).
    description? — descriptive paragraph rendered under `label`. When
                   both `label` and `description` are set, the card
                   variant renders; otherwise the inline variant.
    children?    — snippet rendered next to the square in the inline
                   variant. Ignored in card variant.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';

  type Props = {
    checked: boolean;
    onchange?: (checked: boolean) => void;
    disabled?: boolean;
    ariaLabel?: string;
    label?: string;
    description?: string;
    children?: Snippet;
  };

  let {
    checked = $bindable(),
    onchange,
    disabled = false,
    ariaLabel,
    label,
    description,
    children,
  }: Props = $props();

  // Card variant lights up only when the caller provides both a
  // heading AND a description. `label` alone falls back to the
  // inline variant (label just replaces the children snippet).
  const isCard = $derived(!!description);

  function toggle(): void {
    if (disabled) return;
    const next = !checked;
    checked = next;
    onchange?.(next);
  }
</script>

<button
  type="button"
  class="row"
  class:card={isCard}
  role="checkbox"
  aria-checked={checked}
  aria-label={ariaLabel}
  {disabled}
  onclick={toggle}
>
  <span class="checkbox-square" class:checked aria-hidden="true">
    <svg
      viewBox="0 0 24 24"
      width="12"
      height="12"
      fill="none"
      stroke="currentColor"
      stroke-width="3"
      stroke-linecap="round"
      stroke-linejoin="round"
    >
      <polyline points="5 12 10 17 19 7" />
    </svg>
  </span>
  {#if isCard}
    <span class="card-text">
      <span class="card-label">{label}</span>
      <span class="card-detail">{description}</span>
    </span>
  {:else if children}
    <span class="inline-label">{@render children()}</span>
  {:else if label}
    <span class="inline-label">{label}</span>
  {/if}
</button>

<style>
  /*
    Button reset + row layout for the inline variant. The card
    variant layers `.card` on top to swap alignment and add
    padding/border/background.
  */
  .row {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0;
    margin: 0;
    background: none;
    border: none;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .row:disabled {
    cursor: default;
    opacity: 0.7;
  }

  /*
    Focus indicator on the inner .checkbox-square so the ring hugs
    the glyph rather than a giant card outline. Matches the
    standalone .checkbox-square focus treatment on the task list.
  */
  .row:focus-visible {
    outline: none;
  }
  .row:focus-visible :global(.checkbox-square) {
    box-shadow: 0 0 0 2px var(--focus-glow);
    border-color: var(--accent-primary);
  }
  .row:not(.card):hover:not(:disabled) :global(.checkbox-square) {
    border-color: var(--accent-primary);
    background: var(--bg-surface);
  }

  .inline-label {
    color: var(--text-primary);
    line-height: 1.4;
  }

  /*
    Card variant — mirrors .radio-row on the Theme + Mail tabs so
    preference cards across the app read as one control language.
    Full-width so the caller doesn't need to pin sizing; the
    parent's flex-column .section handles the vertical stack.
  */
  .row.card {
    display: flex;
    width: 100%;
    align-items: flex-start;
    gap: var(--space-3);
    padding: var(--space-3);
    background: var(--bg-surface);
    border: 1px solid var(--border-structural);
    border-radius: var(--radius-md);
    transition:
      border-color var(--transition-fast),
      background var(--transition-fast);
  }
  .row.card:hover:not(:disabled) {
    border-color: var(--accent-primary);
  }
  .row.card[aria-checked='true'] {
    border-color: var(--accent-primary);
    background: var(--bg-elevated);
  }
  .row.card :global(.checkbox-square) {
    /* Nudge the glyph down to sit on the heading's cap height
       instead of the flex-start top edge. Same 3px used by
       .radio-row's custom circle. */
    margin-top: 3px;
    flex-shrink: 0;
  }
  .card-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .card-label {
    font-family: var(--font-display);
    font-size: var(--text-button);
    color: var(--text-primary);
    line-height: 1.3;
  }
  .card-detail {
    font-size: var(--text-caption);
    line-height: var(--text-caption-lh);
    color: var(--text-secondary);
  }
</style>
