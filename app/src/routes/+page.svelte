<script lang="ts">
  import { onMount, onDestroy, tick } from 'svelte';
  import { goto } from '$app/navigation';
  import { invoke } from '@tauri-apps/api/core';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { getCurrentWindow } from '@tauri-apps/api/window';

  import Wizard from '$lib/onboarding/Wizard.svelte';
  import TipBubble from '$lib/onboarding/TipBubble.svelte';
  import Modal from '$lib/Modal.svelte';
  import InputField from '$lib/InputField.svelte';
  import RolloverReceipt from '$lib/RolloverReceipt.svelte';
  import DatePickerPopover from '$lib/DatePickerPopover.svelte';
  import ConfirmDialog from '$lib/ConfirmDialog.svelte';
  import TaskMetaChip from '$lib/TaskMetaChip.svelte';
  import TaskRowActionButton from '$lib/TaskRowActionButton.svelte';
  import PrepSelfReviewWizard from '$lib/review-prep/PrepSelfReviewWizard.svelte';
  import { urlPasteUpgrade } from '$lib/url-paste-upgrade';

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
    autoImportCompleted: boolean;
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
    autoImportCompleted: true,
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
    /**
     * Phase 3e — optional due date (YYYY-MM-DD, local). `null` when
     * the task has no date set. The row renders a "Due …" chip when
     * present; overdue tasks (dueDate < today) surface at the top of
     * the Incomplete section under an "Overdue" heading.
     */
    dueDate: string | null;
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

  // Slice 6c delete confirmation state. Modal opens when the trash
  // icon is clicked on a row. Task text is echoed back in the modal
  // body so the user has a chance to reconsider before the row
  // (and its provenance + timestamp) is gone.
  let deleteConfirmTask = $state<TaskListEntry | null>(null);
  let deletingTask = $state(false);
  let deleteError = $state('');

  // Phase 5 — Prep Self Review wizard. Modal-hosted, opened from the
  // .main-actions button below. The wizard fetches its own settings
  // and manages its own lifecycle; landing only owns the visibility
  // flag.
  let prepReviewOpen = $state(false);

  // Phase 3e — due-date picker state. Exactly one picker can be open
  // at a time, keyed by the target task. Storing the anchor element
  // (the calendar button) lets DatePickerPopover position itself
  // relative to that button, just like the editor's date-chip picker.
  let dueDatePickerTask = $state<TaskListEntry | null>(null);
  let dueDatePickerAnchor = $state<HTMLElement | null>(null);
  let dueDatePickerBusy = $state(false);

  // Slice 6b inline-edit state. Exactly one row can be in edit mode at
  // a time (identified by `editingKey`); its input field is bound to
  // `editText`. `editError` surfaces backend validation failures
  // inline under the row so the user sees the reason next to the
  // input they just tried to submit. `savingEdit` disables Enter
  // during the IPC round-trip so a fast second Return doesn't
  // double-submit. `editInputEl` is the DOM handle used by the
  // $effect below to focus + select on open.
  let editingKey = $state<string | null>(null);
  let editText = $state('');
  let editError = $state('');
  let savingEdit = $state(false);
  let editInputEl = $state<HTMLInputElement | null>(null);

  // Polish Sweep #4 — focus-restoration handles. `pencilRefs` maps
  // each task's key to the DOM node of its pencil (edit) action
  // button, so on edit-exit and delete-success we can bring focus
  // back to a sensible landing target instead of dropping to
  // document.body. `addTaskButtonEl` is the safe fallback for
  // delete-when-list-is-empty.
  let pencilRefs: Record<string, HTMLButtonElement | null> = $state({});
  let addTaskButtonEl = $state<HTMLButtonElement | null>(null);

  // Autofocus + select-all on edit open. Tracks `editingKey` so it
  // re-fires when the user switches between rows (pencil → other
  // pencil). Guarded by an element check because the input only
  // exists in the DOM while editingKey is non-null.
  $effect(() => {
    if (editingKey !== null && editInputEl) {
      editInputEl.focus();
      editInputEl.select();
    }
  });

  // Focus a target after Svelte's next DOM flush. Used by the
  // edit-exit + delete-exit paths — both change state that unmounts
  // an input or a row, and the target only exists AFTER the flush.
  async function focusAfterFlush(getTarget: () => HTMLElement | null | undefined): Promise<void> {
    await tick();
    const target = getTarget();
    target?.focus();
  }

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

  // Slice 6a: the landing page renders two grouped sub-lists
  // (Incomplete / Completed) that match the file's `### Tasks`
  // anchor partition. Both groups filter down from `visibleTasks`
  // so they inherit the `showCompleted` toggle for free (turning it
  // off collapses `completedVisibleTasks` to empty).
  //
  // Phase 3e — the Incomplete group further splits into "Overdue"
  // (any task with dueDate strictly earlier than today's LOCAL
  // date) and the regular Incomplete list. "Due today" is NOT
  // overdue per the locked design (one grace day). Overdue tasks
  // sort by earliest date first; regular Incomplete stays in file
  // order. Completed tasks never appear in Overdue regardless of
  // date — the debt was paid.
  const incompleteVisibleTasks = $derived(
    visibleTasks.filter((t) => !t.isCompleted),
  );
  const completedVisibleTasks = $derived(
    visibleTasks.filter((t) => t.isCompleted),
  );
  const overdueVisibleTasks = $derived.by(() => {
    const today = todayIso();
    return incompleteVisibleTasks
      .filter((t) => t.dueDate !== null && t.dueDate < today)
      .slice() // .sort mutates — copy first so the derived source stays intact
      .sort((a, b) => dateStringCompare(a.dueDate ?? '', b.dueDate ?? ''));
  });
  const incompleteNonOverdueTasks = $derived.by(() => {
    const today = todayIso();
    return incompleteVisibleTasks.filter(
      (t) => t.dueDate === null || t.dueDate >= today,
    );
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

  // ---- Slice 6b: inline edit ----

  type TaskEditResult = {
    textHash: string;
    ordinal: number;
    isCompleted: boolean;
  };

  function openEdit(t: TaskListEntry): void {
    // Guard against re-open of the same row (would clobber unsaved
    // input text) but ALLOW switching from one row's edit to another:
    // opening a second pencil implicitly cancels the first.
    const key = taskKey(t);
    if (editingKey === key) return;
    editingKey = key;
    editText = t.text;
    editError = '';
  }

  function cancelEdit(): void {
    if (savingEdit) return;
    // Polish Sweep #4 — capture the key BEFORE clearing so focus
    // can return to that row's pencil after the input unmounts.
    const wasEditingKey = editingKey;
    editingKey = null;
    editText = '';
    editError = '';
    if (wasEditingKey) {
      void focusAfterFlush(() => pencilRefs[wasEditingKey]);
    }
  }

  async function submitEdit(t: TaskListEntry): Promise<void> {
    if (savingEdit) return;
    const trimmed = editText.trim();
    if (trimmed.length === 0) {
      editError = "Task text can't be empty.";
      return;
    }
    // Same-text edit is a no-op — dismiss without a round-trip so the
    // user doesn't get a needless spinner + last_updated stamp for a
    // change they didn't actually make.
    if (trimmed === t.text.trim()) {
      cancelEdit();
      return;
    }
    savingEdit = true;
    editError = '';
    try {
      await withTimeout(
        invoke<TaskEditResult>('edit_task', {
          year: t.year,
          week: t.week,
          textHash: t.textHash,
          ordinal: t.ordinal,
          text: trimmed,
        }),
        ADD_TASK_TIMEOUT_MS,
        'Edit task',
      );
      tasks = await withTimeout(
        invoke<TaskListEntry[]>('list_tasks'),
        ADD_TASK_TIMEOUT_MS,
        'Refresh task list',
      );
      // Polish Sweep #4 — text edit may have changed the task's key
      // (text_hash + ordinal shift). Look up the pencil ref via the
      // NEW key so focus lands on the row the user just finished
      // editing. Fall back to the +Add Task button if the row
      // somehow isn't in the refreshed list.
      const editedText = trimmed;
      editingKey = null;
      editText = '';
      const nextRow = tasks.find(
        (r) => r.year === t.year && r.week === t.week && r.text.trim() === editedText,
      );
      const nextKey = nextRow ? taskKey(nextRow) : null;
      void focusAfterFlush(() => (nextKey && pencilRefs[nextKey]) || addTaskButtonEl);
    } catch (err) {
      editError = String(err);
    } finally {
      savingEdit = false;
    }
  }

  function onEditKeydown(e: KeyboardEvent, t: TaskListEntry): void {
    if (e.key === 'Enter' && !e.isComposing) {
      e.preventDefault();
      void submitEdit(t);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      cancelEdit();
    }
  }

  // ---- Slice 6c: delete confirmation ----

  function openDeleteConfirm(t: TaskListEntry): void {
    if (deleteConfirmTask) return; // one confirm at a time
    deleteConfirmTask = t;
    deleteError = '';
  }

  function closeDeleteConfirm(): void {
    if (deletingTask) return; // Modal also honors blockDismissal
    deleteConfirmTask = null;
    deleteError = '';
  }

  async function submitDelete(): Promise<void> {
    const t = deleteConfirmTask;
    if (!t || deletingTask) return;
    deletingTask = true;
    deleteError = '';
    // Polish Sweep #4 — capture the deleted row's index in the
    // pre-refresh flat task list so we can focus a sensible
    // neighbour after the row disappears. Same-index-after-delete
    // gives us the "next row" the user expects; if the deleted row
    // was last, we fall through to the previous row; if the list is
    // now empty, we fall through to +Add Task.
    const deletedKey = taskKey(t);
    const priorIndex = tasks.findIndex((r) => taskKey(r) === deletedKey);
    try {
      await withTimeout(
        invoke('delete_task', {
          year: t.year,
          week: t.week,
          textHash: t.textHash,
          ordinal: t.ordinal,
        }),
        ADD_TASK_TIMEOUT_MS,
        'Delete task',
      );
      tasks = await withTimeout(
        invoke<TaskListEntry[]>('list_tasks'),
        ADD_TASK_TIMEOUT_MS,
        'Refresh task list',
      );
      // Free the stale pencil ref for the deleted row.
      delete pencilRefs[deletedKey];
      deleteConfirmTask = null;
      void focusAfterFlush(() => {
        if (tasks.length === 0) return addTaskButtonEl;
        // Prefer the row now at the deleted row's OLD index (i.e.
        // the "next row down"). If the deleted row was last, back up.
        const targetIndex = Math.min(priorIndex, tasks.length - 1);
        const targetKey = targetIndex >= 0 ? taskKey(tasks[targetIndex]) : null;
        return (targetKey && pencilRefs[targetKey]) || addTaskButtonEl;
      });
    } catch (err) {
      deleteError = String(err);
    } finally {
      deletingTask = false;
    }
  }

  // ---- Phase 3e: due-date picker ----

  /** Local today as YYYY-MM-DD — used as the picker's seed date when
   *  a task has no due date yet, and as the reference for the
   *  "overdue" test in the derived-groups computation below. */
  function todayIso(): string {
    const d = new Date();
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
  }

  function openDueDatePicker(t: TaskListEntry, anchor: HTMLElement): void {
    if (dueDatePickerTask || dueDatePickerBusy) return;
    dueDatePickerTask = t;
    dueDatePickerAnchor = anchor;
  }

  function closeDueDatePicker(): void {
    if (dueDatePickerBusy) return;
    dueDatePickerTask = null;
    dueDatePickerAnchor = null;
  }

  async function commitDueDate(iso: string): Promise<void> {
    const t = dueDatePickerTask;
    if (!t || dueDatePickerBusy) return;
    dueDatePickerBusy = true;
    try {
      await invoke('set_task_due_date', {
        year: t.year,
        week: t.week,
        textHash: t.textHash,
        ordinal: t.ordinal,
        dueDate: iso,
      });
      tasks = await invoke<TaskListEntry[]>('list_tasks');
    } catch (err) {
      // eslint-disable-next-line no-console
      console.error('[due-date] set failed:', err);
      // TODO: surface a user-facing error like editError / deleteError
      // does. For now the picker closes and the task list refetches
      // don't reflect the failed change; user can retry.
    } finally {
      dueDatePickerBusy = false;
      dueDatePickerTask = null;
      dueDatePickerAnchor = null;
    }
  }

  async function clearDueDate(): Promise<void> {
    const t = dueDatePickerTask;
    if (!t || dueDatePickerBusy) return;
    dueDatePickerBusy = true;
    try {
      await invoke('set_task_due_date', {
        year: t.year,
        week: t.week,
        textHash: t.textHash,
        ordinal: t.ordinal,
        dueDate: null,
      });
      tasks = await invoke<TaskListEntry[]>('list_tasks');
    } catch (err) {
      // eslint-disable-next-line no-console
      console.error('[due-date] clear failed:', err);
    } finally {
      dueDatePickerBusy = false;
    }
  }

  /** Compare two YYYY-MM-DD strings as calendar dates. `a < b` iff
   *  the calendar day represented by `a` is strictly earlier. String
   *  comparison works because the format is zero-padded left-to-right
   *  numeric. */
  function dateStringCompare(a: string, b: string): number {
    if (a < b) return -1;
    if (a > b) return 1;
    return 0;
  }

  /** Format a YYYY-MM-DD as a compact chip label:
   *  - "Due today" when the date == today's local date
   *  - "Due <Weekday>" when within the next 6 days
   *  - "Due Jul 15" when this year
   *  - "Due Jul 15, 2027" otherwise
   */
  function formatDueDateLabel(iso: string): string {
    const [y, m, d] = iso.split('-').map((n) => parseInt(n, 10));
    if (!y || !m || !d) return iso;
    const date = new Date(y, m - 1, d);
    const now = new Date();
    const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
    const days = Math.round((date.getTime() - today.getTime()) / 86_400_000);
    if (days === 0) return 'Due today';
    // Within a week either direction → weekday name (helps triage
    // near-term commitments at a glance).
    if (days > 0 && days < 7) {
      return `Due ${date.toLocaleDateString('en-US', { weekday: 'short' })}`;
    }
    if (y === now.getFullYear()) {
      return `Due ${date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })}`;
    }
    return `Due ${date.toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' })}`;
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

  // Slice 6c-followup: once-per-local-day auto-import of completed
  // tasks into Key accomplishments. All gating (setting off + already-
  // ran-today) is enforced by the backend; the frontend just fires
  // the same trigger set as the rollover check. Idempotent, best-
  // effort by construction — but Polish Sweep #3 surfaces failures
  // in-app instead of hiding them in the JS console. Disk full,
  // permission denied, a corrupt sidecar — the user deserves to know
  // when yesterday's completions didn't roll into Key accomplishments.
  let autoImportInFlight = false;
  let autoImportError = $state<string | null>(null);
  async function checkAndApplyAutoImport(): Promise<void> {
    if (autoImportInFlight) return;
    if (loading || settings === null || settings.firstRun) return;
    // Setting-gate here for a quick short-circuit + to skip the IPC
    // when we already know the answer. The backend enforces the
    // same rule authoritatively.
    if (!taskListPrefs.autoImportCompleted) return;
    autoImportInFlight = true;
    try {
      await invoke('check_and_apply_auto_task_import');
      // Success (or "nothing to do" — the backend swallows the
      // no-op case as a no-op, not an error) clears any lingering
      // error from a previous attempt.
      autoImportError = null;
    } catch (err) {
      // eslint-disable-next-line no-console
      console.error('[auto-import] check failed:', err);
      autoImportError = String(err);
    } finally {
      autoImportInFlight = false;
    }
  }

  function onVisibilityChange(): void {
    if (!document.hidden) {
      void checkAndApplyRollover();
      void checkAndApplyAutoImport();
    }
  }

  function onWeekChangedEvent(): void {
    void checkAndApplyRollover();
    void checkAndApplyAutoImport();
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
    // Slice 6c-followup — auto-import completed tasks. Backend gates
    // on setting + last-import-date, so this is safe to fire alongside
    // the rollover check on every trigger event.
    void checkAndApplyAutoImport();

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
      void checkAndApplyAutoImport();
    });
    document.addEventListener('visibilitychange', onVisibilityChange);
    window.addEventListener('captainslog:week-changed', onWeekChangedEvent);
    // Safety net for the "app focused on / across a week boundary
    // with no user interaction" case — see the comment where
    // rolloverSafetyInterval is declared for the full rationale.
    // Gated on !document.hidden so we don't burn cycles when the
    // window is minimized / on another Space.
    rolloverSafetyInterval = setInterval(() => {
      if (!document.hidden) {
        void checkAndApplyRollover();
        void checkAndApplyAutoImport();
      }
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

      <!-- Phase 5 — Prep Self Review lives in its own centered row
           below the standard action set. Sapphire so it visually
           announces itself as the flagship "big magic" action; a
           marble button in the existing row would have felt lost. -->
      <div class="secondary-actions">
        <button class="btn btn-sapphire" onclick={() => (prepReviewOpen = true)}>
          Prep Self Review
        </button>
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
            <h2 id="task-list-heading">My task list...</h2>
            <button
              type="button"
              class="btn btn-marble btn-sm"
              onclick={openAddTask}
              bind:this={addTaskButtonEl}
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

          <!-- Polish Sweep #3 — auto-import failure surface. Sits in
               the same visual slot as the rollover receipt so the
               user's attention naturally lands on either. Error
               persists until manually dismissed (matches #2). -->
          {#if autoImportError}
            <div class="auto-import-error-wrap">
              <div class="auto-import-error" role="alert" aria-live="assertive">
                <span class="text">
                  <strong>Couldn't auto-import completed tasks:</strong>
                  {autoImportError}
                </span>
                <button
                  type="button"
                  class="dismiss"
                  onclick={() => (autoImportError = null)}
                  aria-label="Dismiss auto-import error"
                >×</button>
              </div>
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

              Slice 6a: tasks are visually grouped into "Incomplete
              Tasks" and "Completed Tasks" sections, matching the
              file's `### Tasks` anchor partition. The `<li>` template
              is factored into a snippet so both sections share the
              same rendering, provenance chip, timestamp chip, and
              a11y wiring.
            -->
            <div
              class="task-scroll"
              role="region"
              aria-label="Task list (scrollable)"
            >
              {#snippet taskRow(t: TaskListEntry)}
                {@const key = taskKey(t)}
                {@const isEditing = editingKey === key}
                <li class="task-item" class:completed={t.isCompleted} class:editing={isEditing}>
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
                    disabled={togglingKeys[key] === true || isEditing}
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
                  {#if isEditing}
                    <!--
                      Slice 6b inline edit. `use:autofocus` selects the
                      full text on open so the user can immediately
                      type-replace or tab through characters. Enter
                      submits, Escape cancels, blur cancels — mirrors
                      the AskUserQuestion patterns in the rest of the
                      app so keyboard-only users get consistent
                      behavior.
                    -->
                    <input
                      type="text"
                      class="task-edit-input"
                      bind:this={editInputEl}
                      bind:value={editText}
                      onkeydown={(e) => onEditKeydown(e, t)}
                      onblur={cancelEdit}
                      disabled={savingEdit}
                      aria-label="Task text"
                      use:urlPasteUpgrade
                    />
                  {:else}
                    <!--
                      Server-sanitized HTML — see render_task_text_inline
                      in src-tauri/src/tasks.rs. Ammonia inline-only
                      allowlist (strong/em/del/code/br) enforces the
                      safety envelope; the frontend just materializes it.
                    -->
                    <!-- eslint-disable-next-line svelte/no-at-html-tags -->
                    <span class="task-text">{@html t.textHtml}</span>
                    {#if t.originalWeek}
                      <TaskMetaChip
                        variant="origin"
                        label={formatOriginLabel(t)}
                        title={`Originally created in Week ${t.originalWeek.week}, ${t.originalWeek.year}`}
                      />
                    {/if}
                    {#if taskListPrefs.showCompletedTimestamp && t.isCompleted}
                      {@const label = formatRelativeCompleted(t.completedAt)}
                      {#if label}
                        <TaskMetaChip
                          variant="time"
                          {label}
                          title={t.completedAt ?? ''}
                        />
                      {/if}
                    {/if}
                    {#if t.dueDate && !t.isCompleted}
                      <!--
                        Phase 3e due-date chip. Rendered only on
                        INCOMPLETE rows — a completed task's due date
                        is history, and the chip becomes needless
                        clutter once the task is done. Sidecar entry
                        survives check/uncheck (toggle re-key
                        preserves it), so unchecking restores the
                        chip. The overdue variant is reinforced by
                        the row landing under the "Overdue" section
                        header on the parent list.
                      -->
                      <TaskMetaChip
                        variant={t.dueDate < todayIso() ? 'due-overdue' : 'due'}
                        label={formatDueDateLabel(t.dueDate)}
                        title={`Due ${t.dueDate}`}
                      />
                    {/if}
                    <!--
                      Slice 6b/6c/3e row actions. Order (text → right):
                      pencil (edit), calendar (due date), trash
                      (delete). Destructive action stays farthest from
                      primary tap targets. All three disabled while a
                      different row is being edited or a delete
                      confirmation is open so keyboard focus can't
                      wander.
                    -->
                    <TaskRowActionButton
                      icon="pencil"
                      variant="neutral"
                      ariaLabel={`Edit task: ${t.text}`}
                      title="Edit task"
                      disabled={togglingKeys[key] === true || editingKey !== null}
                      onclick={() => openEdit(t)}
                      bind:el={pencilRefs[key]}
                    />
                    <TaskRowActionButton
                      icon="calendar"
                      variant="accent"
                      ariaLabel={t.dueDate ? `Change due date for: ${t.text}` : `Set a due date for: ${t.text}`}
                      title={t.dueDate ? 'Change due date' : 'Set a due date'}
                      disabled={togglingKeys[key] === true || editingKey !== null || deleteConfirmTask !== null || dueDatePickerTask !== null}
                      onclick={(e) => openDueDatePicker(t, e.currentTarget as HTMLElement)}
                    />
                    <TaskRowActionButton
                      icon="trash"
                      variant="destructive"
                      ariaLabel={`Delete task: ${t.text}`}
                      title="Delete task"
                      disabled={togglingKeys[key] === true || editingKey !== null || deleteConfirmTask !== null}
                      onclick={() => openDeleteConfirm(t)}
                    />
                  {/if}
                </li>
                {#if isEditing && editError}
                  <li class="task-edit-error" role="alert">{editError}</li>
                {/if}
              {/snippet}

              {#if overdueVisibleTasks.length > 0}
                <!--
                  Phase 3e — Overdue group at the top of the
                  Incomplete section. Only rendered when there's at
                  least one overdue task; empty group never surfaces.
                  Sorted by earliest due date first — the oldest
                  debt sits on top.
                -->
                <h3 class="task-group-header task-group-header-overdue">Overdue</h3>
                <ul class="task-items">
                  {#each overdueVisibleTasks as t (taskKey(t))}
                    {@render taskRow(t)}
                  {/each}
                </ul>
              {/if}
              {#if incompleteNonOverdueTasks.length > 0}
                <h3 class="task-group-header">Incomplete Tasks</h3>
                <ul class="task-items">
                  {#each incompleteNonOverdueTasks as t (taskKey(t))}
                    {@render taskRow(t)}
                  {/each}
                </ul>
              {/if}
              {#if completedVisibleTasks.length > 0}
                <h3 class="task-group-header">Completed Tasks</h3>
                <ul class="task-items">
                  {#each completedVisibleTasks as t (taskKey(t))}
                    {@render taskRow(t)}
                  {/each}
                </ul>
              {/if}
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
          hint="Markdown formatting like **bold**, *italic*, and ~~strike~~ works. Paste a URL to auto-link it."
          warning={addError}
          urlPaste
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

    {#if deleteConfirmTask}
      <!--
        Slice 6c delete confirmation. Standard ConfirmDialog shell —
        matches how LabelDetailsModal handles rename / delete confirms
        elsewhere in the app. Body snippet renders the task text in a
        blockquote so the user sees exactly what they're about to
        delete; error TipBubble stacks above the buttons on failure.
      -->
      <ConfirmDialog
        title="Delete Task"
        confirmLabel={deletingTask ? 'Deleting…' : 'Delete'}
        confirmVariant="ruby"
        cancelLabel="Cancel"
        cancelVariant="marble"
        onConfirm={() => void submitDelete()}
        onCancel={closeDeleteConfirm}
      >
        {#snippet body()}
          <p>
            Delete this task? Its completion timestamp and rollover
            history will be dropped with it.
          </p>
          <blockquote class="delete-confirm-quote">
            <!-- Non-null assertion: the snippet only renders inside the
                 outer {#if deleteConfirmTask}, so this is defined here.
                 Svelte's snippet closures don't carry the outer TS
                 narrowing through, hence the explicit `!`. -->
            {deleteConfirmTask!.text}
          </blockquote>
          {#if deleteError}
            <TipBubble heading="Couldn't delete that task">
              {deleteError}
            </TipBubble>
          {/if}
        {/snippet}
      </ConfirmDialog>
    {/if}

    {#if dueDatePickerTask && dueDatePickerAnchor}
      <!--
        Phase 3e due-date picker. Reuses the same DatePickerPopover
        the editor's inline date chips use. `from`/`to` are irrelevant
        for this call site (they exist for the CodeMirror-transaction
        commit path); we pass 0/0 as placeholders. Seed the picker
        with the task's current date if any, else today. `onClear` is
        wired only for rows that already have a date — passing it
        conditionally makes the picker's Clear button appear only
        when there's something to clear.
      -->
      <DatePickerPopover
        iso={dueDatePickerTask.dueDate ?? todayIso()}
        from={0}
        to={0}
        anchorEl={dueDatePickerAnchor}
        onCommit={(iso) => void commitDueDate(iso)}
        onClose={closeDueDatePicker}
        onClear={dueDatePickerTask.dueDate ? () => void clearDueDate() : undefined}
      />
    {/if}

    <!-- Phase 5 — Prep Self Review wizard. Modal-hosted so it can
         overlay the landing without stealing the whole route the way
         onboarding does. Wizard owns its own state + settings refresh
         cycle; landing just toggles visibility. -->
    <PrepSelfReviewWizard
      open={prepReviewOpen}
      onClose={() => (prepReviewOpen = false)}
    />
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

  /* Prep Self Review — its own centered row, sapphire, sits below the
     three-button navigation cluster above. */
  .secondary-actions {
    margin-top: var(--space-4);
    display: flex;
    justify-content: center;
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

  /* Slice 6a: group headers between the two anchor-partitioned
     sub-lists. Small caps + a hairline underline give them the same
     "section label" quality as the rest of the summary UI without
     competing with the section H2 for hierarchy. */
  .task-group-header {
    font-size: 0.75rem;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-muted);
    margin: var(--space-4) 0 var(--space-2);
    padding-bottom: var(--space-1);
    border-bottom: 1px solid var(--border-decorative);
  }
  /* First header (Incomplete when both groups exist, or the sole
     group when only one exists) shouldn't push the list off the top
     of the scroll region. */
  .task-group-header:first-child {
    margin-top: 0;
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

  /* Polish Sweep #3 — auto-import failure banner. Same slot as the
     rollover receipt so the visual language is "status about the
     task list, appearing here." Persistent (no auto-clear); the ×
     is the only way to dismiss. Red bordering + role="alert" mean
     both sighted users and screen readers hear the failure. */
  .auto-import-error-wrap {
    margin-bottom: var(--space-3);
  }
  .auto-import-error {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-2) var(--space-3);
    background: color-mix(in srgb, var(--accent-danger, #b32d2d) 8%, var(--bg-elevated));
    border: 1px solid var(--accent-danger, #b32d2d);
    border-radius: var(--radius-md);
    color: var(--text-primary);
    line-height: 1.4;
  }
  .auto-import-error .text {
    flex: 1;
  }
  .auto-import-error :global(strong) {
    color: var(--accent-danger, #b32d2d);
    font-weight: 600;
  }
  .auto-import-error .dismiss {
    appearance: none;
    background: none;
    border: none;
    padding: 0 var(--space-2);
    margin: 0;
    cursor: pointer;
    font-size: 1.2em;
    line-height: 1;
    color: var(--text-secondary);
    border-radius: var(--radius-sm, 4px);
  }
  .auto-import-error .dismiss:hover {
    color: var(--text-primary);
  }
  .auto-import-error .dismiss:focus-visible {
    outline: 2px solid var(--focus-glow);
    outline-offset: 2px;
  }

  .toggle-error {
    margin-bottom: var(--space-3);
  }

  .task-text {
    flex: 1;
    word-break: break-word;
  }

  /* Inline edit input. Matches .task-text's flex:1 so the input
     occupies the same slot as the text span it replaces. Border +
     background make it clearly "in edit mode" without needing a
     full theme swap; the .editing class on .task-item adds the
     complementary outer treatment. */
  .task-edit-input {
    flex: 1;
    min-width: 0;
    padding: 4px var(--space-2);
    font: inherit;
    color: var(--text-primary);
    background: var(--bg-surface);
    border: 1px solid var(--accent-primary);
    border-radius: var(--radius-sm);
  }
  .task-edit-input:focus {
    outline: 2px solid var(--accent-primary);
    outline-offset: 1px;
  }

  .task-item.editing {
    /* Subtle border bump so the whole row reads as "the one being
       edited" — useful when tasks are dense and the input alone
       could get lost visually. */
    border-color: var(--accent-primary);
  }

  /* Error strip immediately under the edit input. Not a full
     TipBubble — the input is right there, so a lightweight inline
     message under the row keeps the error tightly coupled to the
     control it belongs to. */
  .task-edit-error {
    list-style: none;
    padding: 0 var(--space-3) 0 40px;
    font-size: var(--text-caption);
    color: var(--accent-danger, #b32d2d);
  }

  /* Phase 3e — Overdue section header. Same visual as the other
     group headers (small caps + hairline underline) but the label
     itself + underline tint maroon so the section reads as
     "attention required" before the user even scans the rows. */
  .task-group-header-overdue {
    color: var(--brand-maroon-text);
    border-bottom-color: color-mix(in srgb, var(--brand-maroon-text) 45%, transparent);
  }

  /* Quoted task text inside the delete-confirm ConfirmDialog body.
     A subtle inset left border echoes the "this is what you're
     about to delete" framing without stealing focus from the
     action buttons below. `--bg-elevated` (rather than a low-alpha
     color-mix on brand-maroon) keeps the panel legible in dark
     theme; the maroon accent stripe carries the destructive cue. */
  .delete-confirm-quote {
    margin: 0 0 var(--space-3);
    padding: var(--space-2) var(--space-3);
    border-left: 3px solid var(--brand-maroon);
    background: var(--bg-elevated);
    border-radius: 0 var(--radius-sm) var(--radius-sm) 0;
    font-style: italic;
    word-break: break-word;
  }
</style>
