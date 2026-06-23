<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';

  type LabelEntry = {
    name: string;
    count: number;
    firstUsed: string;
    lastUsed: string;
  };

  let {
    labels = $bindable<string[]>([]),
    placeholder = 'Add labels…'
  }: {
    labels?: string[];
    placeholder?: string;
  } = $props();

  // Suggestion pool (existing labels in the journal's index). Populated once
  // on mount; new labels created via this component are appended optimistically
  // so subsequent typing in the same session sees them.
  let suggestions = $state<LabelEntry[]>([]);

  let inputValue = $state('');
  let focused = $state(false);
  let highlighted = $state(0);
  let inputEl = $state<HTMLInputElement | null>(null);

  onMount(async () => {
    try {
      suggestions = await invoke<LabelEntry[]>('get_labels');
    } catch {
      // Index not present yet (first-run-ish state). Fine — empty pool.
    }
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

  // Stable per-label color from the accent palette. Same label = same color.
  const ACCENT_VARS = [
    '--accent-pink',
    '--accent-yellow',
    '--accent-sky',
    '--accent-lavender',
    '--accent-green',
    '--accent-teal'
  ];

  function chipAccent(name: string): string {
    let hash = 0;
    for (let i = 0; i < name.length; i++) {
      hash = ((hash << 5) - hash + name.charCodeAt(i)) | 0;
    }
    return ACCENT_VARS[Math.abs(hash) % ACCENT_VARS.length];
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
      {@const accent = chipAccent(label)}
      <span class="chip" style="--chip-color: var({accent});">
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
        {@const accent = chipAccent(s.name)}
        <button
          type="button"
          class="option"
          class:highlighted={i === highlighted}
          style="--chip-color: var({accent});"
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
    border: 1px solid var(--border-subtle);
    border-radius: var(--radius-md);
    min-height: 44px;
    align-items: center;
    cursor: text;
    transition: border-color var(--transition-fast);
  }

  .chips:focus-within {
    border-color: var(--accent-primary);
    box-shadow: 0 0 0 2px rgba(255, 92, 8, 0.25);
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
    border: 1px solid var(--border-subtle);
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
    color: var(--text-muted);
  }

  .option.create {
    border-top: 1px solid var(--border-subtle);
  }

  .create-prefix {
    color: var(--text-secondary);
    margin-right: var(--space-2);
  }

  .create-name {
    color: var(--accent-primary);
  }
</style>
