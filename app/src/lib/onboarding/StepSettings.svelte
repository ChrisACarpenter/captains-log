<!--
  Step 4 — Settings.

  The only step that is NOT all-optional: journal_root is required (we
  need somewhere to put files). It pre-fills with the OS-appropriate
  default (~/Documents/CaptainsLog) so the user can just click
  Continue. The folder picker is here for users who want a different
  location (Dropbox folder, Documents subdirectory, iCloud sync, etc).

  Reminder is optional — it's a quiet nudge feature.
-->
<script lang="ts">
  import StepHeader from './StepHeader.svelte';
  import PointerFinger from '$lib/PointerFinger.svelte';
  import PathPickerField from '$lib/PathPickerField.svelte';
  import Checkbox from '$lib/Checkbox.svelte';

  type Props = {
    defaultJournalRoot: string;
    journalRoot: string;
    reminderEnabled: boolean;
    reminderDay: number;
    reminderTime: string;
  };

  let {
    defaultJournalRoot,
    journalRoot = $bindable(),
    reminderEnabled = $bindable(),
    reminderDay = $bindable(),
    reminderTime = $bindable(),
  }: Props = $props();

  // Match the Settings page's label list verbatim so the wizard reads
  // the same as Settings will after onboarding.
  const DAYS = [
    { value: 0, label: 'Monday' },
    { value: 1, label: 'Tuesday' },
    { value: 2, label: 'Wednesday' },
    { value: 3, label: 'Thursday' },
    { value: 4, label: 'Friday' },
    { value: 5, label: 'Saturday' },
    { value: 6, label: 'Sunday' },
  ];

  // Only journalRoot drives the pointer on this step — reminder fields
  // are optional, and the journal-folder default-prefill means most
  // users see the pointer hidden from the start anyway.
  const nextUnfilledId = $derived(
    journalRoot.trim() === '' ? 'ob-journal-root' : null
  );
</script>

<section class="step">
  <StepHeader
    title="A few last settings."
    lead="Where your journal lives, and whether Captain's Log should nudge you to write a weekly summary."
  />

  <div class="guide-row">
    <PointerFinger hidden={nextUnfilledId !== 'ob-journal-root'} />
    <PathPickerField
      id="ob-journal-root"
      label="Journal folder"
      bind:value={journalRoot}
      placeholder="/Users/you/Documents/CaptainsLog"
      hint="Plain markdown on your machine. You can move it later."
      browseLabel="Browse…"
      dialogTitle="Pick a folder for your journal"
      dialogDefaultPath={defaultJournalRoot}
    />
  </div>

  <div class="field reminder">
    <Checkbox bind:checked={reminderEnabled}>
      Send me a weekly reminder to fill in the Weekly Summary
    </Checkbox>
    {#if reminderEnabled}
      <div class="reminder-row">
        <label class="subfield">
          <span class="subfield-label">Day</span>
          <select class="text-input" bind:value={reminderDay}>
            {#each DAYS as day (day.value)}
              <option value={day.value}>{day.label}</option>
            {/each}
          </select>
        </label>
        <label class="subfield">
          <span class="subfield-label">Time</span>
          <input class="text-input" type="time" bind:value={reminderTime} />
        </label>
      </div>
    {/if}
  </div>
</section>

<style>
  /* Section drives inter-child spacing via flex gap, matching StepAboutYou
     and StepAboutManager. Previous version mixed .field margin-bottom + a
     :last-of-type reset + a guide-row margin-bottom — the combination read
     as a doubled gap between the journal-folder row and the reminder
     block. Switching to flex-gap eliminates the per-element margins and
     keeps the visual rhythm consistent across all three form steps. */
  section.step {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  /* Reminder block uses the local .field class (checkbox + optional
     day/time subrow). InputField/PathPickerField both ship their own
     .field rules so this doesn't conflict — only the reminder section
     here uses the bare class name in this file's scope.

     .guide-row layout (pointer + field) lives in Wizard.svelte as a
     shared :global() rule. */
  .field {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  /* Reminder block intentionally uses a different layout (checkbox-led)
     from the other fields. The interior reminder-row appears only when
     the toggle is on. */
  .reminder {
    gap: var(--space-3);
  }

  .reminder-row {
    display: flex;
    gap: var(--space-4);
    margin-top: var(--space-1);
  }

  .subfield {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    flex: 1;
  }
  .subfield-label {
    font-size: var(--text-caption);
    color: var(--text-secondary);
  }
</style>
