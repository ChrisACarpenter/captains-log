<script lang="ts">
  import { onMount, onDestroy, untrack } from 'svelte';
  import { goto } from '$app/navigation';
  import { invoke } from '@tauri-apps/api/core';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import {
    refreshCurrentWeek,
    formatWeekRange,
    type YearWeek as RolloverYearWeek,
  } from '$lib/weekRollover';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import Icon from '$lib/Icon.svelte';
  import MarkdownEditor from '$lib/MarkdownEditor.svelte';
  import ExternalUpdateBanner from '$lib/ExternalUpdateBanner.svelte';
  import SaveStatus from '$lib/SaveStatus.svelte';
  import type { AutoSaveStatus } from '$lib/save-status';
  import SendToManagerButton from '$lib/SendToManagerButton.svelte';

  const MARKDOWN_CHEAT_SHEET_URL = 'https://www.markdownguide.org/cheat-sheet/';

  function openCheatSheet(e: Event): void {
    e.preventDefault();
    openUrl(MARKDOWN_CHEAT_SHEET_URL).catch((err) => {
      console.error('[journal] cheat-sheet opener failed:', err);
    });
  }
  import { reportDirty } from '$lib/dirty';

  type YearWeek = { year: number; week: number };

  type YearNode = {
    year: number;
    weeks: number[]; // present in the year, sorted descending (newest first)
    loaded: boolean;
    expanded: boolean;
  };

  // AutoSaveStatus type imported from $lib/save-status (shared with
  // /summary + /capture + the SaveStatus indicator component).

  const AUTOSAVE_DEBOUNCE_MS = 1500;

  // ---------- state ----------
  let loadingTree = $state(true);
  let treeError = $state('');
  let nodes = $state<YearNode[]>([]);
  let currentYearWeek = $state<YearWeek | null>(null);
  let selected = $state<YearWeek | null>(null);
  // Phase 3b Slice 2 — populated when the route lands via a search
  // result deep link (`/journal?year=Y&week=W&scrollTo=N`). Passed to
  // MarkdownEditor; its $effect scrolls the target byte offset into
  // view once the editor mounts. Reset to null on subsequent
  // selectWeek calls so free-navigation doesn't inherit stale offsets.
  let pendingScrollOffset = $state<number | null>(null);

  let editorLoading = $state(false);
  let editorError = $state('');
  let content = $state('');
  let initialContent = $state('');

  // ---------- view mode (Preview / Source) ----------

  type ViewMode = 'preview' | 'source';
  const VIEW_MODE_STORAGE_KEY = 'captainslog:journalViewMode';

  /**
   * Persisted across launches (Slack / Typora / VS Code convention).
   * Seed from localStorage at module init; default to 'preview' when
   * absent. Updated via the $effect below on every change.
   *
   * Why localStorage and not the Rust settings layer: this is a UI
   * preference scoped to the journal browser only, and the Tauri webview
   * persists localStorage across launches per-app. Reaching into the
   * settings IPC for one toggle would be heavier than the value justifies.
   */
  function loadViewMode(): ViewMode {
    try {
      const raw = localStorage.getItem(VIEW_MODE_STORAGE_KEY);
      if (raw === 'preview' || raw === 'source') return raw;
    } catch {
      // localStorage unavailable (private mode / disabled). Fall through.
    }
    return 'preview';
  }

  let viewMode = $state<ViewMode>(loadViewMode());

  $effect(() => {
    try {
      localStorage.setItem(VIEW_MODE_STORAGE_KEY, viewMode);
    } catch {
      // Quota exceeded or storage disabled — silent failure is fine.
    }
  });

  function setViewMode(mode: ViewMode): void {
    viewMode = mode;
  }
  function toggleViewMode(): void {
    viewMode = viewMode === 'preview' ? 'source' : 'preview';
  }

  let saveStatus = $state<AutoSaveStatus>('idle');
  let saveErrorMessage = $state('');
  let lastSavedAt = $state<Date | null>(null);
  let autoSaveTimer: ReturnType<typeof setTimeout> | null = null;

  // Cross-route file invalidation. When another writer (e.g. /summary's
  // weekly-summary save, or the menu-bar /capture popup appending a Note)
  // mutates the SAME (year, week) markdown file we have open, the Rust
  // side emits `weekly-file-changed`. We reconcile by re-reading from
  // disk. When the user has unsaved edits, we set `externalUpdate` so
  // the UI can warn instead of clobbering their work.
  let externalUpdate = $state(false);
  let weeklyFileUnlisten: UnlistenFn | null = null;
  let focusUnlisten: UnlistenFn | null = null;

  // Week-rollover dirty-conflict. Set when the system clock crossed into
  // a new ISO week while the user has unsaved edits in the SELECTED note
  // (which is typically — but not always — the previously-current week).
  // Resolved by either saving the dirty content to the old week + moving
  // the "current" highlight, or discarding + moving the highlight.
  let rolloverPending = $state<RolloverYearWeek | null>(null);

  // Own-save event suppression. The `weekly-file-changed` event Rust
  // emits after our own write travels a separate IPC channel from the
  // invoke response, so ordering between the two is not guaranteed —
  // by the time the listener callback runs, saveStatus may have already
  // flipped from 'saving' to 'saved' and `initialContent` may have been
  // rebaselined. A pure `saveStatus === 'saving'` gate would let our
  // own emit through and, if the user typed during the save, falsely
  // surface the "modified outside this view" banner.
  //
  // To suppress reliably we track `pendingCommit` — the exact bytes our
  // most recent in-flight saveNow wrote. The listener treats any disk
  // read equal to either `initialContent` (post-baseline) OR
  // `pendingCommit` (pre-baseline) as our own emit and no-ops. The slot
  // is cleared in saveNow's finally so it can't leak across saves.
  let pendingCommit = $state<string | null>(null);

  // Send-to-manager status mirror. The SendToManagerButton component
  // owns the underlying state (sentRecord, currentHash). It pushes the
  // computed "Sent Jun 26 …" / "Last sent … (edited since)" text out
  // here so we can render the line above the actions row, matching
  // /summary's layout.
  let sentStatusText = $state('');
  let sentStatusIsStale = $state(false);

  // ---------- derived ----------
  const isDirty = $derived(
    !editorLoading && selected !== null && content !== initialContent
  );

  // Cross-window dirty tracking — try_quit and the close handlers read this.
  const pushDirty = reportDirty('journal', 'a past week');
  $effect(() => pushDirty(isDirty));

  // formatWeekRange (ISO week → "Week of June 22 – June 28, 2026")
  // lives in $lib/weekRollover.ts, shared with /summary's weekLabel.

  // formatTime + saveStatusText now live inside <SaveStatus>.

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
      // Eagerly load the current year's weeks into the sidebar so the
      // current week is visible + clickable on first open. We DON'T
      // auto-select it — selection has to be an explicit user action.
      // Without that rule, opening /journal and typing would silently
      // write to the current week's file even when the user didn't mean
      // to be editing it (they came here to browse, not write — that's
      // /summary's job). The placeholder pane in the {#if !selected}
      // branch handles the cold-start state.
      if (currentYearWeek) {
        const currentNode = nodes.find((n) => n.year === currentYearWeek!.year);
        if (currentNode) {
          await loadYearWeeks(currentNode);
        }
      }
    } catch (err) {
      treeError = String(err);
    } finally {
      loadingTree = false;
    }

    // Deep-link support (Phase 3a Slice 1 — Label Library drill-down;
    // Phase 3b Slice 2 — search result scroll-to). If the URL carries
    // ?year=Y&week=W, expand that year in the sidebar tree (loading
    // its weeks if it isn't the current year) and select the target
    // week. Optional ?scrollTo=N (byte offset) additionally scrolls
    // MarkdownEditor to that position after the content loads.
    //
    // Parses defensively — bad values fall through silently and leave
    // the user on the empty-state pane.
    const params = new URLSearchParams(window.location.search);
    const paramYear = Number(params.get('year'));
    const paramWeek = Number(params.get('week'));
    const paramScrollRaw = params.get('scrollTo');
    if (Number.isFinite(paramYear) && Number.isFinite(paramWeek) && paramYear > 0 && paramWeek > 0) {
      let targetNode = nodes.find((n) => n.year === paramYear);
      if (targetNode && !targetNode.loaded) {
        await loadYearWeeks(targetNode);
      }
      // Verify the requested week actually exists in the loaded node
      // before selecting — protects against stale deep-links pointing
      // at a week that no longer has a file on disk.
      if (targetNode && targetNode.weeks.includes(paramWeek)) {
        targetNode.expanded = true;
        await selectWeek({ year: paramYear, week: paramWeek });
        // Set scroll offset AFTER selectWeek so the fresh content is
        // loaded first. MarkdownEditor's $effect fires when both view
        // and offset are ready.
        if (paramScrollRaw !== null) {
          const parsed = Number(paramScrollRaw);
          if (Number.isFinite(parsed) && parsed >= 0) {
            pendingScrollOffset = parsed;
          }
        }
      }
    }

    // Week-rollover triggers. Same three signals as /summary:
    // window focus, document visibility, and the WeekStripe tick's
    // CustomEvent broadcast. See /summary for the rationale.
    focusUnlisten = await getCurrentWindow().listen('tauri://focus', () => {
      void handleWeekRollover();
    });
    document.addEventListener('visibilitychange', onVisibilityChange);
    window.addEventListener('captainslog:week-changed', onWeekChangedEvent);

    // Subscribe to the cross-route file-changed broadcast. Anything that
    // writes to a weekly markdown file (write_week, update_weekly_summary,
    // create_note) emits this from Rust. The listener stays mounted for
    // the lifetime of the route and only acts when the changed (year,
    // week) matches the one we have open.
    weeklyFileUnlisten = await listen<{ year: number; week: number }>(
      'weekly-file-changed',
      async (event) => {
        if (!selected) return;
        if (
          event.payload.year !== selected.year ||
          event.payload.week !== selected.week
        ) {
          return;
        }
        // Don't gate on saveStatus here: Tauri's invoke-response and event
        // emit travel separate IPC paths, so by the time this listener
        // runs saveStatus may already have flipped to 'saved'. Instead,
        // reconcileWithDisk compares disk to BOTH `initialContent` and
        // `pendingCommit` to recognize our own emit. Suppressing on
        // saveStatus alone caused a false-positive externalUpdate banner
        // (and a destructive Reload button) when the user typed during
        // their own save.
        await reconcileWithDisk();
      }
    );
  });

  /// Re-read the currently-selected week from disk and merge with our
  /// in-memory `content`. Three branches:
  ///   1. Disk == initialContent (or == pendingCommit) → silent no-op.
  ///      The disk matches either our last-known baseline or the bytes
  ///      our currently-in-flight save is writing — i.e. nothing has
  ///      diverged from our perspective. This is the common case for
  ///      our own save's emit echoing back.
  ///   2. Disk != baseline, !isDirty → silent reload; replace content +
  ///      baseline. The common case for "/capture appended a note,
  ///      refresh the journal view so the user sees it."
  ///   3. Disk != baseline, isDirty → set externalUpdate = true. The
  ///      banner lets the user pick Reload (lose edits) or keep typing
  ///      (next save overwrites the external change). We never silently
  ///      destroy edits.
  ///
  /// Why compare to `initialContent` and not `content`: the user may
  /// have typed during an own-save's IPC roundtrip. Their newer keystroke
  /// makes `content` !== `initialContent`, so a `disk === content`
  /// check would false-positive every time they type-through-save and
  /// surface a misleading banner. The baseline + pendingCommit pair is
  /// invariant during typing — only saveNow itself moves them.
  async function reconcileWithDisk(): Promise<void> {
    if (!selected) return;
    try {
      const diskRaw = await invoke<string | null>('read_week', {
        year: selected.year,
        week: selected.week
      });
      const disk = diskRaw ?? '';
      if (disk === initialContent || disk === pendingCommit) {
        externalUpdate = false;
        return;
      }
      if (isDirty) {
        externalUpdate = true;
        return;
      }
      initialContent = disk;
      content = disk;
      externalUpdate = false;
    } catch (err) {
      // Reconciliation failure shouldn't surface as a save-error. Stay
      // silent; the user can re-select the week to force a reload.
      console.error('[journal] reconcile failed:', err);
    }
  }

  /// Discard local edits and adopt whatever's on disk now. Wired to the
  /// "Reload" button on the external-update banner.
  async function reloadFromDisk(): Promise<void> {
    if (!selected) return;
    try {
      const text = await invoke<string | null>('read_week', {
        year: selected.year,
        week: selected.week
      });
      initialContent = text ?? '';
      content = initialContent;
      externalUpdate = false;
      saveStatus = 'idle';
    } catch (err) {
      editorError = String(err);
    }
  }

  onDestroy(() => {
    if (autoSaveTimer) clearTimeout(autoSaveTimer);
    weeklyFileUnlisten?.();
    weeklyFileUnlisten = null;
    focusUnlisten?.();
    focusUnlisten = null;
    document.removeEventListener('visibilitychange', onVisibilityChange);
    window.removeEventListener('captainslog:week-changed', onWeekChangedEvent);
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
    externalUpdate = false;
    // Clear any lingering deep-link scroll offset — the onMount deep-
    // link handler sets it AFTER selectWeek returns, so this only nulls
    // it out on subsequent sidebar-driven week switches. Prevents a
    // manual "click Week 24" from re-scrolling to a stale byte offset.
    pendingScrollOffset = null;
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

  // ---------- week rollover ----------
  //
  // When the ISO (year, week) advances while /journal is open, we need
  // to keep two things in sync with reality:
  //   1. `currentYearWeek` — drives the "current" dot in the sidebar.
  //   2. The sidebar tree itself — the new week may belong to a year
  //      that doesn't yet have a node (Dec 31 → Jan 1), or to the same
  //      year but with a week number that wasn't yet present in
  //      `nodes[].weeks`.
  //
  // We do NOT auto-select the new week — journal selection is always an
  // explicit user act. We only refresh the highlight + tree.

  async function refreshTreeForNewWeek(next: RolloverYearWeek): Promise<void> {
    // Same year — just reload its weeks if loaded, so the new week appears.
    let node = nodes.find((n) => n.year === next.year);
    if (node) {
      if (node.loaded) {
        try {
          const weeks = await invoke<number[]>('list_weeks', { year: next.year });
          node.weeks = [...weeks].sort((a, b) => b - a);
        } catch (err) {
          console.error('[journal] refresh weeks failed:', err);
        }
      }
      return;
    }
    // New year — fetch the full list_years again so the prior year's
    // node order (newest-first) stays consistent if a backfill happened.
    try {
      const years = await invoke<number[]>('list_years');
      const sorted = [...years].sort((a, b) => b - a);
      // Preserve expanded/loaded state for years we already know about.
      const byYear = new Map(nodes.map((n) => [n.year, n] as const));
      nodes = sorted.map((y) => {
        const existing = byYear.get(y);
        if (existing) return existing;
        return {
          year: y,
          weeks: [],
          loaded: false,
          expanded: y === next.year
        };
      });
      // Eagerly load the new current year's weeks so the dot lands on
      // a visible row.
      const fresh = nodes.find((n) => n.year === next.year);
      if (fresh && !fresh.loaded) await loadYearWeeks(fresh);
    } catch (err) {
      console.error('[journal] list_years on rollover failed:', err);
    }
  }

  async function handleWeekRollover(): Promise<void> {
    // The selected note is dirty? Treat as conflict — the user's edits
    // may have been intended for last week (the previously-current week).
    const dirtyConflict = selected !== null && isDirty;
    await refreshCurrentWeek({
      currentYearWeek,
      isDirty: dirtyConflict,
      onWeekChange: async (next) => {
        currentYearWeek = next;
        await refreshTreeForNewWeek(next);
      },
      onDirtyConflict: ({ newYearWeek }) => {
        rolloverPending = newYearWeek;
      }
    });
  }

  /// "Save to last week" — flush dirty edits to the currently-selected
  /// week's file, then advance the highlight to the new week.
  async function rolloverSaveOldThenSwap(): Promise<void> {
    if (!rolloverPending) return;
    const next = rolloverPending;
    // saveNow can return synchronously when a save is already in flight
    // (it reschedules via autoSaveTimer). Poll until the save fully
    // settles so the swap doesn't run before the user's dirty edits
    // actually hit disk.
    await saveNow();
    const settleStart = Date.now();
    while (saveStatus === 'saving' || saveStatus === 'dirty') {
      if (Date.now() - settleStart > 10_000) {
        console.warn('[journal] rollover-save timed out waiting for save to settle');
        return;
      }
      await new Promise((r) => setTimeout(r, 50));
    }
    if (saveStatus === 'error') return;
    currentYearWeek = next;
    await refreshTreeForNewWeek(next);
    rolloverPending = null;
  }

  /// Discard dirty edits and advance the highlight.
  async function rolloverDiscardAndSwap(): Promise<void> {
    if (!rolloverPending) return;
    const next = rolloverPending;
    // Drop in-memory edits by re-baselining content to disk.
    await reloadFromDisk();
    currentYearWeek = next;
    await refreshTreeForNewWeek(next);
    rolloverPending = null;
  }

  function onVisibilityChange(): void {
    if (!document.hidden) void handleWeekRollover();
  }
  function onWeekChangedEvent(): void {
    void handleWeekRollover();
  }

  // ---------- auto-save ----------
  $effect(() => {
    content;
    if (editorLoading) return;
    if (!selected) return;
    if (!isDirty) return;
    // Read + write saveStatus inside `untrack` so this effect stays
    // content-driven — WITHOUT untrack the read on `saveStatus !==
    // 'saving'` adds saveStatus as a reactive dep, so every state flip
    // during a save cycle ('dirty' → 'saving' → 'saved') re-fires the
    // effect and REARMS the autoSaveTimer. That stale timer then
    // triggers a redundant second save 1.5s later, causing visible
    // button-flicker (buttons cycling disabled→enabled→disabled→enabled).
    //
    // Don't downgrade saveStatus from 'saving' to 'dirty' mid-save:
    // the own-save suppression (pendingCommit) is a single slot, and
    // a second saveNow firing while the first is in flight would
    // overwrite the slot and let the first save's own emit surface as
    // a false-positive externalUpdate banner.
    untrack(() => {
      if (saveStatus !== 'saving') {
        saveStatus = 'dirty';
      }
    });
    if (autoSaveTimer) clearTimeout(autoSaveTimer);
    autoSaveTimer = setTimeout(() => {
      autoSaveTimer = null;
      void saveNow();
    }, AUTOSAVE_DEBOUNCE_MS);
  });

  async function saveNow() {
    if (!selected) return;
    if (saveStatus === 'saving') {
      // Another save is mid-invoke. The single-slot pendingCommit can't
      // hold both the in-flight bytes AND the new edits. Reschedule
      // ourselves so the new content saves AFTER the current save
      // settles.
      if (autoSaveTimer) clearTimeout(autoSaveTimer);
      autoSaveTimer = setTimeout(() => {
        autoSaveTimer = null;
        void saveNow();
      }, AUTOSAVE_DEBOUNCE_MS);
      return;
    }
    if (autoSaveTimer) {
      clearTimeout(autoSaveTimer);
      autoSaveTimer = null;
    }
    const committed = content;
    const committedFor = selected;
    // Declare intent BEFORE invoke so the cross-route listener can
    // recognize our own emit even when the event arrives before the
    // baseline update (or before invoke even resolves). Cleared in
    // finally regardless of outcome.
    pendingCommit = committed;
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
    } finally {
      pendingCommit = null;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    // e.key for Shift+S resolves to 'S' on most layouts; accept both cases.
    if ((e.metaKey || e.ctrlKey) && (e.key === 's' || e.key === 'S')) {
      if (e.shiftKey) {
        // Cmd+Shift+S → toggle Preview / Source. Only meaningful when a
        // week is open (the toggle controls the editor, which only mounts
        // after a week is selected). Bail on the placeholder screen so the
        // chord isn't a no-op-but-prevented browser shortcut for users
        // who haven't picked a week yet.
        if (!selected) return;
        e.preventDefault();
        toggleViewMode();
      } else {
        e.preventDefault();
        void saveNow();
      }
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
    </header>

    <!-- Phase 3b Slice 1 — Search entry point. Sits below the "Journal"
         header and above the tree so it's discoverable without competing
         with the browsing UI. Uses the shared .btn .btn-marble .btn-sm
         combo so it reads as an action button (matches the Details
         buttons on the Settings → Labels tab) rather than as a text
         input. Full-width via a wrapping class so the whole sidebar
         column becomes a hit target. -->
    <button
      type="button"
      class="btn btn-marble btn-sm sidebar-search-button"
      onclick={() => void goto('/search')}
      aria-label="Search Weekly Summaries"
    >
      <span class="sidebar-search-icon" aria-hidden="true">
        <Icon name="search" size={14} />
      </span>
      Search
    </button>

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
          Past weeks open in rich preview. Hit
          <kbd>⌘⇧S</kbd> or click <strong>Source</strong> to see the raw
          markdown. Edits auto-save after 1.5s, just like the weekly summary.
          New to markdown?
          <button type="button" class="link-button" onclick={openCheatSheet}>
            Open the cheat sheet.
          </button>
        </p>
        {#if currentYearWeek}
          <p class="lead">
            Or write up the current week's structured summary:
          </p>
          <button
            type="button"
            class="btn btn-emerald"
            onclick={() => goto('/summary')}
          >
            Write Weekly Summary
          </button>
        {/if}
      </div>
    {:else}
      <header class="editor-header">
        <div class="editor-header-text">
          <h1>{formatWeekRange(selected)}</h1>
          <p class="subtitle">
            {selected.year}-W{String(selected.week).padStart(2, '0')}
            {#if isCurrentWeek(selected)}
              · current week
            {/if}
          </p>
        </div>
        <!-- Segmented Preview / Source toggle. Mirrors VS Code / Typora /
             Obsidian's mode-switch convention. Cmd+Shift+S is bound globally
             via handleKeydown for power-user speed; this control is the
             discoverable affordance. -->
        <div
          class="view-toggle"
          role="group"
          aria-label="Editor view mode"
        >
          <button
            type="button"
            class="view-toggle-btn"
            class:is-active={viewMode === 'preview'}
            aria-pressed={viewMode === 'preview'}
            onclick={() => setViewMode('preview')}
            title="Rich-text preview (⌘⇧S to toggle)"
          >
            Preview
          </button>
          <button
            type="button"
            class="view-toggle-btn"
            class:is-active={viewMode === 'source'}
            aria-pressed={viewMode === 'source'}
            onclick={() => setViewMode('source')}
            title="Raw markdown source (⌘⇧S to toggle)"
          >
            Source
          </button>
        </div>
      </header>

      {#if editorError}
        <p class="error">{editorError}</p>
      {/if}

      {#if editorLoading}
        <p class="muted">Loading week…</p>
      {:else}
        <!-- Phase 5: dual-mode editor.
          - Preview mode: live-preview decorations hide markdown markers
            (`**`, `>`, `#`, etc.) so prose renders as styled rich text.
            Body font, 16px, default line-height. Toolbar visible — when
            markers are hidden it's the discoverable formatting affordance,
            same role it plays on /summary.
          - Source mode: raw markdown. Monospace 14px / 1.5 line-height,
            matches the prior textarea-era look. Toolbar hidden — raw
            markers + Cmd+B/I/K shortcuts are enough; the toolbar would
            be redundant chrome.

          MarkdownEditor wraps its live-preview extension in a CM6
          Compartment internally; flipping `livePreview` reactively swaps
          the extension on the existing EditorView via reconfigure(), so
          cursor position, selection, scroll, and undo history all survive
          the toggle. Earlier shape used `{#key viewMode}` to force a
          remount, which lost the cursor on every flip. -->
        {#if externalUpdate}
          <ExternalUpdateBanner
            onReload={() => void reloadFromDisk()}
            onDismiss={() => (externalUpdate = false)}
          >
            This week was modified outside this view. Your unsaved edits
            would overwrite it on the next save.
          </ExternalUpdateBanner>
        {/if}

        {#if rolloverPending}
          <!-- Week-rollover dirty-conflict banner. Reload = save the dirty
               edits to the week the user has open (presumed previously-
               current); Dismiss = discard them. Either way the "current"
               highlight advances to the new ISO week. -->
          <ExternalUpdateBanner
            onReload={() => void rolloverSaveOldThenSwap()}
            onDismiss={() => void rolloverDiscardAndSwap()}
          >
            A new week started. Save these edits to last week, or discard?
            Reload saves them to the week you have open; Dismiss discards
            them. Either way the current-week highlight advances.
          </ExternalUpdateBanner>
        {/if}

        <MarkdownEditor
          value={content}
          onChange={(v) => (content = v)}
          scrollTargetOffset={pendingScrollOffset}
          livePreview={viewMode === 'preview'}
          showToolbar={viewMode === 'preview'}
          placeholder="No content yet. Anything you type here saves to the weekly file."
          style={
            viewMode === 'preview'
              ? 'flex: 1; min-height: 200px; --md-padding: var(--space-4);'
              : 'flex: 1; min-height: 200px;'
                + ' --md-padding: var(--space-4);'
                + " --md-font-family: ui-monospace, 'SF Mono', SFMono-Regular, Menlo, monospace;"
                + ' --md-font-size: 14px;'
                + ' --md-line-height: 1.5;'
          }
        />


      {/if}
    {/if}

    <!-- Actions area = sent-status (when present) + actions row stacked
       with gap-based spacing. Same structure as /summary so the visual
       rhythm matches. Back is ALWAYS visible (so the user can always
       exit the journal browser). Save + Send-to-manager only appear
       when a week is selected. The .button-cluster wrapper has
       margin-left: auto, so the cluster sits flush-right regardless of
       which children render — Back-only states still land on the right
       edge. -->
    <div class="actions-area">
      {#if selected && sentStatusText}
        <p class="sent-status" class:is-stale={sentStatusIsStale}>
          {sentStatusText}
        </p>
      {/if}
      <div class="actions">
        {#if selected}
          <SaveStatus
            status={saveStatus}
            lastSavedAt={lastSavedAt}
            onRetry={() => void saveNow()}
          />
        {/if}
        <div class="button-cluster">
          {#if selected}
            <!-- Save is disabled only on `!isDirty` (a stable state that
                 lingers until the user types again). The transient
                 `saveStatus === 'saving'` state used to be part of the
                 disabled condition but was removed — local disk saves
                 complete in ~10ms, well under a 60fps frame, so
                 toggling `disabled` for that window produced visible
                 flicker in WKWebView without offering the user any
                 useful signal. saveNow gates internally on
                 `saveStatus === 'saving'` for the double-click case. -->
            <button
              class="btn btn-emerald btn-save"
              onclick={() => void saveNow()}
              disabled={!isDirty}
            >
              Save
            </button>
          {/if}
          <button class="btn btn-ruby" onclick={() => goto('/')}>Back</button>
          {#if selected}
            <SendToManagerButton
              year={selected.year}
              week={selected.week}
              weekLabel={formatWeekRange(selected)}
              {isDirty}
              {saveStatus}
              bind:sentStatusText
              bind:sentStatusIsStale
            />
          {/if}
        </div>
      </div>
    </div>
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

  /* Phase 3b Slice 1 — Search entry point. The .btn .btn-marble .btn-sm
     combo owns the button chrome (background, shadow, hover, focus).
     Local overrides just make it fill the sidebar column and put the
     icon flush with the label. */
  .sidebar-search-button {
    width: 100%;
    margin-bottom: var(--space-4);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--space-2);
  }
  .sidebar-search-icon {
    display: inline-flex;
    align-items: center;
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
    transition: transform var(--duration-fast) var(--ease-standard);
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
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--space-3);
    margin-bottom: var(--space-4);
  }

  .editor-header-text {
    min-width: 0;
  }

  .editor-header h1 {
    margin: 0;
  }

  /* ---- View toggle (Preview / Source) ---- */

  .view-toggle {
    display: inline-flex;
    flex-shrink: 0;
    background: var(--bg-elevated);
    border: 1px solid var(--border-structural);
    border-radius: var(--radius-md);
    padding: 2px;
    gap: 2px;
  }

  .view-toggle-btn {
    appearance: none;
    background: transparent;
    border: none;
    border-radius: calc(var(--radius-md) - 2px);
    color: var(--text-secondary);
    font: inherit;
    font-size: var(--text-caption);
    padding: 4px 10px;
    cursor: pointer;
    transition: background var(--duration-fast) var(--ease-standard), color var(--duration-fast) var(--ease-standard);
  }

  .view-toggle-btn:hover {
    color: var(--text-primary);
  }

  .view-toggle-btn:focus-visible {
    outline: none;
    box-shadow: 0 0 0 2px var(--focus-glow);
  }

  .view-toggle-btn.is-active {
    background: var(--bg-surface);
    /* accent-primary-text — raw accent-primary at 13px on bg-surface
       only hits 4.20:1 which fails WCAG AA for normal text. */
    color: var(--accent-primary-text);
    box-shadow: inset 0 0 0 1px var(--border-structural);
  }

  .subtitle {
    color: var(--text-secondary);
    font-size: var(--text-caption);
    margin: var(--space-1) 0 0;
  }

  /* Editor chrome (background, border, focus glow, font, padding) is now
   * owned by MarkdownEditor.svelte itself; the monospace + 14px + 16px
   * padding overrides are forwarded via the --md-* CSS variables on the
   * component invocation above. */

  /* External-update banner CSS lives in <ExternalUpdateBanner> now. */

  /* ---- Actions area ---- */

  /* Actions area = sent-status + actions row stacked with gap-based
     spacing. Mirrors /summary so the two routes look identical. */
  .actions-area {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    margin-top: var(--space-6);
  }

  .actions {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }
  /* Button cluster lives flush-right; the SaveStatus (when rendered)
     sits on the left of the row. With Back as the only button (no
     week selected) the cluster still right-aligns correctly because
     margin-left:auto applies to whichever element is the
     button-cluster. */
  .button-cluster {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }

  .btn-save {
    /* Pin the width so the "Save" → "Saving…" text swap during an
     * autosave cycle doesn't reflow the sibling buttons in the row.
     * Sized to comfortably fit "Saving…" plus normal button padding
     * (matches the value used on /summary). */
    min-width: 110px;
  }

  /* .save-status CSS lives in <SaveStatus> now. */

  /* ---- Shared ---- */

  .link-button {
    background: none;
    border: none;
    padding: 0;
    /* accent-primary-text — raw accent-primary fails AA on bg-elevated
       (sidebar header context) and is borderline on bg-base. */
    color: var(--accent-primary-text);
    cursor: pointer;
    font-family: inherit;
    font-size: inherit;
    text-decoration: underline;
  }

  .link-button:hover {
    filter: brightness(1.1);
  }

  .muted {
    /* text-muted on bg-elevated (sidebar surface) is only 4.04:1;
       text-secondary clears 5.41:1. The sidebar "Loading…" / empty
       states are the only signal of state, not decorative. */
    color: var(--text-secondary);
    font-size: var(--text-caption);
    margin: var(--space-2) 0;
  }

  .error {
    color: var(--accent-pink-text);
    font-size: var(--text-caption);
    margin: var(--space-2) 0;
  }
</style>
