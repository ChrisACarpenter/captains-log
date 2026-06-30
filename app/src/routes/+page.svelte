<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { invoke } from '@tauri-apps/api/core';

  import Wizard from '$lib/onboarding/Wizard.svelte';

  type ReminderSettings = {
    enabled: boolean;
    daysOfWeek: number[];
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

  onMount(async () => {
    try {
      settings = await invoke<Settings>('get_settings');
    } catch (err) {
      loadError = String(err);
    } finally {
      loading = false;
    }
  });

  // Wizard finished the persistence step — refetch settings so the
  // firstRun flag flips to false and the template branches to normal
  // mode. complete_first_run already hot-swapped storage + reminder
  // in-process, so this is purely a UI swap; no app restart.
  async function handleWizardComplete(): Promise<void> {
    try {
      settings = await invoke<Settings>('get_settings');
    } catch (err) {
      loadError = String(err);
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
  <Wizard
    defaultJournalRoot={settings.defaultJournalRoot}
    onComplete={handleWizardComplete}
  />
{:else}
  <!-- ============================== Normal mode ============================== -->
  <main class="normal">
    <section class="welcome">
      <h1>{settings?.userName ? `Welcome back, ${settings.userName}.` : "Captain's Log"}</h1>
      <p class="lead">
        A weekly work journal with tools to help you write self reviews.
      </p>

      <div class="card">
        <h2>What now?</h2>
        <p>
          <strong>Capture a note</strong> any time — click the book icon in
          your menu bar at the top of the screen.
        </p>
        <p>
          <strong>Write your weekly summary</strong> when Friday rolls around,
          or whenever it suits you.
        </p>
        <p>
          <strong>Browse past weeks</strong> to read or edit what you've
          written.
        </p>
      </div>

      <div class="main-actions">
        <button class="btn btn-emerald" onclick={() => goto('/summary')}>
          Write Weekly Summary
        </button>
        <button class="btn btn-marble" onclick={() => goto('/journal')}>
          Browse Journal
        </button>
        <button class="btn btn-marble" onclick={() => goto('/settings')}>Settings</button>
      </div>
    </section>

    <footer class="brand-footer" aria-hidden="true">
      <img src="/branded/prodigy-mark.png" class="brand-mark" alt="" />
      <span class="brand-wordmark">Prodigy</span>
    </footer>
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

  main.loading {
    align-items: center;
  }

  /* Normal mode stacks the welcome content + brand footer vertically so
   * the footer can pin to the bottom of the viewport. */
  main.normal {
    flex-direction: column;
    align-items: center;
  }

  .welcome {
    max-width: 560px;
    width: 100%;
  }

  /* Prodigy brand mark — sits at the bottom of the home screen, centered.
   * Subtle (50% opacity) so it's a quiet presence, not a logo lockup. */
  .brand-footer {
    margin-top: auto;
    padding-top: var(--space-12);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-2);
    opacity: 0.55;
    pointer-events: none;
  }

  .brand-mark {
    height: 48px;
    width: auto;
  }

  .brand-wordmark {
    font-family: var(--font-display);
    font-size: 28px;
    line-height: 1;
    color: var(--accent-primary);
    letter-spacing: 0.01em;
  }

  /* Error card for the load-failure branch. */
  .error-card {
    background: var(--bg-error-tint-soft);
    border-color: var(--border-error);
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

  /* .card / .card h2 / .card p / .card p:last-child were promoted to
     app.css as a shared utility — the rule was duplicated verbatim in
     three route files (home, /settings, /summary). */

  .main-actions {
    margin-top: var(--space-6);
    display: flex;
    justify-content: flex-end;
    gap: var(--space-3);
  }
</style>
