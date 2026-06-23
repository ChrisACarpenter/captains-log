<!--
  WeekStripe — a 4px Prodigy-orange progress meter pinned to the top of the
  main window, immediately under the system title bar.

    track  = low-opacity orange (--stripe-track)
    fill   = solid orange (--stripe-fill), width = (now - monday) / 7 days
    Noot   = small mascot positioned at the reminder day/time, if enabled

  Updates every minute so the fill grows smoothly across the week. Also
  re-fetches settings whenever the backend emits "settings-changed", so
  toggling the reminder in Settings makes Noot appear/disappear immediately
  instead of waiting up to a minute for the next tick.
-->
<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';

  type ReminderSettings = {
    enabled: boolean;
    dayOfWeek: number;
    hour: number;
    minute: number;
  };
  type Settings = { reminder: ReminderSettings };

  const WEEK_MS = 7 * 24 * 60 * 60 * 1000;

  let progressPct = $state(0);
  let reminderPosPct = $state<number | null>(null);
  let timer: ReturnType<typeof setInterval> | undefined;
  let unlistenSettings: UnlistenFn | undefined;

  function computeProgress() {
    const now = new Date();
    // JS getDay() is Sun=0..Sat=6; we want ISO Mon=0..Sun=6.
    const dayIdx = (now.getDay() + 6) % 7;
    const elapsedMs =
      dayIdx * 86_400_000 +
      now.getHours() * 3_600_000 +
      now.getMinutes() * 60_000 +
      now.getSeconds() * 1_000;
    progressPct = Math.min(100, (elapsedMs / WEEK_MS) * 100);
  }

  function reminderPosition(r: ReminderSettings): number {
    const elapsedMs =
      r.dayOfWeek * 86_400_000 + r.hour * 3_600_000 + r.minute * 60_000;
    return (elapsedMs / WEEK_MS) * 100;
  }

  async function refresh() {
    computeProgress();
    try {
      const s = await invoke<Settings>('get_settings');
      reminderPosPct = s.reminder?.enabled ? reminderPosition(s.reminder) : null;
    } catch {
      // Stripe still works without the reminder marker.
    }
  }

  onMount(async () => {
    await refresh();
    timer = setInterval(refresh, 60_000);
    unlistenSettings = await listen('settings-changed', () => refresh());
  });

  onDestroy(() => {
    if (timer) clearInterval(timer);
    if (unlistenSettings) unlistenSettings();
  });
</script>

<div class="week-stripe" aria-hidden="true">
  <div class="fill" style="width: {progressPct}%;"></div>
  {#if reminderPosPct !== null}
    <img
      class="noot"
      src="/branded/noot-reminder.png"
      alt=""
      style="left: {reminderPosPct}%;"
    />
  {/if}
</div>

<style>
  .week-stripe {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    height: 4px;
    background: var(--stripe-track);
    z-index: 100;
    pointer-events: none;
  }

  .fill {
    height: 100%;
    background: var(--stripe-fill);
    transition: width 600ms ease-out;
  }

  .noot {
    position: absolute;
    top: 4px; /* hangs just below the stripe, into the route's top padding */
    transform: translateX(-50%);
    height: 28px;
    width: auto;
    filter: drop-shadow(0 1px 2px rgba(0, 0, 0, 0.25));
    pointer-events: none;
  }
</style>
