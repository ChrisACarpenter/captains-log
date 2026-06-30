<!--
  First-run onboarding wizard — Phase 2.7.

  Five-stop flow:
    1. Intro      — welcome + privacy framing
    2. About you  — name, Bamboo title, Jira keys (all optional)
    3. About mgr  — name, email (all optional)
    4. Settings   — journal location (required), reminder (optional)
    5. All set    — celebratory finish

  Owns the entire state machine. Each step is a presentational component
  that takes $bindable props for its fields, so values survive Back +
  Continue without manual marshaling.

  Persistence happens on step 4's "Finish setup" button — by the time
  the user sees step 5, complete_first_run has resolved. The parent
  doesn't refetch settings until the user clicks "Take me in" on step
  5, so the wizard stays mounted through the celebration. (If we
  refetched immediately on save, firstRun would flip to false, the
  parent would swap to normal-mode, and step 5 would never render.)
-->
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';

  import WizardFrame from './WizardFrame.svelte';
  import StepIntro from './StepIntro.svelte';
  import StepAboutYou from './StepAboutYou.svelte';
  import StepAboutManager from './StepAboutManager.svelte';
  import StepSettings from './StepSettings.svelte';
  import StepComplete from './StepComplete.svelte';

  type Props = {
    /// OS-appropriate suggested journal root, fetched from
    /// get_settings before the wizard mounts. Pre-fills step 4's path
    /// field so the user can just click through.
    defaultJournalRoot: string;
    /// Called after the user clicks "Take me in" on the final step.
    /// Parent should refetch settings (firstRun will be false) and
    /// rerender as normal mode.
    onComplete: () => void;
  };

  let { defaultJournalRoot, onComplete }: Props = $props();

  // ----- Step state -----

  const TOTAL_STEPS = 5;
  let step = $state(1);

  // ----- Form state — all initialized to safe defaults -----

  let name = $state('');
  let userEmail = $state('');
  let bambooTitle = $state('');
  let jiraKeys = $state('');

  let managerName = $state('');
  let managerEmail = $state('');

  // Initialize from the parent's default ONCE — subsequent prop changes
  // are not expected (parent doesn't update defaultJournalRoot mid-wizard),
  // and the user owns the value via the folder picker after mount.
  // svelte-ignore state_referenced_locally
  let journalRoot = $state(defaultJournalRoot);
  let reminderEnabled = $state(false);
  let reminderDay = $state(4); // Friday — matches the wizard's prior default
  let reminderTime = $state('16:00');

  let saving = $state(false);
  let savingError = $state('');

  // ----- Ed image per step. Index 0 is intentionally unused so we can
  //       use 1-based indexing matching the step counter. -----

  const ED_IMAGES = [
    '',
    '/branded/ed-01.png',
    '/branded/ed-02.png',
    '/branded/ed-03.png',
    '/branded/ed-04.png',
    '/branded/ed-05.png',
  ];

  const edImageSrc = $derived(ED_IMAGES[step] ?? '/branded/ed-01.png');

  // ----- Navigation handlers -----

  function back(): void {
    if (step > 1) {
      step -= 1;
      savingError = '';
    }
  }

  function next(): void {
    if (step < TOTAL_STEPS) {
      step += 1;
      savingError = '';
    }
  }

  async function finishSetup(): Promise<void> {
    if (saving) return;
    savingError = '';
    saving = true;
    try {
      const [hourStr, minuteStr] = reminderTime.split(':');
      await invoke('complete_first_run', {
        input: {
          userName: name.trim() || null,
          userEmail: userEmail.trim() || null,
          journalRoot,
          reminder: {
            enabled: reminderEnabled,
            // Phase 2.7 backend takes an array of days. The wizard
            // intentionally stays single-day to keep first-run simple;
            // power users discover multi-day later in /settings. We
            // write a one-element vec so the data model is consistent
            // from day one.
            daysOfWeek: [reminderDay],
            hour: Number.parseInt(hourStr, 10),
            minute: Number.parseInt(minuteStr, 10),
          },
          managerName: managerName.trim() || null,
          managerEmail: managerEmail.trim() || null,
          bambooTitle: bambooTitle.trim() || null,
          // Backend tokenizes + uppercases. We just pass through the raw
          // user-typed value as a single-element vec; normalize_jira_keys
          // splits on commas/whitespace and dedupes server-side.
          jiraProjectKeys: jiraKeys.trim() ? [jiraKeys] : [],
        },
      });
      step = TOTAL_STEPS; // advance to "All set" only on success
    } catch (err) {
      savingError = String(err);
    } finally {
      saving = false;
    }
  }

  function takeMeIn(): void {
    onComplete();
  }

  // ----- Derived: can we advance from the current step? -----

  // Step 4 requires a non-empty journalRoot. Everything else is freely
  // skippable. We don't validate format anywhere — typing nonsense into
  // any of these fields is the user's call to make.
  const canAdvance = $derived(step !== 4 || journalRoot.trim().length > 0);
</script>

<main class="wizard-screen">
  <WizardFrame
    {edImageSrc}
    edImageAlt=""
    {step}
    totalSteps={TOTAL_STEPS}
    showIndicator={step < TOTAL_STEPS}
  >
    {#if step === 1}
      <StepIntro />
    {:else if step === 2}
      <StepAboutYou bind:name bind:userEmail bind:bambooTitle bind:jiraKeys />
    {:else if step === 3}
      <StepAboutManager bind:managerName bind:managerEmail />
    {:else if step === 4}
      <StepSettings
        {defaultJournalRoot}
        bind:journalRoot
        bind:reminderEnabled
        bind:reminderDay
        bind:reminderTime
      />
    {:else if step === 5}
      <StepComplete {name} />
    {/if}

    {#if savingError}
      <p class="error" role="alert">{savingError}</p>
    {/if}

    <div class="actions">
      {#if step === 1}
        <button class="btn btn-emerald btn-lg" onclick={next}>Get started</button>
      {:else if step === 5}
        <button class="btn btn-emerald btn-lg" onclick={takeMeIn}>
          Start journaling…
        </button>
      {:else if step === 4}
        <button class="btn btn-marble" onclick={back} disabled={saving}>
          Back
        </button>
        <button
          class="btn btn-emerald"
          onclick={finishSetup}
          disabled={saving || !canAdvance}
        >
          {saving ? 'Saving…' : 'Finish setup'}
        </button>
      {:else}
        <button class="btn btn-marble" onclick={back}>Back</button>
        <button class="btn btn-emerald" onclick={next} disabled={!canAdvance}>
          Continue
        </button>
      {/if}
    </div>
  </WizardFrame>
</main>

<style>
  .wizard-screen {
    display: flex;
    justify-content: center;
    align-items: center;
    padding: var(--space-12) var(--space-4);
    min-height: 100vh;
  }

  /* All action rows are right-aligned across every step. Pairs with the
     bottom-LEFT Ed image in WizardFrame so the two pieces of chrome
     never compete for horizontal space at the bottom of the card.
     The larger top-margin (space-12 vs the more usual space-8) opens
     up the gap between the last step content and the actions row,
     leaving room for Ed's top edge to sit cleanly between the tip
     bubble and the action row without crowding either. */
  .actions {
    display: flex;
    gap: var(--space-3);
    margin-top: var(--space-12);
    justify-content: flex-end;
  }

  .error {
    margin-top: var(--space-4);
    margin-bottom: 0;
    padding: var(--space-3);
    border-radius: var(--radius-md);
    background: var(--bg-error-tint);
    color: var(--accent-pink-text);
    border: 1px solid var(--border-error);
    font-size: var(--text-caption);
  }

  /* Shared per-row container for the form steps: PointerFinger (fixed
     32px, never grows) + the input field (grows). Lives here because
     every step that uses it renders inside Wizard.svelte, and centralizing
     the rule keeps the three form steps from drifting.

     :global() is required because the class is applied inside child step
     components — Svelte's scoping would otherwise rewrite the selector
     to match only descendants of Wizard's own template DOM. The two
     :global() child selectors cover both field shapes the wizard uses
     (InputField's .field on steps 2 + 3, PathPickerField's .path-field
     on step 4). */
  :global(.guide-row) {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }
  :global(.guide-row > .field),
  :global(.guide-row > .path-field) {
    flex: 1;
  }
</style>
