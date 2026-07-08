<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { goto } from '$app/navigation';
  import { invoke } from '@tauri-apps/api/core';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { getCurrentWindow } from '@tauri-apps/api/window';

  import Wizard from '$lib/onboarding/Wizard.svelte';
  import TipBubble from '$lib/onboarding/TipBubble.svelte';
  import Modal from '$lib/Modal.svelte';
  import InputField from '$lib/InputField.svelte';
  import RolloverReceipt from '$lib/RolloverReceipt.svelte';

  type ReminderSettings = {
    enabled: boolean;
    daysOfWeek: number[];
    hour: number;
    minute: number;
  };

  type TaskListSettings = {
    showCompleted: boolean;
    openTasksFirst: boolean;
    showCompletedTimestamp: boolean;
    hideTaskList: boolean;
    autoRolloverEnabled: boolean;
  };

  type Settings = {
    firstRun: boolean;
    journalRoot: string;
    defaultJournalRoot: string;
    userName: string | null;
    reminder: ReminderSettings;
    taskList: TaskListSettings;
  };

  /**
   * Task-list display defaults for the moment settings haven't loaded
   * yet (or the backend returned a payload predating Slice 4). Mirrors
   * `TaskListSettings::default()` on the Rust side.
   */
  const TASK_LIST_DEFAULTS: TaskListSettings = {
    showCompleted: true,
    openTasksFirst: true,
    showCompletedTimestamp: false,
    hideTaskList: false,
    autoRolloverEnabled: true,
  };

  // Shape mirrors the backend `TaskListEntry` (see
  // src-tauri/src/commands.rs). Field names use camelCase because Tauri
  // v2 auto-converts snake_case Rust → camelCase JS at the IPC boundary.
  //
  // `textHtml` is server-sanitized (pulldown-cmark + ammonia inline-only
  // allowlist) so it's safe to render via {@html}. `text` is retained
  // for identity/testing; the UI never displays it directly.
  type TaskListEntry = {
    year: number;
    week: number;
    text: string;
    textHtml: string;
    textHash: string;
    ordinal: number;
    isCompleted: boolean;
    completedAt: string | null;
    /**
     * Slice 5 provenance surface. `null` for tasks that live in the
     * week they were born (the common case). Populated when the task
     * was rolled forward from a prior ISO week — the row renders a
     * "from Wxx" chip so users see the paper trail.
     */
    originalWeek: { year: number; week: number } | null;
  };

  // ---- State ----

  let settings = $state<Settings | null>(null);
  let loading = $state(true);
  let loadError = $state('');

  // Slice 1 task list — read-only view of the current week's Plans
  // section. `tasksLoaded` gates rendering so we don't flash the empty
  // state before the invoke resolves. `tasksError` is set (and surfaced
  // via the empty-state tip) if the IPC call itself fails; parse errors
  // never bubble up here because the backend is defensive.
  let tasks = $state<TaskListEntry[]>([]);
  let tasksLoaded = $state(false);
  let tasksError = $state('');

  // Slice 2 toggle state:
  //   togglingKeys — set of task keys with an in-flight toggle_task
  //     IPC; used to disable the button + prevent double-clicks
  //   toggleError — user-facing text for the most recent toggle
  //     failure, cleared on the next successful toggle. Rendered as
  //     a TipBubble above the task list.
  // Record<string, boolean>: `true` = toggle in flight, `false` (or
  // absent) = idle. We ASSIGN false on completion rather than
  // `delete`ing the key so Svelte 5's proxy reliably notifies the
  // disabled binding — property-set is a first-class trap, whereas
  // deleteProperty can be trickier to reason about across renders.
  let togglingKeys = $state<Record<string, boolean>>({});
  let toggleError = $state('');

  // Slice 3 add-task state. Modal opens via the "+ Add task" button in
  // the task-list header. `addingTask` disables the submit button
  // during the IPC. `addError` renders inline in the modal body via
  // InputField's `warning` prop.
  let showAddTaskModal = $state(false);
  let addTaskText = $state('');
  let addingTask = $state(false);
  let addError = $state('');

  /** How long to wait for either IPC before we consider it hung. In a
   *  local Tauri app the write is nearly instant; 30s exists purely as
   *  a rescue timer so the user isn't stuck staring at a disabled
   *  button if the backend deadlocks. */
  const ADD_TASK_TIMEOUT_MS = 30_000;

  function withTimeout<T>(promise: Promise<T>, ms: number, label: string): Promise<T> {
    return new Promise((resolve, reject) => {
      const timer = setTimeout(
        () => reject(new Error(`${label} timed out after ${ms / 1000}s — try again.`)),
        ms,
      );
      promise.then(
        (v) => {
          clearTimeout(timer);
          resolve(v);
        },
        (e) => {
          clearTimeout(timer);
          reject(e);
        },
      );
    });
  }

  function openAddTask(): void {
    // Guard against double-open from rapid clicks so the "reset the
    // input" side effect doesn't run twice on top of user-entered
    // text if the button somehow gets a second event before the
    // modal renders.
    if (showAddTaskModal) return;
    addTaskText = '';
    addError = '';
    showAddTaskModal = true;
  }

  function closeAddTask(): void {
    if (addingTask) return; // safety net — Modal also honors blockDismissal
    showAddTaskModal = false;
    addError = '';
  }

  async function submitAddTask(): Promise<void> {
    if (addingTask) return;
    // Trim client-side so an all-whitespace input errors out before
    // the round-trip. Backend enforces the same rule as a safety net.
    if (addTaskText.trim().length === 0) {
      addError = "Task text can't be empty.";
      return;
    }
    addingTask = true;
    addError = '';
    try {
      // Order matters: append THEN refetch, but only close the modal
      // AFTER the refetch succeeds. If we close on append success
      // alone and the refetch fails, the user sees the modal
      // disappear (a success signal) while the landing-page list is
      // stale — and re-adding the task now creates a duplicate on the
      // backend.
      await withTimeout(
        invoke('append_task_to_current_week', { text: addTaskText }),
        ADD_TASK_TIMEOUT_MS,
        'Add Task',
      );
      tasks = await withTimeout(
        invoke<TaskListEntry[]>('list_tasks'),
        ADD_TASK_TIMEOUT_MS,
        'Refresh task list',
      );
      showAddTaskModal = false;
      addTaskText = '';
    } catch (err) {
      addError = String(err);
    } finally {
      addingTask = false;
    }
  }

  type TaskToggleResult = {
    isCompleted: boolean;
    completedAt: string | null;
  };

  function taskKey(t: TaskListEntry): string {
    return `${t.year}-${t.week}-${t.textHash}-${t.ordinal}`;
  }

  /**
   * Compact chip label for the origin week of a rolled-over task.
   * Shows "from last week" when the origin is directly one ISO week
   * behind the current entry — otherwise "from W{n}" (same year) or
   * "from W{n}, {year}" (different year). Kept short so it doesn't
   * dominate the row visually.
   */
  function formatOriginLabel(t: TaskListEntry): string {
    if (!t.originalWeek) return '';
    const { year: oy, week: ow } = t.originalWeek;
    const isDirectlyPrior =
      (oy === t.year && ow === t.week - 1) || (oy === t.year - 1 && t.week === 1);
    if (isDirectlyPrior) return 'from last week';
    if (oy === t.year) return `from W${ow}`;
    return `from W${ow}, ${oy}`;
  }

  // Slice 4 toggle-derived view. Filters completed tasks out when the
  // user has that off, then sorts open tasks above completed ones
  // when that toggle is on. Sort is stable (preserves file order
  // within each partition) via Array.prototype.sort's guarantee.
  const taskListPrefs = $derived<TaskListSettings>(
    settings?.taskList ?? TASK_LIST_DEFAULTS,
  );
  const visibleTasks = $derived.by((): TaskListEntry[] => {
    let list = tasks;
    if (!taskListPrefs.showCompleted) {
      list = list.filter((t) => !t.isCompleted);
    }
    if (taskListPrefs.openTasksFirst) {
      // .sort() mutates — clone first so `tasks` (the source) stays
      // in its native file-order for the derived-recompute cycle.
      list = [...list].sort((a, b) => {
        if (a.isCompleted === b.isCompleted) return 0;
        return a.isCompleted ? 1 : -1;
      });
    }
    return list;
  });

  /**
   * Format an ISO 8601 completedAt string as a "checked Xm/h/d ago"
   * label. Deliberately coarse — we're rendering into a small chip
   * next to the task text, not surfacing a full timeline.
   *
   * Returns an empty string if the timestamp is missing or unparseable
   * so the caller can `{#if formatted}` past the label without
   * rendering an ugly "checked NaN ago" chip.
   */
  function formatRelativeCompleted(iso: string | null): string {
    if (!iso) return '';
    const then = Date.parse(iso);
    if (Number.isNaN(then)) return '';
    const deltaMs = Date.now() - then;
    // Future-dated completedAt (clock skew, timezone weirdness) —
    // fall back to a plain "checked" chip rather than announcing a
    // negative duration.
    if (deltaMs < 0) return 'checked';
    const minutes = Math.floor(deltaMs / 60_000);
    if (minutes < 1) return 'checked just now';
    if (minutes < 60) return `checked ${minutes}m ago`;
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return `checked ${hours}h ago`;
    const days = Math.floor(hours / 24);
    if (days < 7) return `checked ${days}d ago`;
    const weeks = Math.floor(days / 7);
    if (weeks < 5) return `checked ${weeks}w ago`;
    // Beyond a month, drop back to a date so "checked 12w ago" doesn't
    // stack into "48w ago" for old entries backfilled by rebuild.
    return `checked ${iso.slice(0, 10)}`;
  }

  async function onToggle(t: TaskListEntry): Promise<void> {
    const key = taskKey(t);
    if (togglingKeys[key]) return;
    togglingKeys[key] = true;
    try {
      const result = await invoke<TaskToggleResult>('toggle_task', {
        year: t.year,
        week: t.week,
        textHash: t.textHash,
        ordinal: t.ordinal,
      });
      // Update the row in place. Svelte 5's $state proxies array
      // element writes, so the row rerenders without a full list
      // refetch.
      const idx = tasks.findIndex((x) => taskKey(x) === key);
      if (idx !== -1) {
        tasks[idx].isCompleted = result.isCompleted;
        tasks[idx].completedAt = result.completedAt;
      }
      toggleError = '';
    } catch (err) {
      // Common cause: the user edited the summary in another window
      // between load and click, so the task no longer exists at that
      // (hash, ordinal). Surface the error and refetch to reconcile.
      toggleError = String(err);
      try {
        tasks = await invoke<TaskListEntry[]>('list_tasks');
      } catch {
        // Refetch itself failed — leave the list as-is; user can
        // reload the page. Don't clobber the more useful
        // toggle-error message with a refetch-error message.
      }
    } finally {
      togglingKeys[key] = false;
    }
  }

  // Refetch settings — used both on mount and whenever the backend
  // broadcasts settings-changed (Settings tab save, first-run wizard
  // hot-swap, etc.).
  async function refetchSettings(): Promise<void> {
    try {
      settings = await invoke<Settings>('get_settings');
    } catch (err) {
      loadError = String(err);
    }
  }

  async function refetchTasks(): Promise<void> {
    try {
      tasks = await invoke<TaskListEntry[]>('list_tasks');
      tasksError = '';
    } catch (err) {
      tasksError = String(err);
    }
  }

  // Handles for the Tauri event listeners so onDestroy can clean up.
  // If a listener promise is still resolving when onDestroy fires
  // (rare), the fallback else-branch below tears it down as soon as
  // it lands.
  let unlistenSettingsChanged: UnlistenFn | null = null;
  let unlistenTaskIndexRebuilt: UnlistenFn | null = null;
  let unlistenFocus: UnlistenFn | null = null;
  /**
   * Safety-net interval for the "user leaves app focused on /
   * across an ISO week boundary" case. WeekStripe (which fires the
   * `captainslog:week-changed` custom event on its own 60s tick) is
   * only mounted on /journal and /summary, so a user idle on the
   * landing page all weekend wouldn't otherwise trigger rollover.
   * 60s is cheap: `check_and_apply_rollover` is idempotent, so most
   * ticks are a no-op backend load-and-return.
   */
  let rolloverSafetyInterval: ReturnType<typeof setInterval> | null = null;

  // Slice 5 rollover state.
  //   rolloverReceipt — when non-null, the RolloverReceipt component
  //     renders above the task list with these details. Cleared on
  //     auto-dismiss or manual close.
  //   rolloverInFlight — guards against concurrent invocations from
  //     multiple triggers (focus + visibility fire back-to-back on
  //     Space switches). The backend is idempotent anyway, but the
  //     frontend guard avoids two "refetch tasks" reloads chasing
  //     each other.
  type RolloverApplied = {
    applied: boolean;
    tasksCopied: number;
    sourceWeek: { year: number; week: number } | null;
    targetWeek: { year: number; week: number };
  };
  let rolloverReceipt = $state<{ tasksCopied: number; sourceLabel: string } | null>(null);
  let rolloverInFlight = false;

  /**
   * Format the rollover source week as a friendly label. When the
   * source is directly the ISO week before the current one, we say
   * "last week"; otherwise the explicit "Week N, YYYY" reads better
   * than "the ISO week preceding …".
   */
  function formatSourceLabel(
    source: { year: number; week: number },
    target: { year: number; week: number },
  ): string {
    const isDirectlyPrior =
      (source.year === target.year && source.week === target.week - 1) ||
      (source.year === target.year - 1 && target.week === 1);
    if (isDirectlyPrior) return 'last week';
    return `Week ${source.week}, ${source.year}`;
  }

  async function checkAndApplyRollover(): Promise<void> {
    // Skip entirely if the user turned it off, or if the previous
    // invoke is still resolving. Skip in first-run state too — the
    // wizard hasn't yet established a journal root.
    if (rolloverInFlight) return;
    if (loading || settings === null || settings.firstRun) return;
    if (!taskListPrefs.autoRolloverEnabled) return;
    rolloverInFlight = true;
    try {
      const result = await invoke<RolloverApplied>('check_and_apply_rollover');
      if (result.applied && result.tasksCopied > 0 && result.sourceWeek) {
        rolloverReceipt = {
          tasksCopied: result.tasksCopied,
          sourceLabel: formatSourceLabel(result.sourceWeek, result.targetWeek),
        };
        // Tasks in the current week changed on disk — refetch so
        // the list re-renders with the rolled-over items.
        await refetchTasks();
      }
    } catch (err) {
      // Rollover is best-effort. Don't block the UI on it.
      // eslint-disable-next-line no-console
      console.error('[rollover] check failed:', err);
    } finally {
      rolloverInFlight = false;
    }
  }

  function onVisibilityChange(): void {
    if (!document.hidden) void checkAndApplyRollover();
  }

  function onWeekChangedEvent(): void {
    void checkAndApplyRollover();
  }

  onMount(async () => {
    try {
      settings = await invoke<Settings>('get_settings');
    } catch (err) {
      loadError = String(err);
    } finally {
      loading = false;
    }
    // Fire this after settings so a first-run wizard never briefly
    // shows a task-list stub. If we're firstRun the section won't
    // render at all — the extra invoke is wasted but harmless.
    await refetchTasks();
    tasksLoaded = true;

    // Slice 5 rollover — kick off a check on mount. The backend
    // command is idempotent (sidecar tracks last_run_to_week), so
    // this is safe to fire on every open even when we've already
    // rolled over for the current week.
    void checkAndApplyRollover();

    // Cross-window sync: when the Settings tab saves a change, refetch
    // settings so `taskListPrefs` picks up the new toggles without a
    // page reload. Matches the pattern /summary and /capture use.
    unlistenSettingsChanged = await listen('settings-changed', () => {
      void refetchSettings();
    });
    // Rebuild task index writes only to the sidecar — no
    // weekly-file-changed emission — so subscribe to a dedicated
    // signal that tells the landing page its task-list data is stale.
    unlistenTaskIndexRebuilt = await listen('task-index-rebuilt', () => {
      void refetchTasks();
    });

    // Rollover triggers — mirror the three signals /journal and
    // /summary already use. Together they catch "user came back
    // to the app after being away" for every realistic path: cold
    // start, alt-tab back into a still-open window, macOS Space
    // switch, and WeekStripe's 60s tick detecting the ISO week
    // boundary while the app is focused.
    unlistenFocus = await getCurrentWindow().listen('tauri://focus', () => {
      void checkAndApplyRollover();
    });
    document.addEventListener('visibilitychange', onVisibilityChange);
    window.addEventListener('captainslog:week-changed', onWeekChangedEvent);
    // Safety net for the "app focused on / across a week boundary
    // with no user interaction" case — see the comment where
    // rolloverSafetyInterval is declared for the full rationale.
    // Gated on !document.hidden so we don't burn cycles when the
    // window is minimized / on another Space.
    rolloverSafetyInterval = setInterval(() => {
      if (!document.hidden) void checkAndApplyRollover();
    }, 60_000);
  });

  onDestroy(() => {
    unlistenSettingsChanged?.();
    unlistenTaskIndexRebuilt?.();
    unlistenFocus?.();
    document.removeEventListener('visibilitychange', onVisibilityChange);
    window.removeEventListener('captainslog:week-changed', onWeekChangedEvent);
    if (rolloverSafetyInterval !== null) {
      clearInterval(rolloverSafetyInterval);
      rolloverSafetyInterval = null;
    }
  });

  // Wizard finished the persistence step — refetch settings so the
  // firstRun flag flips to false and the template branches to normal
  // mode. complete_first_run already hot-swapped storage + reminder
  // in-process, so this is purely a UI swap; no app restart.
  async function handleWizardComplete(): Promise<void> {
    try {
      settings = await invoke<Settings>('get_settings');
    } catch (err) {
      loadError = String(err);
    }
  }
</script>

{#if loading}
  <main class="loading">
    <p>Loading…</p>
  </main>
{:else if loadError}
  <main class="loading">
    <div class="card error-card">
      <h2>Couldn't load settings.</h2>
      <p>{loadError}</p>
    </div>
  </main>
{:else if settings?.firstRun}
  <Wizard
    defaultJournalRoot={settings.defaultJournalRoot}
    onComplete={handleWizardComplete}
  />
{:else}
  <!-- ============================== Normal mode ============================== -->
  <main class="normal">
    <section class="welcome">
      <h1>{settings?.userName ? `Welcome back, ${settings.userName}.` : "Captain's Log"}</h1>
      <p class="lead">
        A weekly work journal with tools to help you write self reviews.
      </p>

      <div class="card">
        <h2>What now?</h2>
        <p>
          <strong>Capture a note</strong> any time — click the book icon in
          your menu bar at the top of the screen.
        </p>
        <p>
          <strong>Write your weekly summary</strong> when Friday rolls around,
          or whenever it suits you.
        </p>
        <p>
          <strong>Browse past weeks</strong> to read or edit what you've
          written.
        </p>
      </div>

      <div class="main-actions">
        <button class="btn btn-emerald" onclick={() => goto('/summary')}>
          Write Weekly Summary
        </button>
        <button class="btn btn-marble" onclick={() => goto('/journal')}>
          Browse Journal
        </button>
        <button class="btn btn-marble" onclick={() => goto('/settings')}>Settings</button>
      </div>

      <!--
        Slice 1 task view — read-only list of the current week's
        `- [ ]` / `- [x]` items from the Plans-and-priorities
        subsection. Rendering waits for `tasksLoaded` so the empty
        state doesn't flash before the IPC resolves.
      -->
      <!--
        Slice 4 escape hatch: `hideTaskList` removes the whole section
        for users who don't use the feature. The section is gated
        BEFORE `tasksLoaded` so hiding doesn't briefly flash before
        settings hydrate.
      -->
      {#if tasksLoaded && !taskListPrefs.hideTaskList}
        <section class="task-list" aria-labelledby="task-list-heading">
          <div class="task-list-header">
            <h2 id="task-list-heading">This week's tasks...</h2>
            <button
              type="button"
              class="btn btn-marble btn-sm"
              onclick={openAddTask}
            >
              + Add Task
            </button>
          </div>

          {#if rolloverReceipt}
            <div class="rollover-receipt-wrap">
              <RolloverReceipt
                tasksCopied={rolloverReceipt.tasksCopied}
                sourceLabel={rolloverReceipt.sourceLabel}
                onDismiss={() => (rolloverReceipt = null)}
              />
            </div>
          {/if}

          {#if tasksError}
            <TipBubble heading="Couldn't load tasks">
              {tasksError}
            </TipBubble>
          {:else if tasks.length === 0}
            <TipBubble heading="No tasks yet">
              Click <strong>+ Add Task</strong> above, or create a new
              <strong>task list</strong> in your Weekly Summary
              "Plans and priorities for next week…" section — they'll
              all show up here.
            </TipBubble>
          {:else if visibleTasks.length === 0}
            <!--
              tasks.length > 0 but the filter (Show completed = off) hid
              them all. Nudge the user with the toggle they'd flip to
              unhide, rather than looking like an empty list. Inline
              button navigates straight to Settings so the "how do I
              undo this" answer is one click away.
            -->
            <div class="all-done">
              <TipBubble heading="All done for this week">
                Every task in this week's list is completed. Turn on
                <strong>Show completed tasks</strong> to bring them back.
              </TipBubble>
              <button
                type="button"
                class="btn btn-marble btn-sm all-done-cta"
                onclick={() => goto('/settings')}
              >
                Open Settings
              </button>
            </div>
          {:else}
            {#if toggleError}
              <!--
                `toggleError` comes straight from the backend and is
                already user-facing (see toggle_checkbox_in_plans in
                src-tauri/src/tasks.rs). Don't wrap it with more copy —
                any suffix here produces a double em-dash mess in the
                common "task couldn't be found" path.
              -->
              <div class="toggle-error">
                <TipBubble heading="Couldn't update that task">
                  {toggleError}
                </TipBubble>
              </div>
            {/if}
            <!--
              Fixed-height scroll region so a dense week doesn't push
              the brand footer off-screen. `role=region` + label so
              screen-reader users hear a landmark before the scroll
              begins.
            -->
            <div
              class="task-scroll"
              role="region"
              aria-label="Task list (scrollable)"
            >
              <ul class="task-items">
                {#each visibleTasks as t (taskKey(t))}
                  <li class="task-item" class:completed={t.isCompleted}>
                    <!--
                      Shared visual with the MarkdownEditor checkbox widget
                      (see .checkbox-square in app.css). role=checkbox +
                      aria-checked matches the ARIA-standard toggle pattern
                      and drives the accent-primary fill via the
                      `[aria-checked="true"]` selector.
                    -->
                    <button
                      type="button"
                      class="checkbox-square"
                      role="checkbox"
                      aria-checked={t.isCompleted}
                      aria-label={t.isCompleted
                        ? 'Done. Click to mark not done.'
                        : 'Click to mark done.'}
                      disabled={togglingKeys[taskKey(t)] === true}
                      onclick={() => onToggle(t)}
                    >
                      <svg
                        viewBox="0 0 24 24"
                        width="12"
                        height="12"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="3"
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        aria-hidden="true"
                      >
                        <polyline points="5 12 10 17 19 7" />
                      </svg>
                    </button>
                    <!--
                      Server-sanitized HTML — see render_task_text_inline
                      in src-tauri/src/tasks.rs. Ammonia inline-only
                      allowlist (strong/em/del/code/br) enforces the
                      safety envelope; the frontend just materializes it.
                    -->
                    <!-- eslint-disable-next-line svelte/no-at-html-tags -->
                    <span class="task-text">{@html t.textHtml}</span>
                    {#if t.originalWeek}
                      {@const originLabel = formatOriginLabel(t)}
                      <!--
                        Slice 5 provenance chip. Present only when
                        the task was rolled over from a prior week
                        (originalWeek populated + different from
                        current). Sits next to task text so the paper
                        trail is glanceable without dominating the
                        row.
                      -->
                      <span
                        class="task-origin"
                        title={`Originally created in Week ${t.originalWeek.week}, ${t.originalWeek.year}`}
                      >
                        {originLabel}
                      </span>
                    {/if}
                    {#if taskListPrefs.showCompletedTimestamp && t.isCompleted}
                      {@const label = formatRelativeCompleted(t.completedAt)}
                      {#if label}
                        <span class="task-time" title={t.completedAt ?? ''}>
                          {label}
                        </span>
                      {/if}
                    {/if}
                  </li>
                {/each}
              </ul>
            </div>
          {/if}
        </section>
      {/if}
    </section>

    <footer class="brand-footer" aria-hidden="true">
      <img src="/branded/prodigy-mark.png" class="brand-mark" alt="" />
      <span class="brand-wordmark">Prodigy</span>
    </footer>

    <!--
      Slice 3 "Add task" modal — reuses the canonical Modal shell.
      Backdrop click + Escape dismiss via `onClose`. Enter on the
      input submits via the surrounding <form>'s submit event. The
      backend appends to the current week's Plans section.
    -->
    <Modal
      open={showAddTaskModal}
      onClose={closeAddTask}
      title="Add Task"
      blockDismissal={addingTask}
      focusFirstInput
    >
      <form
        class="add-task-form"
        onsubmit={(e) => {
          e.preventDefault();
          void submitAddTask();
        }}
      >
        <InputField
          id="add-task-text"
          label="Task"
          placeholder="What needs doing?"
          bind:value={addTaskText}
          hint="Markdown formatting like **bold**, *italic*, and ~~strike~~ works."
          warning={addError}
        />
        <!--
          Order: primary (Add Task) first, secondary (Cancel) second.
          Matches the Web/Windows convention rather than macOS-native
          which puts primary on the right.
        -->
        <div class="add-task-actions">
          <button
            type="submit"
            class="btn btn-emerald"
            disabled={addingTask || addTaskText.trim().length === 0}
          >
            {addingTask ? 'Adding…' : 'Add Task'}
          </button>
          <button
            type="button"
            class="btn btn-ruby"
            onclick={closeAddTask}
            disabled={addingTask}
          >
            Cancel
          </button>
        </div>
      </form>
    </Modal>
  </main>
{/if}

<style>
  /* ---- Layouts ---- */
  main {
    display: flex;
    justify-content: center;
    padding: var(--space-12) var(--space-4);
    min-height: 100vh;
  }

  main.loading {
    align-items: center;
  }

  /* Normal mode stacks the welcome content + brand footer vertically so
   * the footer can pin to the bottom of the viewport. */
  main.normal {
    flex-direction: column;
    align-items: center;
  }

  .welcome {
    max-width: 560px;
    width: 100%;
  }

  /* Prodigy brand mark — sits at the bottom of the home screen, centered.
   * Subtle (50% opacity) so it's a quiet presence, not a logo lockup. */
  .brand-footer {
    margin-top: auto;
    padding-top: var(--space-12);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-2);
    opacity: 0.55;
    pointer-events: none;
  }

  .brand-mark {
    height: 48px;
    width: auto;
  }

  .brand-wordmark {
    font-family: var(--font-display);
    font-size: 28px;
    line-height: 1;
    color: var(--accent-primary);
    letter-spacing: 0.01em;
  }

  /* Error card for the load-failure branch. */
  .error-card {
    background: var(--bg-error-tint-soft);
    border-color: var(--border-error);
  }

  strong {
    color: var(--text-primary);
  }

  /* ---- Normal-mode welcome card ---- */
  h1 {
    margin-bottom: var(--space-3);
  }

  .welcome .lead {
    margin-bottom: var(--space-12);
  }

  /* .card / .card h2 / .card p / .card p:last-child were promoted to
     app.css as a shared utility — the rule was duplicated verbatim in
     three route files (home, /settings, /summary). */

  .main-actions {
    margin-top: var(--space-6);
    display: flex;
    justify-content: flex-end;
    gap: var(--space-3);
  }

  /* ---- Slice 1 task list ---- */
  .task-list {
    margin-top: var(--space-8);
  }

  /* Heading + "+ Add task" button on the same row. Space-between so
     the button hugs the right edge without needing a margin auto on
     the button itself. */
  .task-list-header {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--space-3);
    margin-bottom: var(--space-3);
  }

  .task-list-header h2 {
    margin: 0;
  }

  /* ---- Slice 3 add-task modal ---- */
  .add-task-form {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  .add-task-actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
  }

  /* Fixed-height scroll region: caps the list at ~8 rows so a dense
     week doesn't push the brand footer off-screen. Padding-right so
     the scrollbar doesn't overlap task text on WebKit. */
  .task-scroll {
    max-height: 360px;
    overflow-y: auto;
    padding-right: var(--space-1);
  }

  /* "All done" empty-state — TipBubble + Open Settings CTA stacked
     with a small gap so the button reads as the tip's follow-up
     action rather than a floating element. */
  .all-done {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }
  .all-done-cta {
    align-self: flex-start;
  }

  .task-items {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .task-item {
    display: flex;
    /* center, not baseline: the checkbox is an SVG-only <button> with
       no text baseline of its own, so baseline alignment would drop
       it to the bottom of the row. Center keeps it sitting on the
       body-text midline. */
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-3);
    background: var(--bg-surface);
    border: 1px solid var(--border-structural);
    border-radius: var(--radius-md);
  }

  .task-item.completed .task-text {
    color: var(--text-muted);
    text-decoration: line-through;
  }

  /* Checkbox visual comes from the shared `.checkbox-square` class
     in app.css — same widget the MarkdownEditor uses. Nothing to
     add locally. */

  /* Rollover receipt sits between the task-list header and the
     list itself. Small margin below so the header + receipt + list
     don't clump together. */
  .rollover-receipt-wrap {
    margin-bottom: var(--space-3);
  }

  .toggle-error {
    margin-bottom: var(--space-3);
  }

  .task-text {
    flex: 1;
    word-break: break-word;
  }

  /* Small "checked Xh ago" chip on completed rows when the timestamp
     toggle is on. Muted color + smaller size so it complements
     rather than competes with the task text. `title` on the span
     carries the raw ISO timestamp for anyone who wants precision. */
  .task-time {
    flex-shrink: 0;
    font-size: var(--text-caption);
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  /* Slice 5 origin chip — subtle pill that shows a rolled-over
     task's provenance (e.g. "from W26"). Same visual weight as
     .task-time so multiple chips on one row read as a coherent
     metadata group. Accent-primary-text tint separates provenance
     from time (which uses text-muted) so the two are visually
     distinguishable at a glance. */
  .task-origin {
    flex-shrink: 0;
    font-size: var(--text-caption);
    color: var(--accent-primary-text);
    white-space: nowrap;
    /* Border-less pill; the color alone carries the "chip" cue. */
    font-style: italic;
  }
</style>
