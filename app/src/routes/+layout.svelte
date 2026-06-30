<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { goto } from '$app/navigation';
  import { invoke } from '@tauri-apps/api/core';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { page } from '$app/state';
  import WeekStripe from '$lib/WeekStripe.svelte';
  import HelpButtons from '$lib/HelpButtons.svelte';
  import {
    applyCustomTheme,
    clearCustomTheme,
    deriveTokens,
    type PrimaryTokens,
    type ThemeBase,
  } from '$lib/theme';
  import { parse } from 'culori';
  import '../app.css';

  // Phase 2.8 — CustomTheme wire shape mirrors Rust's struct and the
  // Settings page's local type. Keep in sync with both if the token list
  // changes. (Layout doesn't import from settings/+page to avoid a routes
  // ↔ routes cycle.)
  type CustomTheme = {
    bgBase: string;
    bgSurface: string;
    bgElevated: string;
    textPrimary: string;
    textSecondary: string;
    textMuted: string;
    borderStructural: string;
    borderDecorative: string;
    accentPrimary: string;
    accentGreen: string;
    accentPink: string;
    btnSapphire: string;
  };

  // Infer the derivation base from bg-base luminance — light themes have
  // light backgrounds. Matches the heuristic in /settings.
  function inferBase(bgBaseHex: string): ThemeBase {
    const c = parse(bgBaseHex);
    if (!c || c.mode !== 'rgb') return 'dark';
    const sum = (c.r ?? 0) + (c.g ?? 0) + (c.b ?? 0);
    return sum >= 1.5 ? 'light' : 'dark';
  }

  // Decorative-companion cat: clicking opens a YouTube search for cat
  // content as a small breather. Multiple search variants so the
  // experience feels different each click — YouTube's own listing shuffles
  // featured results inside each query, so even hitting the same URL
  // twice usually surfaces different videos. We use search results pages
  // (not specific video IDs) on purpose: individual videos go offline,
  // search queries are evergreen.
  const CAT_VIDEO_QUERIES = [
    'https://www.youtube.com/results?search_query=cute+cat+videos',
    'https://www.youtube.com/results?search_query=funny+cat+compilation',
    'https://www.youtube.com/results?search_query=kittens+playing',
    'https://www.youtube.com/results?search_query=cats+being+derps',
    'https://www.youtube.com/results?search_query=best+cat+videos',
  ];

  function openCatBreak(): void {
    const url =
      CAT_VIDEO_QUERIES[Math.floor(Math.random() * CAT_VIDEO_QUERIES.length)];
    openUrl(url).catch((err) => {
      console.error('[layout] cat-break opener failed:', err);
    });
  }

  let { children } = $props();
  let unlistenSettings: UnlistenFn | undefined;
  let unlistenOpenSummary: UnlistenFn | undefined;

  // Apply the persisted theme to <html>. Both windows (main + capture) run
  // this layout, so the capture popup picks up theme changes too — via the
  // "settings-changed" event the backend emits after update_settings saves.
  //
  // Custom theme handling: the data-theme attribute is set to the
  // inferred BASE ('dark' or 'light') so any rule that didn't get
  // overridden by deriveTokens still picks up the correct branch (light
  // surfaces, light shadows, etc.). The 30-odd derived overrides then
  // overlay via inline styles on :root.
  async function applyTheme() {
    try {
      const settings = await invoke<{
        theme: 'dark' | 'light' | 'custom';
        customTheme: CustomTheme | null;
      }>('get_settings');
      if (settings.theme === 'custom' && settings.customTheme) {
        const base = inferBase(settings.customTheme.bgBase);
        document.documentElement.setAttribute('data-theme', base);
        try {
          const derived = deriveTokens(
            { ...settings.customTheme } as PrimaryTokens,
            base,
          );
          applyCustomTheme(derived);
        } catch {
          // Bad payload — strip overrides so the base theme isn't half-
          // applied. Settings panel can let the user re-pick colors.
          clearCustomTheme();
        }
      } else {
        // Switching out of Custom (or never had one) — strip overrides.
        clearCustomTheme();
        document.documentElement.setAttribute(
          'data-theme',
          settings.theme === 'light' ? 'light' : 'dark',
        );
      }
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

  // Cat is also hidden on /journal — the journal browser's existing
  // formatting overlaps with the upper-left corner and the cat doesn't
  // play nicely there. Keep WeekStripe visible on /journal though; only
  // the cat is scoped tighter.
  const showCat = $derived(
    page.url.pathname !== '/capture' && page.url.pathname !== '/journal'
  );
</script>

{#if showStripe}
  <WeekStripe />
{/if}

{#if showCat}
  <!-- Help + Nerds Only buttons live alongside the cat: same scope
       (main-window only, hidden in /capture), but anchored to the
       lower-right corner with a smaller form factor. -->
  <HelpButtons />

  <!-- Decorative companion + emergency cat-break button. Fixed top-left,
       sits BELOW the 4px WeekStripe with a small gap, and BELOW Noot in
       z-stack so when Noot's reminder-day position lands on the left edge
       of the week early on (Sun/Mon), Noot layers in front instead of
       being obscured by the cat. Hidden on /journal (formatting overlap)
       and /capture (popup is minimal). Click → opens a YouTube search
       for cat content in the system browser, picked at random from a few
       variants so it feels different each time. -->
  <button
    type="button"
    class="floor-cat"
    onclick={openCatBreak}
    aria-label="Take a cat break — open a random cat video in your browser"
  >
    <img src="/branded/cat.webp" alt="" aria-hidden="true" />
    <span class="floor-cat-tooltip" aria-hidden="true">Meow!</span>
  </button>
{/if}

{@render children()}

<style>
  .floor-cat {
    /* Reset the native <button> chrome so it reads as an inline image,
     * not a system button. Click target stays the full 48px square. */
    position: fixed;
    top: 12px;
    left: 12px;
    z-index: 50;
    margin: 0;
    padding: 0;
    border: 0;
    background: transparent;
    cursor: pointer;
    transition: transform 150ms ease-out;
  }
  .floor-cat:hover {
    transform: scale(1.06);
  }
  .floor-cat:focus-visible {
    outline: 2px solid var(--accent-primary);
    outline-offset: 3px;
    border-radius: 50%;
  }

  .floor-cat img {
    display: block;
    width: 48px;
    height: auto;
    /* Crisp on hi-DPI; the source is a small webp drawn at 48px. */
    image-rendering: -webkit-optimize-contrast;
  }

  /* "Meow!" tooltip — pill to the right of the cat. Hidden by default;
   * opacity-fades in on hover or keyboard focus. Pure CSS, no JS, no
   * portal — relies on absolute positioning relative to the cat button.
   * `pointer-events: none` so the tooltip itself doesn't intercept the
   * cat's click target. */
  .floor-cat-tooltip {
    position: absolute;
    left: calc(100% + 8px);
    top: 50%;
    transform: translateY(-50%);
    padding: 4px 10px;
    background: var(--bg-elevated);
    /* Lifted orange variant — raw accent-primary on bg-elevated at 14px
       only hits 3.70:1, failing AA. */
    color: var(--accent-primary-text);
    border: 1px solid var(--border-structural);
    border-radius: var(--radius-pill);
    font-size: 14px;
    font-weight: 600;
    line-height: 1.2;
    white-space: nowrap;
    opacity: 0;
    pointer-events: none;
    transition: opacity 150ms ease-out;
  }
  .floor-cat:hover .floor-cat-tooltip,
  .floor-cat:focus-visible .floor-cat-tooltip {
    opacity: 1;
  }
</style>
