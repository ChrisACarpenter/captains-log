<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { invoke } from '@tauri-apps/api/core';
  import { open } from '@tauri-apps/plugin-dialog';
  import { openUrl } from '@tauri-apps/plugin-opener';

  type Theme = 'dark' | 'light';

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
    theme: Theme;
  };

  const DAYS = [
    { value: 0, label: 'Monday' },
    { value: 1, label: 'Tuesday' },
    { value: 2, label: 'Wednesday' },
    { value: 3, label: 'Thursday' },
    { value: 4, label: 'Friday' },
    { value: 5, label: 'Saturday' },
    { value: 6, label: 'Sunday' }
  ];

  // State
  let loading = $state(true);
  let loadError = $state('');
  let saving = $state(false);
  let saveError = $state('');
  // Form fields
  let nameInput = $state('');
  let journalRootInput = $state('');
  let originalJournalRoot = $state('');
  let originalTheme = $state<Theme>('dark');
  let currentTheme = $state<Theme>('dark');
  let reminderEnabled = $state(false);
  let reminderDay = $state(4);
  let reminderTime = $state('16:00');

  onMount(async () => {
    try {
      const s = await invoke<Settings>('get_settings');
      nameInput = s.userName ?? '';
      journalRootInput = s.journalRoot;
      originalJournalRoot = s.journalRoot;
      originalTheme = s.theme;
      currentTheme = s.theme;
      reminderEnabled = s.reminder.enabled;
      reminderDay = s.reminder.dayOfWeek;
      reminderTime = `${String(s.reminder.hour).padStart(2, '0')}:${String(s.reminder.minute).padStart(2, '0')}`;
    } catch (err) {
      loadError = String(err);
    } finally {
      loading = false;
    }
  });

  // Theme preview is live — toggling instantly applies via data-theme on <html>.
  // If the user cancels, we revert to originalTheme before leaving the page.
  function setTheme(t: Theme) {
    currentTheme = t;
    document.documentElement.setAttribute('data-theme', t);
  }

  // Deep link to System Settings → Notifications → Captain's Log.
  // macOS defaults newly-installed apps to "Temporary" notifications which
  // auto-dismiss and hide our Write/OK buttons behind a hover-to-reveal.
  // "Persistent" keeps the reminder on screen with buttons visible until clicked.
  async function openNotificationSettings() {
    try {
      await openUrl(
        'x-apple.systempreferences:com.apple.preference.notifications?id=com.prodigygame.captainslog'
      );
    } catch (err) {
      saveError = String(err);
    }
  }

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
      saveError = String(err);
    }
  }

  async function save() {
    saveError = '';
    saving = true;
    try {
      const [hourStr, minuteStr] = reminderTime.split(':');
      await invoke('update_settings', {
        input: {
          userName: nameInput.trim() || null,
          journalRoot: journalRootInput,
          reminder: {
            enabled: reminderEnabled,
            dayOfWeek: reminderDay,
            hour: Number.parseInt(hourStr, 10),
            minute: Number.parseInt(minuteStr, 10)
          },
          theme: currentTheme
        }
      });
      // Storage, reminder, and theme all hot-swap in-process — no restart needed.
      await goto('/');
    } catch (err) {
      saveError = String(err);
    } finally {
      saving = false;
    }
  }

  async function cancel() {
    // Revert any live theme preview before navigating away.
    if (currentTheme !== originalTheme) {
      document.documentElement.setAttribute('data-theme', originalTheme);
    }
    await goto('/');
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
      <button class="btn btn-marble" onclick={() => goto('/')}>Back</button>
    </div>
  </main>
{:else}
  <main>
    <section class="page">
      <header>
        <h1>Settings</h1>
        <p class="subtitle">Change your name, journal location, reminder, and theme.</p>
      </header>

      <div class="form">
        <!-- Name -->
        <div class="field">
          <label for="name">Name</label>
          <input
            id="name"
            class="text-input"
            type="text"
            placeholder="Chris"
            bind:value={nameInput}
          />
          <p class="hint">Used in the reminder notification body.</p>
        </div>

        <!-- Journal location -->
        <div class="field">
          <label for="root">Journal location</label>
          <div class="path-row">
            <input
              id="root"
              class="text-input path-input"
              type="text"
              bind:value={journalRootInput}
            />
            <button class="btn btn-marble btn-sm" onclick={pickFolder}>Browse…</button>
          </div>
          {#if journalRootInput !== originalJournalRoot}
            <p class="hint">
              The change applies as soon as you click Done — existing notes
              stay at the old location.
            </p>
          {/if}
        </div>

        <!-- Theme -->
        <div class="field">
          <span class="field-heading">Theme</span>
          <div class="theme-row">
            <button
              type="button"
              class="theme-option"
              class:active={currentTheme === 'dark'}
              onclick={() => setTheme('dark')}
            >
              Dark
            </button>
            <button
              type="button"
              class="theme-option"
              class:active={currentTheme === 'light'}
              onclick={() => setTheme('light')}
            >
              Light
            </button>
          </div>
          <p class="hint">Applies immediately; saves when you click Done.</p>
        </div>

        <!-- Reminder -->
        <div class="field">
          <label class="checkbox-row">
            <input type="checkbox" bind:checked={reminderEnabled} />
            <span>Weekly reminder</span>
          </label>
          {#if reminderEnabled}
            <div class="reminder-row">
              <label class="subfield">
                <span class="subfield-label">Day</span>
                <select class="text-input" bind:value={reminderDay}>
                  {#each DAYS as day}
                    <option value={day.value}>{day.label}</option>
                  {/each}
                </select>
              </label>
              <label class="subfield">
                <span class="subfield-label">Time</span>
                <input class="text-input" type="time" bind:value={reminderTime} />
              </label>
            </div>
            <p class="hint persistent-hint">
              Tip: macOS sets new apps to <strong>Temporary</strong> notifications by default,
              which auto-dismiss and hide the Write button behind a hover.
              <button type="button" class="link-button" onclick={openNotificationSettings}>
                Open Notification settings
              </button>
              and switch <strong>Alert Style</strong> to <strong>Persistent</strong> so the
              reminder stays on screen with buttons visible.
            </p>
          {/if}
        </div>

        {#if saveError}
          <p class="error">{saveError}</p>
        {/if}

        <div class="actions">
          <button class="btn btn-marble" onclick={cancel} disabled={saving}>Cancel</button>
          <button class="btn btn-emerald" onclick={save} disabled={saving}>
            {saving ? 'Saving…' : 'Done'}
          </button>
        </div>
      </div>
    </section>
  </main>
{/if}

<style>
  main {
    display: flex;
    justify-content: center;
    padding: var(--space-8) var(--space-4);
    min-height: 100vh;
  }

  main.loading {
    align-items: center;
  }

  .page {
    width: 100%;
    max-width: 640px;
  }

  header {
    margin-bottom: var(--space-8);
  }

  .subtitle {
    color: var(--text-secondary);
    margin-top: var(--space-2);
  }

  .form {
    display: flex;
    flex-direction: column;
    gap: var(--space-6);
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .field > label,
  .field > .field-heading {
    font-family: var(--font-display);
    font-size: var(--text-button);
    color: var(--text-primary);
  }

  .text-input {
    width: 100%;
    padding: var(--space-3);
    background: var(--bg-surface);
    color: var(--text-primary);
    border: 1px solid var(--border-structural);
    border-radius: var(--radius-md);
    font-family: var(--font-body);
    font-size: var(--text-body);
    line-height: var(--text-body-lh);
    transition: border-color var(--transition-fast);
  }

  .text-input:focus-visible {
    outline: none;
    border-color: var(--accent-primary);
    box-shadow: 0 0 0 2px var(--focus-glow);
  }

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
    color: var(--text-secondary);
  }

  .persistent-hint {
    margin-top: var(--space-3);
    padding: var(--space-3);
    background: var(--bg-elevated);
    border-radius: var(--radius-md);
    border-left: 3px solid var(--accent-primary);
  }

  .persistent-hint strong {
    color: var(--text-primary);
    font-weight: normal;
    font-family: var(--font-display);
  }

  /* Inline link styled like a text link — used in hint text to open
   * external URLs (macOS System Settings deep links). */
  .link-button {
    display: inline;
    background: none;
    border: none;
    padding: 0;
    margin: 0;
    color: var(--accent-primary);
    font: inherit;
    text-decoration: underline;
    cursor: pointer;
  }

  .link-button:hover {
    filter: brightness(1.1);
  }

  /* Theme toggle */
  .theme-row {
    display: flex;
    gap: var(--space-2);
  }

  .theme-option {
    flex: 1;
    padding: var(--space-3) var(--space-4);
    background: var(--bg-surface);
    color: var(--text-secondary);
    border: 1px solid var(--border-structural);
    border-radius: var(--radius-md);
    font-family: var(--font-display);
    font-size: var(--text-button);
    cursor: pointer;
    transition: all var(--transition-fast);
  }

  .theme-option.active {
    background: var(--accent-primary);
    color: #ffffff;
    border-color: var(--accent-primary);
  }

  /* Reminder */
  .checkbox-row {
    display: flex !important;
    flex-direction: row !important;
    align-items: center;
    gap: var(--space-3);
    font-size: var(--text-body) !important;
    cursor: pointer;
  }

  .checkbox-row input[type='checkbox'] {
    width: 18px;
    height: 18px;
    accent-color: var(--accent-primary);
  }

  .reminder-row {
    display: flex;
    gap: var(--space-4);
    margin-top: var(--space-3);
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

  /* Actions */
  .actions {
    display: flex;
    gap: var(--space-3);
    justify-content: flex-end;
    margin-top: var(--space-4);
  }

  .error {
    padding: var(--space-3);
    border-radius: var(--radius-md);
    background: rgba(235, 1, 139, 0.12);
    color: var(--accent-pink);
    border: 1px solid rgba(235, 1, 139, 0.4);
    font-size: var(--text-caption);
    margin: 0;
  }

  /* Cards (for error/restart states) */
  .card {
    max-width: 480px;
    padding: var(--space-6);
    background: var(--bg-surface);
    border: 1px solid var(--border-structural);
    border-radius: var(--radius-lg);
  }

  .card h2 {
    margin-bottom: var(--space-3);
  }

  .card p {
    color: var(--text-secondary);
    margin-bottom: var(--space-3);
  }

  .error-card {
    background: rgba(235, 1, 139, 0.08);
    border-color: rgba(235, 1, 139, 0.4);
  }
</style>
