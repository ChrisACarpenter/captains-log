<!--
  StepHeader — shared "<header><h1|h2/><p class='lead'/></header>" block
  used by every onboarding step. Folds the verbatim duplication of that
  pattern (and its .lead + heading CSS) out of the five step components.

  ## Heading level

  Intro and Complete are h1 (top-of-flow page titles). The three form
  steps in the middle are h2 (they sit inside the wizard's broader
  narrative). The .lead bottom-margin differs to match — h1 steps use a
  tighter space-4 because they're followed by prose, h2 steps use a
  looser space-6 because they're followed by form fields and want more
  air before the first input.

  ## Lead variants

  `lead` is the plain-string case (every existing step). `leadSnippet`
  is reserved for future steps that want inline markup in the subtitle
  (links, <strong>, etc.). Mutually exclusive — leadSnippet wins if both
  are passed.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';

  type Props = {
    title: string;
    level?: 'h1' | 'h2';
    lead?: string;
    leadSnippet?: Snippet;
  };

  let { title, level = 'h2', lead, leadSnippet }: Props = $props();
</script>

<header>
  {#if level === 'h1'}
    <h1>{title}</h1>
  {:else}
    <h2>{title}</h2>
  {/if}
  {#if leadSnippet}
    {@render leadSnippet()}
  {:else if lead}
    <p class="lead">{lead}</p>
  {/if}
</header>

<style>
  header {
    /* Header sits at the top of the step's flex flow. No outer margin —
       the heading + lead own their own bottom spacing below. */
    display: block;
  }

  h1,
  h2 {
    margin-bottom: var(--space-3);
  }

  /* Single .lead rule — every step (h1 + h2) used var(--space-6) before
   * the slice-3 extraction. The earlier level-keyed split tightened h1
   * leads to var(--space-4) and produced a visible regression on the
   * Welcome + All-set screens. Keep parity with pre-refactor; revisit
   * with an explicit visual slice if a tighter h1 rhythm is wanted. */
  .lead {
    color: var(--text-secondary);
    margin-bottom: var(--space-6);
  }
</style>
