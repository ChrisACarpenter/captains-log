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
  import LabelInput from '$lib/LabelInput.svelte';
  import MarkdownEditor from '$lib/MarkdownEditor.svelte';
  import ExternalUpdateBanner from '$lib/ExternalUpdateBanner.svelte';
  import SaveStatus from '$lib/SaveStatus.svelte';
  import type { AutoSaveStatus } from '$lib/save-status';
  import SendToManagerButton from '$lib/SendToManagerButton.svelte';
  import { reportDirty } from '$lib/dirty';

  type YearWeek = { year: number; week: number };

  type WeeklySummary = {
    keyAccomplishments: string;
    plansAndPriorities: string;
    challengesOrRoadblocks: string;
    anythingElse: string;
    labels: string[];
    lastUpdated: string | null;
  };

  // Send-to-manager types (SentRecord, ComposeResult) live in the
  // SendToManagerButton component now.

  // Auto-save status. 'idle' = settled, no unsaved edits and no recent save
  // to advertise. 'dirty' = typed something, debounce timer pending. 'saving'
  // = invoke in-flight. 'saved' = last write succeeded; show the timestamp.
  // 'error' = last save threw; show retry affordance.
  // AutoSaveStatus type imported from $lib/save-status (shared with
  // /journal + /capture + the SaveStatus indicator component).

  const AUTOSAVE_DEBOUNCE_MS = 1500;

  // State
  let loading = $state(true);
  let loadError = $state('');
  let saveStatus = $state<AutoSaveStatus>('idle');
  let saveErrorMessage = $state('');
  let lastSavedAt = $state<Date | null>(null);

  let yearWeek = $state<YearWeek | null>(null);
  let lastUpdated = $state<string | null>(null);

  let keyAccomplishments = $state('');
  let plansAndPriorities = $state('');
  let challengesOrRoadblocks = $state('');
  let anythingElse = $state('');
  let labels = $state<string[]>([]);

  // Last-saved snapshot. We compare the live form values against this to
  // know whether the route is "dirty" (has unsaved edits). Reset on load
  // and after a successful save.
  let snapshot = $state({
    keyAccomplishments: '',
    plansAndPriorities: '',
    challengesOrRoadblocks: '',
    anythingElse: '',
    labelsJson: '[]'
  });

  // Send-to-manager state lives inside the SendToManagerButton component.
  // The component pushes its sent-status text + stale-flag back out via
  // these bindable mirrors so we can render the "Sent Jun 26 …" line
  // above the actions row instead of inside it.
  let sentStatusText = $state('');
  let sentStatusIsStale = $state(false);

  // Cross-route file invalidation. When /journal's raw-markdown editor
  // saves the same weekly file (or /capture appends a Note), Rust emits
  // `weekly-file-changed`. We re-fetch the structured summary so the
  // four fields reflect whatever's on disk. With unsaved edits in this
  // form, we set `externalUpdate` instead of overwriting them.
  let externalUpdate = $state(false);
  let weeklyFileUnlisten: UnlistenFn | null = null;

  // Week-rollover state. When the system clock crosses into a new ISO
  // week while /summary is open (long weekend, sleep-wake, manual clock
  // change), we re-query Rust and either swap cleanly OR — if the user
  // has typed unsaved edits into the now-stale week — surface a bespoke
  // conflict banner ("save to last week, or discard?"). We track the
  // detected target separately from `externalUpdate` because the
  // resolution (write to OLD week, then swap) doesn't match the existing
  // "reload from disk" banner action.
  let rolloverPending = $state<RolloverYearWeek | null>(null);
  let focusUnlisten: UnlistenFn | null = null;

  // Phase 2.8 follow-on: drives chip rendering in LabelInput. Refreshed on
  // 'settings-changed' so toggling Theme tab updates this page live.
  let colorfulLabels = $state(false);
  let settingsUnlisten: UnlistenFn | null = null;

  async function refreshColorfulLabels(): Promise<void> {
    try {
      const s = await invoke<{ colorfulLabels?: boolean }>('get_settings');
      colorfulLabels = s.colorfulLabels ?? false;
    } catch {
      // Pre-storage / first-run — leave at default false.
    }
  }

  async function handleWeekRollover(): Promise<void> {
    await refreshCurrentWeek({
      currentYearWeek: yearWeek,
      isDirty,
      onWeekChange: (next) => void swapToWeek(next),
      onDirtyConflict: ({ newYearWeek }) => {
        rolloverPending = newYearWeek;
      }
    });
  }

  /// Clean swap to a different ISO week. Re-fetch the structured summary
  /// for the new week and rebaseline. Used both for the auto-swap branch
  /// of the rollover detector and for the "Discard" resolution of the
  /// dirty-conflict banner.
  async function swapToWeek(next: RolloverYearWeek): Promise<void> {
    try {
      const s = await invoke<WeeklySummary>('get_weekly_summary', {
        year: next.year,
        week: next.week
      });
      yearWeek = next;
      keyAccomplishments = s.keyAccomplishments;
      plansAndPriorities = s.plansAndPriorities;
      challengesOrRoadblocks = s.challengesOrRoadblocks;
      anythingElse = s.anythingElse;
      labels = s.labels ?? [];
      lastUpdated = s.lastUpdated;
      snapshot = {
        keyAccomplishments,
        plansAndPriorities,
        challengesOrRoadblocks,
        anythingElse,
        labelsJson: JSON.stringify(labels)
      };
      externalUpdate = false;
      rolloverPending = null;
      saveStatus = 'idle';
      lastSavedAt = null;
    } catch (err) {
      console.error('[summary] swap-to-week failed:', err);
    }
  }

  /// "Save to last week" — flush current dirty edits to the OLD (stale)
  /// week file, THEN swap. Used by the dirty-conflict banner's Save action.
  function onVisibilityChange(): void {
    if (!document.hidden) void handleWeekRollover();
  }
  function onWeekChangedEvent(): void {
    void handleWeekRollover();
  }

  async function rolloverSaveOldThenSwap(): Promise<void> {
    if (!rolloverPending) return;
    const next = rolloverPending;
    // saveNow uses the current `yearWeek` (still the old week) — exactly
    // what we want here. But saveNow returns synchronously when another
    // save is already in flight (it reschedules via autoSaveTimer instead),
    // so we must poll for the save to fully settle before swapping —
    // otherwise swapToWeek replaces the form fields with the new week's
    // content and edits typed during the in-flight save are lost.
    await saveNow();
    const settleStart = Date.now();
    while (saveStatus === 'saving' || saveStatus === 'dirty') {
      if (Date.now() - settleStart > 10_000) {
        console.warn('[summary] rollover-save timed out waiting for save to settle');
        return;
      }
      await new Promise((r) => setTimeout(r, 50));
    }
    if (saveStatus === 'error') return; // banner stays up; user can retry.
    await swapToWeek(next);
  }

  // Snapshot shape used to compare the FOUR fields + labels for equality.
  // Mirrors the existing `snapshot` $state. `pendingCommit` holds the
  // signature of an in-flight save so the listener can recognize our own
  // emit even when it arrives after saveStatus has flipped from 'saving'
  // to 'saved' (Tauri's invoke-response + event-emit are not strictly
  // ordered, so a pure saveStatus gate is racy and would surface false-
  // positive externalUpdate banners when the user types during a save).
  type SummarySignature = {
    keyAccomplishments: string;
    plansAndPriorities: string;
    challengesOrRoadblocks: string;
    anythingElse: string;
    labelsJson: string;
  };
  let pendingCommit = $state<SummarySignature | null>(null);

  function summariesEqual(a: SummarySignature, b: SummarySignature): boolean {
    return (
      a.keyAccomplishments === b.keyAccomplishments &&
      a.plansAndPriorities === b.plansAndPriorities &&
      a.challengesOrRoadblocks === b.challengesOrRoadblocks &&
      a.anythingElse === b.anythingElse &&
      a.labelsJson === b.labelsJson
    );
  }

  /// Mirror the normalization Rust applies on write + read:
  ///   - Each field body is trimmed both ends (notes.rs:render_weekly_summary
  ///     writes via trim_body / trim_end; extract_subsection trims both
  ///     ends on read).
  ///   - Each label is trimmed, leading '#' chars stripped, then empties
  ///     filtered out (commands.rs update_weekly_summary + notes.rs label
  ///     parse).
  /// Used ONLY for the disk-vs-baseline equality check in
  /// reconcileWithDisk — never applied to the form fields or the
  /// in-memory snapshot, so the user's trailing whitespace and "#release"
  /// label form survive in the editor between saves. The pre-normalize
  /// snapshot stays pre-normalize (so isDirty correctly compares form to
  /// what was last typed); we only normalize at compare time so a
  /// normalization-only difference between disk (post-normalize) and
  /// pre-normalize snapshot doesn't surface as a false "modified
  /// externally" banner OR silently rewrite the user's fields on every
  /// own-save echo.
  function normalizedSig(s: SummarySignature): SummarySignature {
    let labels: string[];
    try {
      labels = JSON.parse(s.labelsJson) as string[];
    } catch {
      labels = [];
    }
    return {
      keyAccomplishments: s.keyAccomplishments.trim(),
      plansAndPriorities: s.plansAndPriorities.trim(),
      challengesOrRoadblocks: s.challengesOrRoadblocks.trim(),
      anythingElse: s.anythingElse.trim(),
      labelsJson: JSON.stringify(
        labels
          .map((l) => l.trim().replace(/^#+/, '').trim())
          .filter((l) => l.length > 0)
      )
    };
  }

  const isDirty = $derived(
    !loading &&
      (keyAccomplishments !== snapshot.keyAccomplishments ||
        plansAndPriorities !== snapshot.plansAndPriorities ||
        challengesOrRoadblocks !== snapshot.challengesOrRoadblocks ||
        anythingElse !== snapshot.anythingElse ||
        JSON.stringify(labels) !== snapshot.labelsJson)
  );

  const pushDirty = reportDirty('summary', 'the weekly summary');
  $effect(() => pushDirty(isDirty));

  // Auto-save: debounced 1.5s after typing stops. We touch the inputs
  // explicitly so this effect re-runs on every keystroke (a derived bool
  // like isDirty wouldn't, since once it's true it stays true).
  let autoSaveTimer: ReturnType<typeof setTimeout> | null = null;

  $effect(() => {
    keyAccomplishments;
    plansAndPriorities;
    challengesOrRoadblocks;
    anythingElse;
    labels;

    if (loading) return;
    if (!isDirty) return;

    // Read + write saveStatus inside `untrack` so this effect stays
    // keystroke-driven — WITHOUT untrack the read on line `saveStatus
    // !== 'saving'` adds saveStatus as a reactive dep, so every state
    // flip during a save cycle ('dirty' → 'saving' → 'saved') re-fires
    // the effect and REARMS the autoSaveTimer. That stale timer then
    // triggers a redundant second save 1.5s later, which is the
    // button-flicker Chris was seeing (the buttons cycling
    // disabled→enabled→disabled→enabled).
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

  // Computed week range label like "Week of June 22 – June 28, 2026".
  // ISO-week math lives in $lib/weekRollover.ts (shared with /journal).
  const weekLabel = $derived(yearWeek ? formatWeekRange(yearWeek) : '');

  onMount(async () => {
    try {
      yearWeek = await invoke<YearWeek>('get_current_year_week');
      const s = await invoke<WeeklySummary>('get_weekly_summary', {
        year: yearWeek.year,
        week: yearWeek.week
      });
      keyAccomplishments = s.keyAccomplishments;
      plansAndPriorities = s.plansAndPriorities;
      challengesOrRoadblocks = s.challengesOrRoadblocks;
      anythingElse = s.anythingElse;
      labels = s.labels ?? [];
      lastUpdated = s.lastUpdated;
      // Baseline the dirty-comparison snapshot to what we just loaded.
      snapshot = {
        keyAccomplishments,
        plansAndPriorities,
        challengesOrRoadblocks,
        anythingElse,
        labelsJson: JSON.stringify(labels)
      };
      // Send-to-manager state (sentRecord, currentHash, managerEmail)
      // is owned by the SendToManagerButton component now — it loads
      // its own state from year/week on mount.
    } catch (err) {
      loadError = String(err);
    } finally {
      loading = false;
    }

    // Week-rollover triggers. Three independent signals because no single
    // one catches every case:
    //   - tauri://focus: user switches back to the app window (the common
    //     "returned from weekend" case).
    //   - visibilitychange: webview becomes visible (covers some Spaces /
    //     full-screen transitions where focus doesn't fire).
    //   - captainslog:week-changed: the WeekStripe 60s tick noticed the
    //     ISO week roll over while the app was foreground.
    focusUnlisten = await getCurrentWindow().listen('tauri://focus', () => {
      void handleWeekRollover();
    });
    document.addEventListener('visibilitychange', onVisibilityChange);
    window.addEventListener('captainslog:week-changed', onWeekChangedEvent);

    await refreshColorfulLabels();
    settingsUnlisten = await listen('settings-changed', () => {
      void refreshColorfulLabels();
    });

    // Subscribe to the cross-route file-changed broadcast. Any writer to
    // the same (year, week) markdown file emits this from Rust. We only
    // act when the changed week matches what we have open (always the
    // current week for /summary).
    weeklyFileUnlisten = await listen<{ year: number; week: number }>(
      'weekly-file-changed',
      async (event) => {
        if (!yearWeek) return;
        if (
          event.payload.year !== yearWeek.year ||
          event.payload.week !== yearWeek.week
        ) {
          return;
        }
        // Don't gate on saveStatus here: Tauri's invoke-response and event
        // emit travel separate IPC paths, so by the time this listener
        // runs saveStatus may have already flipped to 'saved'. Instead,
        // reconcileWithDisk compares the disk-loaded summary to BOTH
        // `snapshot` (post-baseline) and `pendingCommit` (pre-baseline)
        // to recognize our own emit. Suppressing on saveStatus alone
        // caused a false-positive externalUpdate banner — and a
        // destructive Reload — when the user typed during their own save.
        await reconcileWithDisk();
      }
    );
  });

  /// Re-fetch the structured summary from disk and merge with the form.
  ///   1. Disk == snapshot (or == pendingCommit) → silent no-op. The
  ///      disk matches either our last-known baseline or the bytes our
  ///      currently-in-flight save is writing — own emit echo, no
  ///      reconcile needed.
  ///   2. Disk != baseline, !isDirty → silent reload; replace fields +
  ///      baseline.
  ///   3. Disk != baseline, isDirty → set externalUpdate = true. Banner
  ///      lets the user pick Reload (lose edits) or keep typing (next
  ///      save overwrites the external change).
  ///
  /// Why compare to `snapshot` and not to the live form fields: the user
  /// may have typed during an own-save's IPC roundtrip. Their newer
  /// keystroke makes form != snapshot, so a `disk === form` check would
  /// false-positive every time they type-through-save and surface a
  /// misleading banner. The baseline + pendingCommit pair is invariant
  /// during typing — only saveNow itself moves them.
  async function reconcileWithDisk(): Promise<void> {
    if (!yearWeek) return;
    try {
      const s = await invoke<WeeklySummary>('get_weekly_summary', {
        year: yearWeek.year,
        week: yearWeek.week
      });
      // The disk-loaded signature is already post-normalization (Rust's
      // extract_subsection trims; label parse strips '#' + drops empties).
      // We normalize the snapshot + pendingCommit at compare-time so a
      // pre-normalize-vs-post-normalize delta is treated as "no real
      // change". See `normalizedSig` for the rationale.
      const diskSig: SummarySignature = {
        keyAccomplishments: s.keyAccomplishments,
        plansAndPriorities: s.plansAndPriorities,
        challengesOrRoadblocks: s.challengesOrRoadblocks,
        anythingElse: s.anythingElse,
        labelsJson: JSON.stringify(s.labels ?? [])
      };
      if (
        summariesEqual(diskSig, normalizedSig(snapshot)) ||
        (pendingCommit && summariesEqual(diskSig, normalizedSig(pendingCommit)))
      ) {
        externalUpdate = false;
        // Refresh lastUpdated even on a no-op — external writers can bump
        // the timestamp without changing field contents (rare but possible).
        lastUpdated = s.lastUpdated;
        return;
      }
      if (isDirty) {
        externalUpdate = true;
        return;
      }
      // Clean form, but disk differs — adopt the new content silently.
      keyAccomplishments = s.keyAccomplishments;
      plansAndPriorities = s.plansAndPriorities;
      challengesOrRoadblocks = s.challengesOrRoadblocks;
      anythingElse = s.anythingElse;
      labels = s.labels ?? [];
      lastUpdated = s.lastUpdated;
      snapshot = {
        keyAccomplishments,
        plansAndPriorities,
        challengesOrRoadblocks,
        anythingElse,
        labelsJson: JSON.stringify(labels)
      };
      externalUpdate = false;
    } catch (err) {
      console.error('[summary] reconcile failed:', err);
    }
  }

  /// Discard local edits and adopt whatever's on disk now. Wired to the
  /// "Reload" button on the external-update banner.
  async function reloadFromDisk(): Promise<void> {
    if (!yearWeek) return;
    try {
      const s = await invoke<WeeklySummary>('get_weekly_summary', {
        year: yearWeek.year,
        week: yearWeek.week
      });
      keyAccomplishments = s.keyAccomplishments;
      plansAndPriorities = s.plansAndPriorities;
      challengesOrRoadblocks = s.challengesOrRoadblocks;
      anythingElse = s.anythingElse;
      labels = s.labels ?? [];
      lastUpdated = s.lastUpdated;
      snapshot = {
        keyAccomplishments,
        plansAndPriorities,
        challengesOrRoadblocks,
        anythingElse,
        labelsJson: JSON.stringify(labels)
      };
      externalUpdate = false;
      saveStatus = 'idle';
    } catch (err) {
      loadError = String(err);
    }
  }

  onDestroy(() => {
    // Cancel any pending autosave timer so it can't fire saveNow on a
    // destroyed component. The reschedule arm in saveNow's gate can
    // leave a chained timer active even after the user clicks Done
    // (which navigates immediately, only blocked by saveStatus === 'saving').
    if (autoSaveTimer) {
      clearTimeout(autoSaveTimer);
      autoSaveTimer = null;
    }
    weeklyFileUnlisten?.();
    weeklyFileUnlisten = null;
    focusUnlisten?.();
    focusUnlisten = null;
    settingsUnlisten?.();
    settingsUnlisten = null;
    document.removeEventListener('visibilitychange', onVisibilityChange);
    window.removeEventListener('captainslog:week-changed', onWeekChangedEvent);
  });

  /// Save the current form to disk. Used by both the auto-save debounce and
  /// the manual Save button + Cmd+S / Cmd+↩ shortcuts. Idempotent: returns
  /// early if a save is already in flight.
  async function saveNow() {
    if (!yearWeek) return;
    if (saveStatus === 'saving') {
      // Another save is mid-invoke. The single-slot pendingCommit can't
      // hold both the in-flight bytes AND the new edits. Reschedule
      // ourselves so the new content saves AFTER the current save
      // settles — the new $effect-guard above keeps saveStatus = 'saving'
      // through completion, so this branch fires for every typing
      // round-trip while saving.
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

    // Capture what we're about to save BEFORE the await so we can re-baseline
    // the snapshot to exactly what hit disk — even if the user keeps typing
    // during the await, isDirty will correctly flip back to true after this
    // save completes and trigger another auto-save.
    const committed = {
      keyAccomplishments,
      plansAndPriorities,
      challengesOrRoadblocks,
      anythingElse,
      labelsJson: JSON.stringify(labels),
      labels: [...labels]
    };

    // Declare intent for the cross-route listener BEFORE invoke so it
    // can recognize our own emit even when the event arrives before
    // (or co-incident with) the post-save snapshot baseline. Cleared
    // in finally regardless of outcome.
    pendingCommit = {
      keyAccomplishments: committed.keyAccomplishments,
      plansAndPriorities: committed.plansAndPriorities,
      challengesOrRoadblocks: committed.challengesOrRoadblocks,
      anythingElse: committed.anythingElse,
      labelsJson: committed.labelsJson
    };

    saveStatus = 'saving';
    saveErrorMessage = '';
    try {
      await invoke('update_weekly_summary', {
        input: {
          year: yearWeek.year,
          week: yearWeek.week,
          keyAccomplishments: committed.keyAccomplishments,
          plansAndPriorities: committed.plansAndPriorities,
          challengesOrRoadblocks: committed.challengesOrRoadblocks,
          anythingElse: committed.anythingElse,
          labels: committed.labels
        }
      });
      // Refresh lastUpdated from the server (avoids drift from frontend clock).
      const refreshed = await invoke<WeeklySummary>('get_weekly_summary', {
        year: yearWeek.year,
        week: yearWeek.week
      });
      lastUpdated = refreshed.lastUpdated;
      snapshot = {
        keyAccomplishments: committed.keyAccomplishments,
        plansAndPriorities: committed.plansAndPriorities,
        challengesOrRoadblocks: committed.challengesOrRoadblocks,
        anythingElse: committed.anythingElse,
        labelsJson: committed.labelsJson
      };
      // The "Send button edited-since gate" used to live here as an
      // inline get_summary_hash refresh. That state moved to
      // SendToManagerButton, which listens for `weekly-file-changed`
      // (which Rust emits after update_weekly_summary completes) and
      // re-fetches its own hash + sentRecord automatically — no
      // per-route coordination needed.
      lastSavedAt = new Date();
      saveStatus = 'saved';
    } catch (err) {
      saveErrorMessage = String(err);
      saveStatus = 'error';
    } finally {
      pendingCommit = null;
    }
  }

  // formatTime + saveStatusText now live inside <SaveStatus>.

  function handleKeydown(e: KeyboardEvent) {
    // Send-modal Escape + Cmd-S swallow handling lives inside
    // SendToManagerButton now (it owns its own window keydown listener
    // for its modal state).
    if ((e.metaKey || e.ctrlKey) && e.key === 's') {
      e.preventDefault();
      void saveNow();
    } else if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
      e.preventDefault();
      void saveNow();
    }
  }

  // ---------- Send to manager ----------
  //
  // Send-to-manager state machine, gating, compose flow, modal, and
  // sent-status display are all owned by SendToManagerButton now.
</script>

<svelte:window onkeydown={handleKeydown} />

{#if loading}
  <main class="loading">
    <p>Loading…</p>
  </main>
{:else if loadError}
  <main class="loading">
    <div class="card is-narrow error-card">
      <h2>Couldn't load this week.</h2>
      <p>{loadError}</p>
      <button class="btn btn-marble" onclick={() => goto('/')}>Back</button>
    </div>
  </main>
{:else}
  <main>
    <section class="page">
      <header>
        <h1>Weekly Summary</h1>
        <p class="subtitle">{weekLabel}</p>
        {#if lastUpdated}
          <p class="last-updated">Last updated: {lastUpdated}</p>
        {/if}
      </header>

      {#if externalUpdate}
        <ExternalUpdateBanner
          onReload={() => void reloadFromDisk()}
          onDismiss={() => (externalUpdate = false)}
        >
          This week was modified outside this view (likely from /journal
          or a quick-capture note). Your unsaved edits would overwrite
          it on the next save.
        </ExternalUpdateBanner>
      {/if}

      {#if rolloverPending}
        <!-- Week-rollover dirty-conflict banner. Reuses the same component
             as the external-update banner — visually consistent — but
             rewires the actions: "Reload" means "save these edits to LAST
             week, then swap", and "Dismiss" means "discard the edits and
             swap to the new week." We override the action label via the
             banner's only customization point (the children Snippet) by
             leaning on the existing component shape; the action button
             reads "Reload (lose my edits)" by default, so we explain the
             choice in the body copy. -->
        <ExternalUpdateBanner
          onReload={() => void rolloverSaveOldThenSwap()}
          onDismiss={() => rolloverPending && void swapToWeek(rolloverPending)}
        >
          A new week started. Save these edits to last week, or discard?
          Reload saves them to the previous week then opens the current
          one; Dismiss discards them and opens the current week.
        </ExternalUpdateBanner>
      {/if}

      <div class="form">
        <!-- Phase 2.5 Step 4: each field is a CodeMirror MarkdownEditor.
          Native WebKit spell-check + clickable Markdown links + GFM
          parsing for free; auto-save flow stays the same because the
          editor's onChange wires straight into the existing dirty/$effect
          debounce. The `style="--md-min-height: ..."` numbers approximate
          the prior `rows={3|4|5}` initial heights (~22px line-height
          + 24px vertical padding). The editor scrolls internally when
          content exceeds; resize: vertical on the wrapper lets the user
          drag-grow each field, matching the textarea-era affordance. -->
        <div class="field">
          <label for="key-acc">Key accomplishments…</label>
          <MarkdownEditor
            id="key-acc"
            value={keyAccomplishments}
            onChange={(v) => (keyAccomplishments = v)}
            placeholder=""
            livePreview
            style="--md-min-height: 112px; resize: vertical; overflow: hidden;"
          />
        </div>

        <div class="field">
          <label for="plans">Plans and priorities for next week…</label>
          <MarkdownEditor
            id="plans"
            value={plansAndPriorities}
            onChange={(v) => (plansAndPriorities = v)}
            placeholder=""
            livePreview
            style="--md-min-height: 112px; resize: vertical; overflow: hidden;"
          />
        </div>

        <div class="field">
          <label for="challenges">Challenges or roadblocks…</label>
          <MarkdownEditor
            id="challenges"
            value={challengesOrRoadblocks}
            onChange={(v) => (challengesOrRoadblocks = v)}
            placeholder=""
            livePreview
            style="--md-min-height: 112px; resize: vertical; overflow: hidden;"
          />
        </div>

        <div class="field">
          <label for="else">Anything else on your mind…</label>
          <MarkdownEditor
            id="else"
            value={anythingElse}
            onChange={(v) => (anythingElse = v)}
            placeholder=""
            livePreview
            style="--md-min-height: 112px; resize: vertical; overflow: hidden;"
          />
        </div>

        <div class="field">
          <span class="field-heading">Labels</span>
          <LabelInput
            bind:labels
            placeholder="Tag this week (type to search, Enter to add)"
            {colorfulLabels}
          />
        </div>
      </div>

      {#if saveStatus === 'error' && saveErrorMessage}
        <p class="status status-error">Error: {saveErrorMessage}</p>
      {/if}

        <!-- Phase 2.7 button-pass layout: Save (primary) leftmost, then
           Back (ruby/destructive, was "Done"), then Send to manager. Save
           status anchors all the way left so autosave indicators sit in
           one consistent spot across journal/summary/capture. The
           .actions-area wrapper owns the spacing above the action row
           AND the gap between the sent-status line + the buttons —
           gap-based instead of margin-based so the visual rhythm
           survives both block (this route) and flex (/journal) parent
           contexts. -->
        <div class="actions-area">
          {#if sentStatusText}
            <p class="sent-status" class:is-stale={sentStatusIsStale}>
              {sentStatusText}
            </p>
          {/if}
          <div class="actions">
          <SaveStatus
            status={saveStatus}
            lastSavedAt={lastSavedAt}
            onRetry={() => void saveNow()}
          />
          <!-- Save + Back are NOT visually disabled during 'saving'.
               Local disk saves complete in ~10ms — well under one 60fps
               frame — so any class/attribute toggle for that window
               would show up as WKWebView paint flicker without ever
               actually communicating anything useful to the user.
               saveNow gates internally on `saveStatus === 'saving'`
               and its rescheduling arm handles typing-through-save,
               so double-clicks during the 10ms window are safe. -->
          <button
            class="btn btn-emerald btn-save"
            onclick={() => void saveNow()}
          >
            Save
          </button>
          <button
            class="btn btn-ruby"
            onclick={() => goto('/')}
          >
            Back
          </button>
          <!-- Send to manager (Phase 2.6). Marble so it doesn't compete
            with the primary Save action. Component owns its own gating,
            sent-status display, and confirmation modal. -->
          {#if yearWeek}
            <SendToManagerButton
              year={yearWeek.year}
              week={yearWeek.week}
              {weekLabel}
              {isDirty}
              {saveStatus}
              bind:sentStatusText
              bind:sentStatusIsStale
            />
          {/if}
        </div>
      </div>
    </section>
  </main>


  <!-- Confirmation modal for the Send-to-manager flow lives inside the
     SendToManagerButton component now. -->
{/if}

<style>
  main {
    display: flex;
    justify-content: center;
    padding: var(--space-8) var(--space-4);
    min-height: 100vh;
  }

  main.loading {
    align-items: center;
  }

  .page {
    width: 100%;
    max-width: 720px;
  }

  header {
    margin-bottom: var(--space-8);
  }

  .subtitle {
    color: var(--text-secondary);
    margin-top: var(--space-2);
  }

  .last-updated {
    color: var(--text-muted);
    font-size: var(--text-caption);
    line-height: var(--text-caption-lh);
    margin-top: var(--space-1);
  }

  /* External-update banner CSS lives in <ExternalUpdateBanner> now. */

  .form {
    display: flex;
    flex-direction: column;
    gap: var(--space-6);
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .field > label,
  .field > .field-heading {
    font-family: var(--font-display);
    font-size: var(--text-button);
    color: var(--text-primary);
  }

  /* Editor chrome (background, border, focus glow, font, line-height) is
   * owned by MarkdownEditor.svelte itself. Per-field initial height +
   * user-resize affordance is set inline on each MarkdownEditor's `style`
   * via --md-min-height + `resize: vertical`. */

  /* Actions area = sent-status + actions row stacked with gap-based
     spacing. Sits as a direct child of .page (block-flow) so its margin
     above isolates from .form's flex-column gap that was previously
     adding visible doubled spacing on this route. */
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

  .btn-save {
    margin-left: auto;
    /* Pin the width so the "Save" → "Saving…" text swap during an
     * autosave cycle doesn't reflow the sibling buttons in the row.
     * Sized to comfortably fit "Saving…" plus normal button padding. */
    min-width: 110px;
  }

  /* Send button + sent-status + modal CSS live in <SendToManagerButton>
     now. .save-status lives in <SaveStatus>. */

  /* Save-error banner above the actions row uses the shared .status +
     .status-error utility from app.css. */

  /* .card + is-narrow + .card h2 / .card p live in app.css as a shared
     utility. This route only adds the pink-tint overlay for the load-
     error placeholder. */
  .error-card {
    background: var(--bg-error-tint-soft);
    border-color: var(--border-error);
  }
</style>
