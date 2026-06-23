<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { invoke } from '@tauri-apps/api/core';
  import { open } from '@tauri-apps/plugin-dialog';

  type ReminderSettings = {
    enabled: boolean;
    dayOfWeek: number;
    hour: number;
    minute: number;
  };

  type Settings = {
    firstRun: boolean;
    journalRoot: string;
    defaultJournalRoot: string;
    userName: string | null;
    reminder: ReminderSettings;
  };

  // ---- State ----

  let settings = $state<Settings | null>(null);
  let loading = $state(true);
  let loadError = $state('');

  // Wizard state
  let step = $state(0); // 0=welcome, 1=name, 2=location, 3=reminder
  let nameInput = $state('');
  let journalRootInput = $state('');
  let reminderEnabled = $state(false);
  let reminderTime = $state('16:00');
  let reminderDay = $state(4);
  let savingError = $state('');
  let saving = $state(false);
  let restartNeeded = $state(false);

  const DAYS = [
    { value: 0, label: 'Monday' },
    { value: 1, label: 'Tuesday' },
    { value: 2, label: 'Wednesday' },
    { value: 3, label: 'Thursday' },
    { value: 4, label: 'Friday' },
    { value: 5, label: 'Saturday' },
    { value: 6, label: 'Sunday' }
  ];

  onMount(async () => {
    try {
      settings = await invoke<Settings>('get_settings');
      if (settings.firstRun) {
        journalRootInput = settings.defaultJournalRoot;
      }
    } catch (err) {
      loadError = String(err);
    } finally {
      loading = false;
    }
  });

  async function pickFolder() {
    try {
      const result = await open({
        directory: true,
        multiple: false,
        defaultPath: journalRootInput || undefined
      });
      if (typeof result === 'string' && result.length > 0) {
        journalRootInput = result;
      }
    } catch (err) {
      savingError = String(err);
    }
  }

  async function completeWizard() {
    savingError = '';
    saving = true;
    try {
      const [hourStr, minuteStr] = reminderTime.split(':');
      const restart = await invoke<boolean>('complete_first_run', {
        input: {
          userName: nameInput.trim() || null,
          journalRoot: journalRootInput,
          reminder: {
            enabled: reminderEnabled,
            dayOfWeek: reminderDay,
            hour: Number.parseInt(hourStr, 10),
            minute: Number.parseInt(minuteStr, 10)
          }
        }
      });
      if (restart) {
        restartNeeded = true;
      } else {
        // No restart needed — refetch and the conditional will swap to normal mode.
        settings = await invoke<Settings>('get_settings');
      }
    } catch (err) {
      savingError = String(err);
    } finally {
      saving = false;
    }
  }
</script>

{#if loading}
  <main class="loading">
    <p>Loading…</p>
  </main>
{:else if loadError}
  <main class="loading">
    <div class="card error-card">
      <h2>Couldn't load settings.</h2>
      <p>{loadError}</p>
    </div>
  </main>
{:else if settings?.firstRun}
  <!-- ============================== Wizard ============================== -->
  <main class="wizard">
    <div class="wizard-frame">
      {#if restartNeeded}
        <section class="step">
          <h1>Almost there.</h1>
          <p class="lead">
            You picked a journal location that's different from the default.
          </p>
          <p>
            Please quit Captain's Log (<strong>⌘Q</strong>) and reopen it.
            Your settings are saved — your next launch will start clean at your
            chosen location.
          </p>
        </section>
      {:else if step === 0}
        <section class="step">
          <h1>Welcome to Captain's Log.</h1>
          <p class="lead">A weekly work journal that makes self-reviews painless.</p>
          <p>Capture what you do as you do it — Captain's Log handles the rest.</p>
          <div class="actions">
            <button class="btn btn-emerald btn-lg" onclick={() => (step = 1)}>
              Get started
            </button>
          </div>
        </section>
      {:else if step === 1}
        <section class="step">
          <h2>What should we call you?</h2>
          <input
            class="text-input"
            type="text"
            placeholder="Chris"
            bind:value={nameInput}
          />
          <p class="hint">
            Just for the app — your journal stays on your machine.
          </p>
          <div class="actions">
            <button class="btn btn-marble" onclick={() => (step = 0)}>Back</button>
            <button class="btn btn-emerald" onclick={() => (step = 2)}>Continue</button>
          </div>
        </section>
      {:else if step === 2}
        <section class="step">
          <h2>Where should we store your journal files?</h2>
          <div class="path-row">
            <input
              class="text-input path-input"
              type="text"
              bind:value={journalRootInput}
            />
            <button class="btn btn-marble btn-sm" onclick={pickFolder}>Browse…</button>
          </div>
          <p class="hint">
            Plain markdown on your machine. You can move it later in Settings.
          </p>
          <div class="actions">
            <button class="btn btn-marble" onclick={() => (step = 1)}>Back</button>
            <button class="btn btn-emerald" onclick={() => (step = 3)}>Continue</button>
          </div>
        </section>
      {:else if step === 3}
        <section class="step">
          <h2>Want a weekly nudge to fill in your Weekly Summary?</h2>
          <label class="checkbox-row">
            <input type="checkbox" bind:checked={reminderEnabled} />
            <span>Yes, remind me</span>
          </label>
          {#if reminderEnabled}
            <div class="reminder-row">
              <label class="field">
                <span class="field-label">Day</span>
                <select class="text-input" bind:value={reminderDay}>
                  {#each DAYS as day}
                    <option value={day.value}>{day.label}</option>
                  {/each}
                </select>
              </label>
              <label class="field">
                <span class="field-label">Time</span>
                <input class="text-input" type="time" bind:value={reminderTime} />
              </label>
            </div>
          {/if}
          <div class="actions">
            <button class="btn btn-marble" onclick={() => (step = 2)} disabled={saving}>
              Back
            </button>
            <button class="btn btn-emerald" onclick={completeWizard} disabled={saving}>
              {saving ? 'Saving…' : 'Finish'}
            </button>
          </div>
          {#if savingError}
            <p class="error">{savingError}</p>
          {/if}
        </section>
      {/if}

      {#if !restartNeeded && step > 0}
        <div class="steps-indicator">
          <span class="dot" class:active={step === 1}></span>
          <span class="dot" class:active={step === 2}></span>
          <span class="dot" class:active={step === 3}></span>
        </div>
      {/if}
    </div>
  </main>
{:else}
  <!-- ============================== Normal mode ============================== -->
  <main>
    <section class="welcome">
      <h1>{settings?.userName ? `Welcome back, ${settings.userName}.` : "Captain's Log"}</h1>
      <p class="lead">Weekly work journal that makes self-reviews painless.</p>

      <div class="card">
        <h2>You're in the main window.</h2>
        <p>
          This space will become the journal browser — year/week sidebar, past
          notes, weekly summaries. That's coming next.
        </p>
        <p>
          For now, capture a note by clicking the
          <strong>book icon in your menu bar</strong> (top of the screen).
        </p>
      </div>

      <div class="main-actions">
        <button class="btn btn-marble" onclick={() => goto('/settings')}>Settings</button>
      </div>
    </section>
  </main>
{/if}

<style>
  /* ---- Layouts ---- */
  main {
    display: flex;
    justify-content: center;
    padding: var(--space-12) var(--space-4);
    min-height: 100vh;
  }

  main.loading,
  main.wizard {
    align-items: center;
  }

  .welcome {
    max-width: 560px;
    width: 100%;
  }

  /* ---- Wizard ---- */
  .wizard-frame {
    width: 100%;
    max-width: 560px;
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-lg);
    padding: var(--space-8);
  }

  .step h1 {
    margin-bottom: var(--space-3);
  }

  .step h2 {
    margin-bottom: var(--space-4);
  }

  .lead {
    color: var(--text-secondary);
    margin-bottom: var(--space-4);
  }

  .step p {
    color: var(--text-secondary);
    margin-bottom: var(--space-4);
  }

  .step p:last-of-type {
    margin-bottom: 0;
  }

  .actions {
    display: flex;
    gap: var(--space-3);
    margin-top: var(--space-8);
  }

  .hint {
    font-size: var(--text-caption);
    line-height: var(--text-caption-lh);
    color: var(--text-secondary);
    margin-top: var(--space-2);
    margin-bottom: 0;
  }

  /* ---- Inputs ---- */
  .text-input {
    width: 100%;
    padding: var(--space-3);
    background: var(--bg-base);
    color: var(--text-primary);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-md);
    font-family: var(--font-body);
    font-size: var(--text-body);
    line-height: var(--text-body-lh);
    transition: border-color var(--transition-fast);
  }

  .text-input:focus-visible {
    outline: none;
    border-color: var(--accent-primary);
    box-shadow: 0 0 0 2px rgba(255, 92, 8, 0.25);
  }

  /* Path row: input grows, button stays fixed */
  .path-row {
    display: flex;
    gap: var(--space-3);
    align-items: center;
  }

  .path-input {
    flex: 1;
    font-family: var(--font-body);
    font-size: var(--text-caption);
  }

  /* Checkbox + reminder */
  .checkbox-row {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    font-size: var(--text-body);
    cursor: pointer;
    margin-bottom: var(--space-4);
  }

  .checkbox-row input[type='checkbox'] {
    width: 18px;
    height: 18px;
    accent-color: var(--accent-primary);
  }

  .reminder-row {
    display: flex;
    gap: var(--space-4);
    margin-bottom: var(--space-2);
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    flex: 1;
  }

  .field-label {
    font-size: var(--text-caption);
    color: var(--text-secondary);
  }

  /* Steps indicator */
  .steps-indicator {
    display: flex;
    gap: var(--space-2);
    justify-content: center;
    margin-top: var(--space-8);
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--border-subtle);
  }

  .dot.active {
    background: var(--accent-primary);
  }

  /* Error */
  .error {
    margin-top: var(--space-4);
    padding: var(--space-3);
    border-radius: var(--radius-md);
    background: rgba(235, 1, 139, 0.12);
    color: var(--accent-pink);
    border: 1px solid rgba(235, 1, 139, 0.4);
    font-size: var(--text-caption);
  }

  .error-card {
    background: rgba(235, 1, 139, 0.08);
    border-color: rgba(235, 1, 139, 0.4);
  }

  strong {
    color: var(--text-primary);
  }

  /* ---- Normal-mode welcome card ---- */
  h1 {
    margin-bottom: var(--space-3);
  }

  .welcome .lead {
    margin-bottom: var(--space-12);
  }

  .card {
    padding: var(--space-6);
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-lg);
  }

  .card h2 {
    margin-bottom: var(--space-3);
  }

  .card p {
    color: var(--text-secondary);
    margin-bottom: var(--space-3);
  }

  .card p:last-child {
    margin-bottom: 0;
  }

  .main-actions {
    margin-top: var(--space-6);
    display: flex;
    justify-content: flex-end;
  }
</style>
