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
    daysOfWeek: number[];
    hour: number;
    minute: number;
  };
  type Settings = { reminder: ReminderSettings };

  const WEEK_MS = 7 * 24 * 60 * 60 * 1000;

  // Last ISO (year, week) the tick observed. When this rolls over (e.g.
  // user left the app open across Sunday-midnight, or returned from a
  // weekend sleep), we dispatch a window-level CustomEvent so /summary
  // and /journal can refresh their cached week WITHOUT us reaching into
  // their state. Zero Rust changes — the 60s tick is the heartbeat.
  let lastTickYearWeek: { year: number; week: number } | null = null;

  // ISO-week (year, week) for a given Date. Matches Rust's chrono ISO
  // calculation (Mon=1..Sun=7, Jan 4 always in week 1).
  function isoYearWeek(d: Date): { year: number; week: number } {
    const tmp = new Date(Date.UTC(d.getFullYear(), d.getMonth(), d.getDate()));
    const dayNum = tmp.getUTCDay() || 7;
    tmp.setUTCDate(tmp.getUTCDate() + 4 - dayNum);
    const yearStart = new Date(Date.UTC(tmp.getUTCFullYear(), 0, 1));
    const week = Math.ceil(((tmp.getTime() - yearStart.getTime()) / 86_400_000 + 1) / 7);
    return { year: tmp.getUTCFullYear(), week };
  }

  let progressPct = $state(0);
  // Phase 2.7: reminders go from one day to many. The stripe renders one
  // Noot per selected day so a Mon/Wed/Fri configuration shows three
  // mascots scattered across the bar. Empty array = no Noots (reminder
  // disabled OR enabled-but-no-days).
  let reminderPosPcts = $state<number[]>([]);
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

    // Rollover detection. If the ISO (year, week) changed since the last
    // tick, broadcast so any open route caches refresh. Initial mount
    // sets the baseline without firing.
    const yw = isoYearWeek(now);
    if (lastTickYearWeek === null) {
      lastTickYearWeek = yw;
    } else if (
      lastTickYearWeek.year !== yw.year ||
      lastTickYearWeek.week !== yw.week
    ) {
      lastTickYearWeek = yw;
      window.dispatchEvent(
        new CustomEvent('captainslog:week-changed', { detail: yw })
      );
    }
  }

  function reminderPositions(r: ReminderSettings): number[] {
    if (!r.enabled) return [];
    const days = r.daysOfWeek ?? [];
    return days.map((d) => {
      const elapsedMs = d * 86_400_000 + r.hour * 3_600_000 + r.minute * 60_000;
      return (elapsedMs / WEEK_MS) * 100;
    });
  }

  async function refresh() {
    computeProgress();
    try {
      const s = await invoke<Settings>('get_settings');
      reminderPosPcts = s.reminder ? reminderPositions(s.reminder) : [];
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
  {#each reminderPosPcts as pct, i (i)}
    <img
      class="noot"
      src="/branded/noot-reminder.png"
      alt=""
      style="left: {pct}%;"
    />
  {/each}
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
