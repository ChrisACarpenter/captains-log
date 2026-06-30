<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { chipStyleFor, type ChipEntry } from '$lib/labelChip';

  type LabelEntry = {
    name: string;
    count: number;
    firstUsed: string;
    lastUsed: string;
    // Phase 2.8 follow-on "Colorful Labels": optional persisted hex
    // override (`#rrggbb`, lowercase). Absent on legacy entries and on
    // anything the user hasn't explicitly colored. Only the (future)
    // Label Manager populates this via invoke('set_label_color', …) when
    // the user picks a specific hex; otherwise chips render against the
    // deterministic generated color (regenerated against the current
    // theme on every render, so theme switches always produce a readable
    // hue — see colorfulChipStyle below).
    color?: string | null;
  };

  let {
    labels = $bindable<string[]>([]),
    placeholder = 'Add labels…',
    colorfulLabels = false
  }: {
    labels?: string[];
    placeholder?: string;
    /**
     * When true, chips + autocomplete options paint with a per-label
     * generated/persisted hex. When false, the existing 5-color accent-var
     * palette is used (the pre-2.8-follow-on behavior). The parent owns
     * this so the toggle in /settings drives every LabelInput callsite
     * without each component having to re-IPC the settings bundle.
     */
    colorfulLabels?: boolean;
  } = $props();

  // Suggestion pool (existing labels in the journal's index). Populated once
  // on mount; new labels created via this component are appended optimistically
  // so subsequent typing in the same session sees them.
  let suggestions = $state<LabelEntry[]>([]);

  let inputValue = $state('');
  let focused = $state(false);
  let highlighted = $state(0);
  let inputEl = $state<HTMLInputElement | null>(null);

  // Reactivity nonce for theme-derived chip colors.
  //
  // `colorfulChipStyle` reads off `document.documentElement` (data-theme +
  // computed --bg-surface) to feed `generateLabelColor`. Svelte 5's
  // reactivity tracker can't see those DOM reads, so a Custom-theme tweak
  // from /settings — which mutates :root inline styles without changing
  // any prop or $state on this component — would otherwise leave chips
  // showing colors derived from the OLD bgSurface until the next remount.
  // We bump this nonce on every 'settings-changed' event and read it from
  // inside colorfulChipStyle to register the dependency.
  let themeNonce = $state(0);
  let unlistenSettings: UnlistenFn | null = null;

  onMount(async () => {
    try {
      suggestions = await invoke<LabelEntry[]>('get_labels');
    } catch {
      // Index not present yet (first-run-ish state). Fine — empty pool.
    }
    // Theme + Custom-theme updates flow through 'settings-changed'. Bumping
    // the nonce here forces every $derived chip style to re-run and pick up
    // the fresh data-theme attribute / --bg-surface variable.
    try {
      unlistenSettings = await listen('settings-changed', () => {
        themeNonce += 1;
      });
    } catch {
      // No Tauri event bus (e.g. SSR / vitest). Chips will still render —
      // they just won't auto-recolor on theme changes, which is fine in
      // contexts where there's no settings UI driving theme switches.
    }
  });

  onDestroy(() => {
    unlistenSettings?.();
  });

  // Filter suggestions to the current input. Empty input → top 10 recent.
  // Excludes labels already added so the dropdown doesn't suggest duplicates.
  const filtered = $derived.by(() => {
    const query = inputValue.trim().replace(/^#+/, '').toLowerCase();
    const taken = new Set(labels);
    const pool = suggestions.filter((s) => !taken.has(s.name));
    if (query.length === 0) {
      return pool.slice(0, 10);
    }
    return pool.filter((s) => s.name.toLowerCase().includes(query)).slice(0, 10);
  });

  const queryAsLabel = $derived(inputValue.trim().replace(/^#+/, ''));

  const showCreateOption = $derived(
    queryAsLabel.length > 0 &&
      !filtered.some((s) => s.name.toLowerCase() === queryAsLabel.toLowerCase()) &&
      !labels.includes(queryAsLabel)
  );

  function addLabel(name: string) {
    const normalized = name.trim().replace(/^#+/, '');
    if (normalized.length === 0 || labels.includes(normalized)) return;
    labels = [...labels, normalized];
    inputValue = '';
    highlighted = 0;
    // If this is a brand-new label, optimistically add to the suggestion pool
    // so subsequent typing in the same session can autocomplete it.
    if (!suggestions.some((s) => s.name === normalized)) {
      const today = new Date().toISOString().slice(0, 10);
      suggestions = [
        { name: normalized, count: 0, firstUsed: today, lastUsed: today },
        ...suggestions
      ];
    }
  }

  function removeLabel(name: string) {
    labels = labels.filter((l) => l !== name);
    inputEl?.focus();
  }

  function onKeydown(e: KeyboardEvent) {
    const optionsCount = filtered.length + (showCreateOption ? 1 : 0);

    if (e.key === 'ArrowDown') {
      e.preventDefault();
      if (optionsCount > 0) {
        highlighted = Math.min(highlighted + 1, optionsCount - 1);
      }
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      highlighted = Math.max(highlighted - 1, 0);
    } else if (e.key === 'Enter' || e.key === 'Tab') {
      if (highlighted < filtered.length) {
        e.preventDefault();
        addLabel(filtered[highlighted].name);
      } else if (showCreateOption) {
        e.preventDefault();
        addLabel(queryAsLabel);
      } else if (e.key === 'Tab') {
        // Allow Tab to escape the field naturally when there's nothing to insert.
        return;
      }
      // Enter with no options: do nothing — let the form's submit handler decide.
    } else if (e.key === 'Backspace' && inputValue === '' && labels.length > 0) {
      // Backspacing on an empty input removes the last chip.
      labels = labels.slice(0, -1);
    } else if (e.key === ',' || e.key === ' ') {
      // Comma or space as separators when content present.
      if (queryAsLabel.length > 0) {
        e.preventDefault();
        addLabel(queryAsLabel);
      }
    }
  }

  // ----- Chip color resolution ---------------------------------------------
  //
  // Both the colorful-labels-OFF (5-color accent cycle) and ON (per-label
  // generated/persisted hex) paths live in `$lib/labelChip.ts` so the Labels
  // tab list + LabelDetailsModal preview chip can paint identically. See
  // that module's docblock for the full resolution order and the Phase 2.7
  // contrast-audit notes on the accent palette.
  //
  // The colorful-on branch reads the live theme + --bg-surface off
  // `document.documentElement` on every render so a /settings theme switch
  // refreshes generated colors without remounting. Those DOM reads are
  // invisible to Svelte's reactivity tracker, which is what `themeNonce`
  // (above) exists to paper over — touching it inside `chipStyle` wires up
  // the dependency on 'settings-changed' events.

  function currentThemeMode(): 'light' | 'dark' | 'custom' {
    if (typeof document === 'undefined') return 'dark';
    const root = document.documentElement;
    // The layout sets data-theme to the BASE polarity ('light'|'dark') even
    // when the user has activated Custom; the Custom marker is the presence
    // of the inline-overrides attribute applyCustomTheme writes onto :root.
    if (root.hasAttribute('data-cap-custom-keys')) return 'custom';
    const t = root.getAttribute('data-theme');
    return t === 'light' ? 'light' : 'dark';
  }

  function currentBgSurface(): string | undefined {
    if (typeof document === 'undefined') return undefined;
    const v = getComputedStyle(document.documentElement)
      .getPropertyValue('--bg-surface')
      .trim();
    return v.length > 0 ? v : undefined;
  }

  function chipStyle(name: string): string {
    // Touch the nonce so Svelte tracks this function's output against
    // theme-change events. The read alone wires up the dependency — the
    // value isn't used. Without this, a Custom-theme bgSurface tweak from
    // /settings wouldn't recolor existing chips because the DOM reads
    // inside chipStyleFor are invisible to the reactivity system.
    themeNonce;

    const entry: ChipEntry = suggestions.find((s) => s.name === name) ?? {
      name,
      color: null,
    };
    return chipStyleFor(entry, colorfulLabels, currentThemeMode(), currentBgSurface());
  }
</script>

<div class="label-input">
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="chips"
    onmousedown={(e) => {
      // Click on the padded chips wrapper focuses the input. Use mousedown not
      // click so it fires before the chip-x buttons (which stopPropagation).
      if (e.target === e.currentTarget) {
        e.preventDefault();
        inputEl?.focus();
      }
    }}
  >
    {#each labels as label (label)}
      <span class="chip" style={chipStyle(label)}>
        #{label}
        <button
          type="button"
          class="chip-x"
          aria-label="Remove {label}"
          onclick={(e) => {
            e.stopPropagation();
            removeLabel(label);
          }}
        >×</button>
      </span>
    {/each}
    <input
      bind:this={inputEl}
      type="text"
      placeholder={labels.length === 0 ? placeholder : ''}
      bind:value={inputValue}
      onfocus={() => (focused = true)}
      onblur={() => {
        // Tiny delay so an option click can fire before blur hides the dropdown.
        setTimeout(() => (focused = false), 150);
      }}
      onkeydown={onKeydown}
    />
  </div>

  {#if focused && (filtered.length > 0 || showCreateOption)}
    <div class="dropdown" role="listbox">
      {#each filtered as s, i (s.name)}
        <button
          type="button"
          class="option"
          class:highlighted={i === highlighted}
          style={chipStyle(s.name)}
          onmousedown={(e) => {
            e.preventDefault();
            addLabel(s.name);
          }}
          role="option"
          aria-selected={i === highlighted}
        >
          <span class="option-name">#{s.name}</span>
          {#if s.count > 0}
            <span class="option-meta">{s.count} {s.count === 1 ? 'use' : 'uses'}</span>
          {/if}
        </button>
      {/each}
      {#if showCreateOption}
        <button
          type="button"
          class="option create"
          class:highlighted={highlighted === filtered.length}
          onmousedown={(e) => {
            e.preventDefault();
            addLabel(queryAsLabel);
          }}
          role="option"
          aria-selected={highlighted === filtered.length}
        >
          <span class="create-prefix">Create</span>
          <span class="create-name">#{queryAsLabel}</span>
        </button>
      {/if}
    </div>
  {/if}
</div>

<style>
  .label-input {
    position: relative;
    width: 100%;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-3);
    background: var(--bg-surface);
    border: 1px solid var(--border-structural);
    border-radius: var(--radius-md);
    min-height: 44px;
    align-items: center;
    cursor: text;
    transition: border-color var(--transition-fast);
  }

  .chips:focus-within {
    border-color: var(--accent-primary);
    box-shadow: 0 0 0 2px var(--focus-glow);
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    padding: 2px var(--space-2);
    background: transparent;
    border: 1.5px solid var(--chip-color);
    color: var(--chip-color);
    border-radius: var(--radius-pill);
    font-family: var(--font-body);
    font-size: var(--text-caption);
    line-height: var(--text-caption-lh);
    white-space: nowrap;
  }

  .chip-x {
    background: transparent;
    border: none;
    color: inherit;
    cursor: pointer;
    font-size: 14px;
    line-height: 1;
    padding: 0 2px;
    margin-left: 2px;
    opacity: 0.7;
    transition: opacity var(--transition-fast);
  }

  .chip-x:hover {
    opacity: 1;
  }

  .chips input {
    flex: 1;
    min-width: 80px;
    padding: 2px 0;
    border: none;
    background: transparent;
    color: var(--text-primary);
    font-family: var(--font-body);
    font-size: var(--text-body);
    line-height: var(--text-body-lh);
  }

  .chips input:focus-visible {
    outline: none;
  }

  .dropdown {
    position: absolute;
    top: 100%;
    left: 0;
    right: 0;
    margin-top: var(--space-1);
    background: var(--bg-elevated);
    border: 1px solid var(--border-structural);
    border-radius: var(--radius-md);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
    max-height: 280px;
    overflow-y: auto;
    z-index: 10;
  }

  .option {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    padding: var(--space-2) var(--space-3);
    background: transparent;
    border: none;
    color: var(--text-primary);
    font-family: var(--font-body);
    font-size: var(--text-body);
    line-height: var(--text-body-lh);
    text-align: left;
    cursor: pointer;
    transition: background var(--transition-fast);
  }

  .option:hover,
  .option.highlighted {
    background: var(--bg-surface);
  }

  .option-name {
    color: var(--chip-color);
  }

  .option-meta {
    font-size: var(--text-caption);
    /* text-muted on bg-elevated (3.83:1) / bg-surface (4.46:1) both fail
       AA at 13px. text-secondary clears comfortably on both. */
    color: var(--text-secondary);
  }

  .option.create {
    border-top: 1px solid var(--border-structural);
  }

  .create-prefix {
    color: var(--text-secondary);
    margin-right: var(--space-2);
  }

  .create-name {
    /* Lifted variant — raw accent-primary on bg-elevated/bg-surface at
       16px is below WCAG AA (3.77 / 4.39). */
    color: var(--accent-primary-text);
  }
</style>
