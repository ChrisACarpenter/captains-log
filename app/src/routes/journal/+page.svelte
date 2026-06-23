<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { goto } from '$app/navigation';
  import { invoke } from '@tauri-apps/api/core';
  import SpellcheckTextarea from '$lib/SpellcheckTextarea.svelte';
  import { reportDirty } from '$lib/dirty';

  type YearWeek = { year: number; week: number };

  type YearNode = {
    year: number;
    weeks: number[]; // present in the year, sorted descending (newest first)
    loaded: boolean;
    expanded: boolean;
  };

  type SaveStatus = 'idle' | 'dirty' | 'saving' | 'saved' | 'error';

  const AUTOSAVE_DEBOUNCE_MS = 1500;

  // ---------- state ----------
  let loadingTree = $state(true);
  let treeError = $state('');
  let nodes = $state<YearNode[]>([]);
  let currentYearWeek = $state<YearWeek | null>(null);
  let selected = $state<YearWeek | null>(null);

  let editorLoading = $state(false);
  let editorError = $state('');
  let content = $state('');
  let initialContent = $state('');

  let saveStatus = $state<SaveStatus>('idle');
  let saveErrorMessage = $state('');
  let lastSavedAt = $state<Date | null>(null);
  let autoSaveTimer: ReturnType<typeof setTimeout> | null = null;

  // ---------- derived ----------
  const isDirty = $derived(
    !editorLoading && selected !== null && content !== initialContent
  );

  // Cross-window dirty tracking — try_quit and the close handlers read this.
  const pushDirty = reportDirty('journal', 'a past week');
  $effect(() => pushDirty(isDirty));

  // Format a YearWeek as "Week of June 22 – June 28, 2026". Shared with
  // /summary's weekLabel logic (ISO week → Monday-of-week → 7-day range).
  function formatWeekRange(yw: YearWeek): string {
    const { year, week } = yw;
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
  }

  function formatTime(d: Date): string {
    return d.toLocaleTimeString('en-US', { hour: 'numeric', minute: '2-digit' });
  }

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

  // ---------- load tree on mount ----------
  onMount(async () => {
    try {
      currentYearWeek = await invoke<YearWeek>('get_current_year_week');
      const years = await invoke<number[]>('list_years');
      // Newest year first.
      const sorted = [...years].sort((a, b) => b - a);
      nodes = sorted.map((year) => ({
        year,
        weeks: [],
        loaded: false,
        // Auto-expand the current year so the user immediately sees their
        // most recent activity. Others stay collapsed.
        expanded: year === currentYearWeek?.year
      }));
      // Eagerly load + auto-select the current week so the editor isn't
      // empty on first open.
      if (currentYearWeek) {
        const currentNode = nodes.find((n) => n.year === currentYearWeek!.year);
        if (currentNode) {
          await loadYearWeeks(currentNode);
          await selectWeek(currentYearWeek);
        }
      }
    } catch (err) {
      treeError = String(err);
    } finally {
      loadingTree = false;
    }
  });

  onDestroy(() => {
    if (autoSaveTimer) clearTimeout(autoSaveTimer);
  });

  // ---------- tree interactions ----------
  async function toggleYear(node: YearNode) {
    if (!node.expanded && !node.loaded) {
      await loadYearWeeks(node);
    }
    node.expanded = !node.expanded;
  }

  async function loadYearWeeks(node: YearNode) {
    try {
      const weeks = await invoke<number[]>('list_weeks', { year: node.year });
      // Newest week first.
      node.weeks = [...weeks].sort((a, b) => b - a);
      node.loaded = true;
    } catch (err) {
      treeError = String(err);
    }
  }

  async function selectWeek(yw: YearWeek) {
    // Flush any pending auto-save for the previously-selected week BEFORE
    // we replace the content — otherwise a debounce firing after the
    // switch would either no-op against new content (best case) or write
    // the wrong thing.
    if (autoSaveTimer) {
      clearTimeout(autoSaveTimer);
      autoSaveTimer = null;
      if (selected && isDirty) {
        await saveNow();
      }
    }
    editorLoading = true;
    editorError = '';
    saveStatus = 'idle';
    saveErrorMessage = '';
    lastSavedAt = null;
    selected = yw;
    try {
      const text = await invoke<string | null>('read_week', {
        year: yw.year,
        week: yw.week
      });
      initialContent = text ?? '';
      content = initialContent;
    } catch (err) {
      editorError = String(err);
    } finally {
      editorLoading = false;
    }
  }

  // ---------- auto-save ----------
  $effect(() => {
    content;
    if (editorLoading) return;
    if (!selected) return;
    if (!isDirty) return;
    saveStatus = 'dirty';
    if (autoSaveTimer) clearTimeout(autoSaveTimer);
    autoSaveTimer = setTimeout(() => {
      autoSaveTimer = null;
      void saveNow();
    }, AUTOSAVE_DEBOUNCE_MS);
  });

  async function saveNow() {
    if (!selected) return;
    if (saveStatus === 'saving') return;
    if (autoSaveTimer) {
      clearTimeout(autoSaveTimer);
      autoSaveTimer = null;
    }
    const committed = content;
    const committedFor = selected;
    saveStatus = 'saving';
    saveErrorMessage = '';
    try {
      await invoke('write_week', {
        year: committedFor.year,
        week: committedFor.week,
        content: committed
      });
      // Only update the snapshot if the user is still on the week we just
      // saved — otherwise they switched mid-save and we'd incorrectly mark
      // the new week as clean.
      if (selected?.year === committedFor.year && selected?.week === committedFor.week) {
        initialContent = committed;
        lastSavedAt = new Date();
        saveStatus = 'saved';
      }
    } catch (err) {
      saveErrorMessage = String(err);
      saveStatus = 'error';
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key === 's') {
      e.preventDefault();
      void saveNow();
    }
  }

  // True when this YearWeek matches the current ISO week — gets a "current"
  // dot in the sidebar.
  function isCurrentWeek(yw: YearWeek): boolean {
    return (
      currentYearWeek?.year === yw.year && currentYearWeek?.week === yw.week
    );
  }

  function isSelected(yw: YearWeek): boolean {
    return selected?.year === yw.year && selected?.week === yw.week;
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<main>
  <aside class="sidebar">
    <header>
      <h2>Journal</h2>
      <button class="link-button" onclick={() => goto('/')} type="button">
        ← Home
      </button>
    </header>

    {#if loadingTree}
      <p class="muted">Loading…</p>
    {:else if treeError}
      <p class="error">{treeError}</p>
    {:else if nodes.length === 0}
      <p class="muted">
        No notes yet. Open the menu bar icon to capture your first note.
      </p>
    {:else}
      <ul class="tree">
        {#each nodes as node (node.year)}
          <li class="year">
            <button
              type="button"
              class="year-toggle"
              onclick={() => toggleYear(node)}
              aria-expanded={node.expanded}
            >
              <span class="chevron" class:open={node.expanded}>▸</span>
              {node.year}
            </button>
            {#if node.expanded}
              <ul class="weeks">
                {#each node.weeks as week (week)}
                  {@const yw = { year: node.year, week }}
                  <li>
                    <button
                      type="button"
                      class="week-link"
                      class:selected={isSelected(yw)}
                      onclick={() => selectWeek(yw)}
                    >
                      Week {week}
                      {#if isCurrentWeek(yw)}
                        <span class="current-dot" title="Current week">●</span>
                      {/if}
                    </button>
                  </li>
                {/each}
                {#if node.loaded && node.weeks.length === 0}
                  <li class="muted">(no notes this year)</li>
                {/if}
              </ul>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  </aside>

  <section class="content">
    {#if !selected}
      <div class="placeholder">
        <h1>Pick a week to read or edit</h1>
        <p class="lead">
          Past weeks open in raw markdown. Edits auto-save after 1.5s, just
          like the weekly summary.
        </p>
        {#if currentYearWeek}
          <p class="lead">
            For the current week's structured weekly summary view, go to
            <button type="button" class="link-button" onclick={() => goto('/summary')}>
              Write Weekly Summary →
            </button>
          </p>
        {/if}
      </div>
    {:else}
      <header class="editor-header">
        <h1>{formatWeekRange(selected)}</h1>
        <p class="subtitle">
          {selected.year}-W{String(selected.week).padStart(2, '0')}
          {#if isCurrentWeek(selected)}
            · current week
          {/if}
        </p>
      </header>

      {#if editorError}
        <p class="error">{editorError}</p>
      {/if}

      {#if editorLoading}
        <p class="muted">Loading week…</p>
      {:else}
        <!-- Wrap the editor in SpellcheckTextarea so misspelled words get
          a wavy-red underline. The CSS variables forward the editor's
          monospace font + 16px padding to the backdrop so the squiggles
          line up under the real glyphs to the pixel. -->
        <SpellcheckTextarea
          class="editor"
          bind:value={content}
          placeholder="No content yet. Anything you type here saves to the weekly file."
          style="flex: 1; min-height: 200px;
            --sq-padding: var(--space-4);
            --sq-font-family: ui-monospace, 'SF Mono', SFMono-Regular, Menlo, monospace;
            --sq-font-size: 14px;
            --sq-line-height: 1.5;"
        />


        <div class="actions">
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
            disabled={saveStatus === 'saving' || !isDirty}
          >
            {saveStatus === 'saving' ? 'Saving…' : 'Save'}
          </button>
        </div>
      {/if}
    {/if}
  </section>
</main>

<style>
  main {
    display: flex;
    height: 100vh;
    min-height: 0;
  }

  /* ---- Sidebar ---- */

  .sidebar {
    width: 240px;
    flex-shrink: 0;
    background: var(--bg-elevated);
    border-right: 1px solid var(--border-structural);
    padding: var(--space-4);
    overflow-y: auto;
  }

  .sidebar header {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--space-2);
    margin-bottom: var(--space-4);
  }

  .sidebar h2 {
    font-family: var(--font-display);
    font-size: var(--text-display-sm);
    line-height: var(--text-display-sm-lh);
    margin: 0;
  }

  .tree {
    list-style: none;
    padding: 0;
    margin: 0;
  }

  .year {
    margin-bottom: var(--space-2);
  }

  .year-toggle {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    width: 100%;
    padding: var(--space-2) var(--space-3);
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--text-primary);
    font-family: var(--font-display);
    font-size: var(--text-button);
    cursor: pointer;
    text-align: left;
  }

  .year-toggle:hover {
    background: var(--bg-surface);
  }

  .chevron {
    display: inline-block;
    font-size: 10px;
    transition: transform var(--transition-fast);
    color: var(--text-muted);
  }

  .chevron.open {
    transform: rotate(90deg);
  }

  .weeks {
    list-style: none;
    padding: 0;
    margin: var(--space-1) 0 0 var(--space-6);
  }

  .week-link {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    width: 100%;
    padding: var(--space-1) var(--space-3);
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    font-family: var(--font-body);
    font-size: var(--text-body);
    cursor: pointer;
    text-align: left;
  }

  .week-link:hover {
    background: var(--bg-surface);
    color: var(--text-primary);
  }

  .week-link.selected {
    background: var(--accent-maroon);
    color: var(--neutral-cream);
  }

  .current-dot {
    color: var(--accent-primary);
    font-size: 10px;
    margin-left: auto;
  }

  .week-link.selected .current-dot {
    color: var(--neutral-cream);
  }

  /* ---- Content area ---- */

  .content {
    flex: 1;
    display: flex;
    flex-direction: column;
    padding: var(--space-8);
    min-width: 0;
    min-height: 0;
  }

  .placeholder {
    max-width: 480px;
    margin: auto;
    text-align: center;
  }

  .placeholder h1 {
    margin-bottom: var(--space-4);
  }

  .placeholder .lead {
    color: var(--text-secondary);
    margin-bottom: var(--space-3);
  }

  .editor-header {
    margin-bottom: var(--space-4);
  }

  .editor-header h1 {
    margin: 0;
  }

  .subtitle {
    color: var(--text-secondary);
    font-size: var(--text-caption);
    margin: var(--space-1) 0 0;
  }

  /* Editor lives inside <SpellcheckTextarea>. Reach across the
   * component boundary with :global() to give the inner textarea the
   * same monospace look + flex-grow behavior the raw textarea had.
   * Font/padding here MUST match the --sq-* CSS variables passed to
   * the component or the squiggles will misalign. */
  :global(textarea.sq-textarea.editor) {
    flex: 1;
    min-height: 200px;
    width: 100%;
    padding: var(--space-4);
    background: var(--bg-surface);
    color: var(--text-primary);
    border: 1px solid var(--border-structural);
    border-radius: var(--radius-md);
    font-family: ui-monospace, 'SF Mono', SFMono-Regular, Menlo, monospace;
    font-size: 14px;
    line-height: 1.5;
    resize: none;
    transition: border-color var(--transition-fast);
  }

  :global(textarea.sq-textarea.editor:focus-visible) {
    outline: none;
    border-color: var(--accent-primary);
    box-shadow: 0 0 0 2px var(--focus-glow);
  }

  /* ---- Actions row ---- */

  .actions {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    margin-top: var(--space-3);
  }

  .save-status {
    font-size: var(--text-caption);
    line-height: var(--text-caption-lh);
    font-style: italic;
    color: var(--text-muted);
    background: none;
    border: none;
    padding: 0;
    font-family: var(--font-body);
  }

  .save-status.is-saving,
  .save-status.is-dirty {
    color: var(--text-secondary);
  }

  .save-status.is-error {
    color: var(--accent-pink);
    cursor: pointer;
    text-decoration: underline;
  }

  .save-status.is-error:hover {
    filter: brightness(1.1);
  }

  .btn-save {
    margin-left: auto;
  }

  /* ---- Shared ---- */

  .link-button {
    background: none;
    border: none;
    padding: 0;
    color: var(--accent-primary);
    cursor: pointer;
    font-family: inherit;
    font-size: inherit;
    text-decoration: underline;
  }

  .link-button:hover {
    filter: brightness(1.1);
  }

  .muted {
    color: var(--text-muted);
    font-size: var(--text-caption);
    margin: var(--space-2) 0;
  }

  .error {
    color: var(--accent-pink);
    font-size: var(--text-caption);
    margin: var(--space-2) 0;
  }
</style>
