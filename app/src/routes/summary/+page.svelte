<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { invoke } from '@tauri-apps/api/core';
  import LabelInput from '$lib/LabelInput.svelte';
  import { reportDirty } from '$lib/dirty';

  type YearWeek = { year: number; week: number };

  type WeeklySummary = {
    keyAccomplishments: string;
    plansAndPriorities: string;
    challengesOrRoadblocks: string;
    anythingElse: string;
    labels: string[];
    lastUpdated: string | null;
  };

  // Auto-save status. 'idle' = settled, no unsaved edits and no recent save
  // to advertise. 'dirty' = typed something, debounce timer pending. 'saving'
  // = invoke in-flight. 'saved' = last write succeeded; show the timestamp.
  // 'error' = last save threw; show retry affordance.
  type SaveStatus = 'idle' | 'dirty' | 'saving' | 'saved' | 'error';

  const AUTOSAVE_DEBOUNCE_MS = 1500;

  // State
  let loading = $state(true);
  let loadError = $state('');
  let saveStatus = $state<SaveStatus>('idle');
  let saveErrorMessage = $state('');
  let lastSavedAt = $state<Date | null>(null);

  let yearWeek = $state<YearWeek | null>(null);
  let lastUpdated = $state<string | null>(null);

  let keyAccomplishments = $state('');
  let plansAndPriorities = $state('');
  let challengesOrRoadblocks = $state('');
  let anythingElse = $state('');
  let labels = $state<string[]>([]);

  // Last-saved snapshot. We compare the live form values against this to
  // know whether the route is "dirty" (has unsaved edits). Reset on load
  // and after a successful save.
  let snapshot = $state({
    keyAccomplishments: '',
    plansAndPriorities: '',
    challengesOrRoadblocks: '',
    anythingElse: '',
    labelsJson: '[]'
  });

  const isDirty = $derived(
    !loading &&
      (keyAccomplishments !== snapshot.keyAccomplishments ||
        plansAndPriorities !== snapshot.plansAndPriorities ||
        challengesOrRoadblocks !== snapshot.challengesOrRoadblocks ||
        anythingElse !== snapshot.anythingElse ||
        JSON.stringify(labels) !== snapshot.labelsJson)
  );

  const pushDirty = reportDirty('summary', 'the weekly summary');
  $effect(() => pushDirty(isDirty));

  // Auto-save: debounced 1.5s after typing stops. We touch the inputs
  // explicitly so this effect re-runs on every keystroke (a derived bool
  // like isDirty wouldn't, since once it's true it stays true).
  let autoSaveTimer: ReturnType<typeof setTimeout> | null = null;

  $effect(() => {
    keyAccomplishments;
    plansAndPriorities;
    challengesOrRoadblocks;
    anythingElse;
    labels;

    if (loading) return;
    if (!isDirty) return;

    saveStatus = 'dirty';
    if (autoSaveTimer) clearTimeout(autoSaveTimer);
    autoSaveTimer = setTimeout(() => {
      autoSaveTimer = null;
      void saveNow();
    }, AUTOSAVE_DEBOUNCE_MS);
  });

  // Computed week range label like "Week of June 22 – June 28, 2026"
  const weekLabel = $derived.by(() => {
    if (!yearWeek) return '';
    // ISO week → Monday of that week. JS doesn't have a built-in for ISO week
    // arithmetic, so do it manually: find the year's Jan 4 (always in week 1),
    // then offset.
    const { year, week } = yearWeek;
    const jan4 = new Date(year, 0, 4);
    const jan4Day = jan4.getDay() || 7; // Sunday → 7
    const mondayOfWeek1 = new Date(year, 0, 4 - (jan4Day - 1));
    const monday = new Date(mondayOfWeek1);
    monday.setDate(mondayOfWeek1.getDate() + (week - 1) * 7);
    const sunday = new Date(monday);
    sunday.setDate(monday.getDate() + 6);

    const fmt = (d: Date) =>
      d.toLocaleDateString('en-US', { month: 'long', day: 'numeric' });
    const sameYear = monday.getFullYear() === sunday.getFullYear();
    if (sameYear) {
      return `Week of ${fmt(monday)} – ${fmt(sunday)}, ${monday.getFullYear()}`;
    }
    return `Week of ${fmt(monday)}, ${monday.getFullYear()} – ${fmt(sunday)}, ${sunday.getFullYear()}`;
  });

  onMount(async () => {
    try {
      yearWeek = await invoke<YearWeek>('get_current_year_week');
      const s = await invoke<WeeklySummary>('get_weekly_summary', {
        year: yearWeek.year,
        week: yearWeek.week
      });
      keyAccomplishments = s.keyAccomplishments;
      plansAndPriorities = s.plansAndPriorities;
      challengesOrRoadblocks = s.challengesOrRoadblocks;
      anythingElse = s.anythingElse;
      labels = s.labels ?? [];
      lastUpdated = s.lastUpdated;
      // Baseline the dirty-comparison snapshot to what we just loaded.
      snapshot = {
        keyAccomplishments,
        plansAndPriorities,
        challengesOrRoadblocks,
        anythingElse,
        labelsJson: JSON.stringify(labels)
      };
    } catch (err) {
      loadError = String(err);
    } finally {
      loading = false;
    }
  });

  /// Save the current form to disk. Used by both the auto-save debounce and
  /// the manual Save button + Cmd+S / Cmd+↩ shortcuts. Idempotent: returns
  /// early if a save is already in flight.
  async function saveNow() {
    if (!yearWeek) return;
    if (saveStatus === 'saving') return;
    if (autoSaveTimer) {
      clearTimeout(autoSaveTimer);
      autoSaveTimer = null;
    }

    // Capture what we're about to save BEFORE the await so we can re-baseline
    // the snapshot to exactly what hit disk — even if the user keeps typing
    // during the await, isDirty will correctly flip back to true after this
    // save completes and trigger another auto-save.
    const committed = {
      keyAccomplishments,
      plansAndPriorities,
      challengesOrRoadblocks,
      anythingElse,
      labelsJson: JSON.stringify(labels),
      labels: [...labels]
    };

    saveStatus = 'saving';
    saveErrorMessage = '';
    try {
      await invoke('update_weekly_summary', {
        input: {
          year: yearWeek.year,
          week: yearWeek.week,
          keyAccomplishments: committed.keyAccomplishments,
          plansAndPriorities: committed.plansAndPriorities,
          challengesOrRoadblocks: committed.challengesOrRoadblocks,
          anythingElse: committed.anythingElse,
          labels: committed.labels
        }
      });
      // Refresh lastUpdated from the server (avoids drift from frontend clock).
      const refreshed = await invoke<WeeklySummary>('get_weekly_summary', {
        year: yearWeek.year,
        week: yearWeek.week
      });
      lastUpdated = refreshed.lastUpdated;
      snapshot = {
        keyAccomplishments: committed.keyAccomplishments,
        plansAndPriorities: committed.plansAndPriorities,
        challengesOrRoadblocks: committed.challengesOrRoadblocks,
        anythingElse: committed.anythingElse,
        labelsJson: committed.labelsJson
      };
      lastSavedAt = new Date();
      saveStatus = 'saved';
    } catch (err) {
      saveErrorMessage = String(err);
      saveStatus = 'error';
    }
  }

  /// Format a Date as "2:34 PM" — used in the "Saved HH:MM" status line.
  function formatTime(d: Date): string {
    return d.toLocaleTimeString('en-US', { hour: 'numeric', minute: '2-digit' });
  }

  // The save-status indicator text. Empty string when idle (don't clutter
  // the actions row with "nothing to report").
  const saveStatusText = $derived.by(() => {
    switch (saveStatus) {
      case 'saving':
        return 'Saving…';
      case 'dirty':
        return 'Unsaved changes';
      case 'saved':
        return lastSavedAt ? `Saved ${formatTime(lastSavedAt)}` : 'Saved';
      case 'error':
        return "Couldn't save — retry?";
      case 'idle':
      default:
        return '';
    }
  });

  function handleKeydown(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key === 's') {
      e.preventDefault();
      void saveNow();
    } else if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
      e.preventDefault();
      void saveNow();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if loading}
  <main class="loading">
    <p>Loading…</p>
  </main>
{:else if loadError}
  <main class="loading">
    <div class="card error-card">
      <h2>Couldn't load this week.</h2>
      <p>{loadError}</p>
      <button class="btn btn-marble" onclick={() => goto('/')}>Back</button>
    </div>
  </main>
{:else}
  <main>
    <section class="page">
      <header>
        <h1>Weekly Summary</h1>
        <p class="subtitle">{weekLabel}</p>
        {#if lastUpdated}
          <p class="last-updated">Last updated: {lastUpdated}</p>
        {/if}
      </header>

      <div class="form">
        <div class="field">
          <label for="key-acc">Key accomplishments</label>
          <textarea
            id="key-acc"
            bind:value={keyAccomplishments}
            placeholder="- "
            rows="5"
          ></textarea>
        </div>

        <div class="field">
          <label for="plans">Plans and priorities for next week</label>
          <textarea
            id="plans"
            bind:value={plansAndPriorities}
            placeholder="- "
            rows="4"
          ></textarea>
        </div>

        <div class="field">
          <label for="challenges">Challenges or roadblocks</label>
          <textarea
            id="challenges"
            bind:value={challengesOrRoadblocks}
            placeholder="- "
            rows="3"
          ></textarea>
        </div>

        <div class="field">
          <label for="else">Anything else on your mind</label>
          <textarea
            id="else"
            bind:value={anythingElse}
            placeholder=""
            rows="3"
          ></textarea>
        </div>

        <div class="field">
          <span class="field-heading">Labels</span>
          <LabelInput bind:labels placeholder="Tag this week (type to search, Enter to add)" />
        </div>

        {#if saveStatus === 'error' && saveErrorMessage}
          <p class="status status-error">Error: {saveErrorMessage}</p>
        {/if}

        <div class="actions">
          <button
            class="btn btn-marble"
            onclick={() => goto('/')}
            disabled={saveStatus === 'saving'}
          >
            Done
          </button>
          <!-- Auto-save status indicator. Click-to-retry when in error state. -->
          {#if saveStatus === 'error'}
            <button
              type="button"
              class="save-status is-error"
              onclick={() => void saveNow()}
            >
              {saveStatusText}
            </button>
          {:else}
            <span class="save-status is-{saveStatus}">{saveStatusText}</span>
          {/if}
          <button
            class="btn btn-emerald btn-save"
            onclick={() => void saveNow()}
            disabled={saveStatus === 'saving'}
          >
            {saveStatus === 'saving' ? 'Saving…' : 'Save'}
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
    max-width: 720px;
  }

  header {
    margin-bottom: var(--space-8);
  }

  .subtitle {
    color: var(--text-secondary);
    margin-top: var(--space-2);
  }

  .last-updated {
    color: var(--text-muted);
    font-size: var(--text-caption);
    line-height: var(--text-caption-lh);
    margin-top: var(--space-1);
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

  textarea {
    width: 100%;
    padding: var(--space-3);
    background: var(--bg-surface);
    color: var(--text-primary);
    border: 1px solid var(--border-structural);
    border-radius: var(--radius-md);
    font-family: var(--font-body);
    font-size: var(--text-body);
    line-height: var(--text-body-lh);
    resize: vertical;
    min-height: 60px;
    transition: border-color var(--transition-fast);
  }

  textarea:focus-visible {
    outline: none;
    border-color: var(--accent-primary);
    box-shadow: 0 0 0 2px var(--focus-glow);
  }

  .actions {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    margin-top: var(--space-4);
  }

  .btn-save {
    margin-left: auto;
  }

  /* Auto-save indicator — small italic text between Done and Save. State is
   * encoded via .is-{idle|dirty|saving|saved|error} modifier classes. Stays
   * intentionally subtle so it doesn't compete with the form content. */
  .save-status {
    font-size: var(--text-caption);
    line-height: var(--text-caption-lh);
    font-style: italic;
    color: var(--text-muted);
    /* The error variant is a button — strip default button chrome so it
     * matches the span variants except for the underline + cursor. */
    background: none;
    border: none;
    padding: 0;
    font-family: var(--font-body);
  }

  .save-status.is-saving,
  .save-status.is-dirty {
    color: var(--text-secondary);
  }

  .save-status.is-saved {
    color: var(--text-muted);
  }

  .save-status.is-error {
    color: var(--accent-pink);
    cursor: pointer;
    text-decoration: underline;
  }

  .save-status.is-error:hover {
    filter: brightness(1.1);
  }

  .status {
    margin: 0;
    padding: var(--space-3);
    border-radius: var(--radius-md);
    font-size: var(--text-body);
  }

  .status-success {
    background: rgba(149, 193, 59, 0.15);
    color: var(--accent-green);
    border: 1px solid rgba(149, 193, 59, 0.4);
  }

  .status-error {
    background: rgba(235, 1, 139, 0.12);
    color: var(--accent-pink);
    border: 1px solid rgba(235, 1, 139, 0.4);
  }

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
