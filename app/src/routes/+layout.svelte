<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { goto } from '$app/navigation';
  import { invoke } from '@tauri-apps/api/core';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { page } from '$app/state';
  import WeekStripe from '$lib/WeekStripe.svelte';
  import '../app.css';

  let { children } = $props();
  let unlistenSettings: UnlistenFn | undefined;
  let unlistenOpenSummary: UnlistenFn | undefined;

  // Apply the persisted theme to <html>. Both windows (main + capture) run
  // this layout, so the capture popup picks up theme changes too — via the
  // "settings-changed" event the backend emits after update_settings saves.
  async function applyTheme() {
    try {
      const settings = await invoke<{ theme: 'dark' | 'light' }>('get_settings');
      document.documentElement.setAttribute('data-theme', settings.theme);
    } catch {
      // First launch / pre-storage: dark stays default.
    }
  }

  onMount(async () => {
    await applyTheme();
    unlistenSettings = await listen('settings-changed', () => applyTheme());

    // Fired by the Rust side when the user clicks "Write" (or the body) on
    // the weekly reminder notification. Only the main window listens — the
    // capture popup ignores it.
    if (page.url.pathname !== '/capture') {
      unlistenOpenSummary = await listen('open-summary', () => goto('/summary'));
    }
  });

  onDestroy(() => {
    if (unlistenSettings) unlistenSettings();
    if (unlistenOpenSummary) unlistenOpenSummary();
  });

  // Week stripe lives on the main window only — the quick-capture popup
  // (label "capture", served at /capture) stays minimal.
  const showStripe = $derived(page.url.pathname !== '/capture');
</script>

{#if showStripe}
  <WeekStripe />
{/if}

{@render children()}
