<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { page } from '$app/state';
  import WeekStripe from '$lib/WeekStripe.svelte';
  import '../app.css';

  let { children } = $props();

  // On every window mount (main, capture, settings) read the persisted theme
  // and apply it. data-theme="dark" is already on <html> from app.html so
  // dark stays dark; only light needs to flip the attribute.
  onMount(async () => {
    try {
      const settings = await invoke<{ theme: 'dark' | 'light' }>('get_settings');
      document.documentElement.setAttribute('data-theme', settings.theme);
    } catch {
      // If settings can't load (e.g., first launch before storage is ready),
      // dark remains the default — no recovery needed.
    }
  });

  // Week stripe lives on the main window only — the quick-capture popup
  // (label "capture", served at /capture) stays minimal.
  const showStripe = $derived(page.url.pathname !== '/capture');
</script>

{#if showStripe}
  <WeekStripe />
{/if}

{@render children()}
