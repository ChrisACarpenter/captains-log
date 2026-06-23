<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { invoke } from '@tauri-apps/api/core';

  type YearWeek = { year: number; week: number };

  type WeeklySummary = {
    keyAccomplishments: string;
    plansAndPriorities: string;
    challengesOrRoadblocks: string;
    anythingElse: string;
    lastUpdated: string | null;
  };

  // State
  let loading = $state(true);
  let loadError = $state('');
  let saving = $state(false);
  let saveError = $state('');
  let savedFlash = $state(false);

  let yearWeek = $state<YearWeek | null>(null);
  let lastUpdated = $state<string | null>(null);

  let keyAccomplishments = $state('');
  let plansAndPriorities = $state('');
  let challengesOrRoadblocks = $state('');
  let anythingElse = $state('');

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
      lastUpdated = s.lastUpdated;
    } catch (err) {
      loadError = String(err);
    } finally {
      loading = false;
    }
  });

  async function save() {
    if (!yearWeek) return;
    saveError = '';
    saving = true;
    try {
      await invoke('update_weekly_summary', {
        input: {
          year: yearWeek.year,
          week: yearWeek.week,
          keyAccomplishments,
          plansAndPriorities,
          challengesOrRoadblocks,
          anythingElse
        }
      });
      // Refresh last_updated from server (avoids drift from frontend clock).
      const refreshed = await invoke<WeeklySummary>('get_weekly_summary', {
        year: yearWeek.year,
        week: yearWeek.week
      });
      lastUpdated = refreshed.lastUpdated;
      savedFlash = true;
      setTimeout(() => (savedFlash = false), 2000);
    } catch (err) {
      saveError = String(err);
    } finally {
      saving = false;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key === 's') {
      e.preventDefault();
      save();
    } else if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
      e.preventDefault();
      save();
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

        {#if saveError}
          <p class="status status-error">Error: {saveError}</p>
        {/if}
        {#if savedFlash}
          <p class="status status-success">Saved ✓</p>
        {/if}

        <div class="actions">
          <button class="btn btn-marble" onclick={() => goto('/')} disabled={saving}>
            Done
          </button>
          <span class="hint">⌘S / ⌘↩ to save</span>
          <button class="btn btn-emerald btn-save" onclick={save} disabled={saving}>
            {saving ? 'Saving…' : 'Save'}
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

  .field > label {
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

  .hint {
    font-size: var(--text-caption);
    line-height: var(--text-caption-lh);
    color: var(--text-secondary);
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
