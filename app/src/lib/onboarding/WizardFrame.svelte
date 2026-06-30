<!--
  Wizard frame — visual chrome shared by every onboarding step.

  Responsibilities:
    - Card surface (background, border, radius, padding) matching the
      pre-Phase-2.7 wizard look so the redesign feels like the same
      product, just with more rooms.
    - Decorative "Ed" image in the bottom-left corner. Different Ed per
      step so the user gets a sense of progression. Positioned opposite
      the action buttons (which always sit bottom-right), so the two
      pieces of chrome share the same baseline without ever competing
      for space. The earlier top-right placement caused text overlap
      with the lead paragraph and field labels — moving Ed below the
      content sidesteps the layout fight entirely.
    - Steps-indicator dots at the bottom. Past steps render filled-
      muted; current step is solid orange; future steps are unfilled.
      Hidden on the final "All set" stop because we don't want to
      suggest there's more to do.

  Step content (headings, fields, tip bubbles, navigation buttons) is
  passed in via the `children` snippet. The frame intentionally doesn't
  own the Back/Continue buttons — the parent Wizard.svelte does, so it
  can tailor the labels per step ("Get started" / "Continue" / "Finish
  setup" / "Take me in") without ballooning this component's API.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';

  type Props = {
    /// Path of the Ed image to render in the corner. Usually
    /// `/branded/ed-NN.png`.
    edImageSrc: string;
    /// Alt text — kept empty for purely decorative usage but the prop
    /// exists so a step that wants to surface meaning can override.
    edImageAlt?: string;
    /// Current step number, 1-based.
    step: number;
    /// Total number of stops in the flow. Used to render the right
    /// number of indicator dots.
    totalSteps: number;
    /// Whether to render the steps indicator. Final celebration step
    /// hides it.
    showIndicator?: boolean;
    children: Snippet;
  };

  let {
    edImageSrc,
    edImageAlt = '',
    step,
    totalSteps,
    showIndicator = true,
    children,
  }: Props = $props();
</script>

<section class="wizard-frame">
  <img
    src={edImageSrc}
    alt={edImageAlt}
    class="ed"
    aria-hidden={edImageAlt === '' ? 'true' : undefined}
  />

  <div class="content">
    {@render children()}
  </div>

  <!--
    Indicator is always rendered (visibility-hidden on the final step
    rather than `{#if}`'d out) so the action row stays at a consistent
    distance from the card bottom across every step. Ed's bottom-left
    positioning is baselined off that distance — if step 5 collapsed
    the indicator's vertical footprint, the buttons would shift down
    ~48px and Ed's vertical center would no longer line up with the
    button row.
  -->
  <div
    class="steps-indicator"
    class:steps-hidden={!showIndicator}
    aria-hidden="true"
  >
    {#each Array.from({ length: totalSteps }) as _, i (i)}
      <span
        class="dot"
        class:active={i + 1 === step}
        class:done={i + 1 < step}
      ></span>
    {/each}
  </div>
</section>

<style>
  .wizard-frame {
    position: relative;
    width: 100%;
    max-width: 560px;
    background: var(--bg-surface);
    border: 1px solid var(--border-structural);
    border-radius: var(--radius-lg);
    padding: var(--space-8);
    /* Ed image overflows the top-right slightly; clip the corner so the
       browser doesn't draw a halo outside the rounded edge. */
    overflow: hidden;
    /* The wizard card sits on bg-surface, so child .text-input fields
       use bg-base for visible contrast against their parent surface.
       The shared .text-input utility in app.css defaults to bg-surface;
       this override is scoped via CSS-variable inheritance. */
    --input-bg: var(--bg-base);
  }

  /* Ed sits in the bottom-left, with his vertical center aligned to
     the action-row's vertical center. The math:
       - Card padding-bottom + steps-indicator block ≈ 80px
       - Action row's vertical center ≈ 24-28px above that
       => action center sits ~104px above the card bottom.
     Ed is 130px tall, so `bottom: 40px` puts Ed's vertical center at
     ~105px — flush with the buttons across all 5 steps (the indicator
     is visibility-hidden on step 5 to keep that baseline constant).
     Slight left negative offset keeps the original "peek-out" character. */
  .ed {
    position: absolute;
    bottom: 40px;
    left: -8px;
    width: 130px;
    height: 130px;
    object-fit: contain;
    pointer-events: none;
    /* Subtle drop-shadow matches the guide-hand sprite treatment so the
       character cutouts feel like the same family of art. */
    filter: drop-shadow(0 2px 4px rgba(0, 0, 0, 0.18));
    z-index: 0;
  }

  .content {
    position: relative;
    z-index: 1;
  }

  .steps-indicator {
    display: flex;
    gap: var(--space-2);
    justify-content: center;
    margin-top: var(--space-8);
  }
  /* Step 5 hides the dots but keeps their vertical footprint so the
     action-row baseline (and therefore Ed's baseline) stays put. */
  .steps-indicator.steps-hidden {
    visibility: hidden;
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--border-decorative);
    transition: background var(--transition-fast);
  }
  .dot.done {
    background: var(--accent-primary);
    /* 0.8 (was 0.45) keeps "done" visibly distinct from "active" while
       still clearing the 3:1 UI-component threshold against bg-surface.
       At 0.45 the composited dot was only 1.87:1 vs the card. */
    opacity: 0.8;
  }
  .dot.active {
    background: var(--accent-primary);
  }
</style>
