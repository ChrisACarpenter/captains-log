<!--
  Hand-rolled date picker popover. Opens on chip click; emits a commit
  event to `window` so the date-chip CM6 extension can dispatch the
  actual editor transaction (decouples the Svelte UI from the editor
  internals).

  ## Why hand-rolled

  Adding a date-picker dependency (bits-ui, flatpickr, etc.) brings in
  ~30KB-100KB of code, theming overhead, and another package to keep
  pinned. For a fixed scope (month grid + prev/next + today button +
  Mon-first weekday header + keyboard nav) the implementation is ~200
  lines. Worth owning given the brand-aligned styling needs and the
  fact that all interaction is local to a 280-wide popover.

  ## Positioning

  The picker is anchored to the chip via getBoundingClientRect(). On
  mount we compute (chipRect.bottom + 6, chipRect.left) as the preferred
  position. If the resulting popover would overflow the bottom of the
  viewport, we flip above the chip (chipRect.top - popoverHeight - 6).
  Horizontal clamp keeps the popover within the window's right edge.

  ## Keyboard

  - Arrow keys move day-by-day within the month.
  - PageUp / PageDown jump month-by-month.
  - Shift+PageUp / Shift+PageDown jump year-by-year.
  - Home / End jump to start/end of current week.
  - Enter commits the focused day.
  - Escape closes without committing.

  ## Outside-click

  Document `mousedown` listener (NOT click — click fires after blur,
  which can race with focus management). If the mousedown target isn't
  inside the popover, we close.
-->
<script lang="ts">
  import { onMount, onDestroy, tick } from 'svelte';

  let {
    /** ISO YYYY-MM-DD currently shown by the chip. The picker opens
     *  pre-selected on this date. */
    iso,
    /** Document positions of the chip in the source. Passed back via
     *  the commit event so the editor extension knows what range to
     *  replace. */
    from,
    to,
    /** The chip's DOM element. Used for positioning + outside-click
     *  detection. */
    anchorEl,
    /** Fired when the user picks a day. Parent dispatches the editor
     *  transaction in response. */
    onCommit,
    /** Fired when the picker should close (outside-click, Escape,
     *  successful pick). */
    onClose,
    /** Optional: fired when the user hits the "Clear due date" button
     *  in the footer. When absent, the button doesn't render (keeps
     *  the editor date-chip use case unchanged). Used by task rows
     *  where a due date is optional and cleanable. */
    onClear,
  }: {
    iso: string;
    from: number;
    to: number;
    anchorEl: HTMLElement;
    onCommit: (newIso: string) => void;
    onClose: () => void;
    onClear?: () => void;
  } = $props();

  /** Parse the inbound ISO string into local-time year/month/day. */
  function parseIso(s: string): { year: number; month: number; day: number } {
    const [y, m, d] = s.split('-').map((n) => parseInt(n, 10));
    return { year: y, month: m, day: d };
  }

  /** Format year/month/day as ISO YYYY-MM-DD with zero-padding. */
  function formatIso(year: number, month: number, day: number): string {
    return `${String(year).padStart(4, '0')}-${String(month).padStart(2, '0')}-${String(day).padStart(2, '0')}`;
  }

  // svelte-ignore state_referenced_locally
  // The popover is mounted fresh each time the user clicks a chip (the
  // parent uses {#if datePickerOpen} to gate the render), so capturing
  // the initial `iso` at mount-time is exactly the intent — we don't
  // want the picker to reactively follow the chip's source-of-truth
  // while the user is mid-pick.
  const initial = parseIso(iso);
  // The month being shown in the grid (independent of the selected day).
  let viewYear = $state(initial.year);
  let viewMonth = $state(initial.month);
  // The day currently focused / selected.
  let focusedDay = $state(initial.day);

  /** Today's date for the "Today" button + visual highlight in the grid. */
  function today() {
    const now = new Date();
    return {
      year: now.getFullYear(),
      month: now.getMonth() + 1,
      day: now.getDate(),
    };
  }

  /** Days in the displayed month. Handles leap years via the Date trick
   *  (day 0 of next month = last day of current month). */
  const daysInMonth = $derived.by(() => {
    return new Date(viewYear, viewMonth, 0).getDate();
  });

  /** Weekday index (0=Sun, 1=Mon, ..., 6=Sat) of the FIRST day of the
   *  displayed month. Used to offset the grid so dates line up under
   *  weekday columns. We display Mon-first, so re-map Sun (0) to 6. */
  const firstDayOffset = $derived.by(() => {
    const jsDay = new Date(viewYear, viewMonth - 1, 1).getDay();
    return jsDay === 0 ? 6 : jsDay - 1;
  });

  const monthName = $derived.by(() => {
    return new Date(viewYear, viewMonth - 1, 1).toLocaleDateString('en-US', {
      month: 'long',
      year: 'numeric',
    });
  });

  const WEEKDAY_HEADERS = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];

  function prevMonth(): void {
    if (viewMonth === 1) {
      viewMonth = 12;
      viewYear -= 1;
    } else {
      viewMonth -= 1;
    }
    // Clamp focusedDay to new month's last day so the focused cell
    // stays valid (e.g. moving from Jan 31 back to Feb).
    focusedDay = Math.min(focusedDay, daysInMonth);
  }

  function nextMonth(): void {
    if (viewMonth === 12) {
      viewMonth = 1;
      viewYear += 1;
    } else {
      viewMonth += 1;
    }
    focusedDay = Math.min(focusedDay, daysInMonth);
  }

  function selectDay(day: number): void {
    const newIso = formatIso(viewYear, viewMonth, day);
    onCommit(newIso);
    onClose();
  }

  /** Jump the view to today's month AND commit today's date. Users
   *  expect "Today" to be a one-click shortcut, not just a view
   *  navigator that still requires another click on the day cell.
   *  If a caller ever needs pure view-navigation (no commit), we can
   *  split this into two actions — no consumer has asked yet. */
  function goToToday(): void {
    const t = today();
    viewYear = t.year;
    viewMonth = t.month;
    focusedDay = t.day;
    selectDay(t.day);
  }

  // -- Keyboard nav --
  function onKeyDown(e: KeyboardEvent): void {
    if (e.key === 'Escape') {
      e.preventDefault();
      onClose();
      return;
    }
    if (e.key === 'Enter') {
      e.preventDefault();
      selectDay(focusedDay);
      return;
    }
    let handled = true;
    if (e.key === 'ArrowLeft') focusedDay -= 1;
    else if (e.key === 'ArrowRight') focusedDay += 1;
    else if (e.key === 'ArrowUp') focusedDay -= 7;
    else if (e.key === 'ArrowDown') focusedDay += 7;
    else if (e.key === 'PageUp') {
      if (e.shiftKey) viewYear -= 1;
      else prevMonth();
    } else if (e.key === 'PageDown') {
      if (e.shiftKey) viewYear += 1;
      else nextMonth();
    } else {
      handled = false;
    }
    if (!handled) return;
    e.preventDefault();
    // Spill over month boundaries when arrow nav goes past day 1 or
    // past the last day of the month. Lets the user keep arrowing
    // without manually clicking prev/next.
    while (focusedDay < 1) {
      prevMonth();
      focusedDay += daysInMonth;
    }
    while (focusedDay > daysInMonth) {
      focusedDay -= daysInMonth;
      nextMonth();
    }
  }

  // -- Positioning + outside-click --
  let popoverEl = $state<HTMLDivElement | undefined>(undefined);
  let position = $state<{ top: number; left: number } | undefined>(undefined);

  function computePosition(): void {
    if (!popoverEl || !anchorEl) return;
    const chipRect = anchorEl.getBoundingClientRect();
    const popRect = popoverEl.getBoundingClientRect();
    const margin = 6;
    const viewportPad = 8;

    // Prefer placing the popover below the chip. If that would overflow
    // the viewport bottom, flip above. In tight viewports (e.g. the
    // Quick Capture popup window) NEITHER side may fit — the chip is
    // near the top AND the popover is taller than the room below it.
    // The clamp at the end handles that case by sliding the popover
    // back into the viewport, even if it ends up overlapping the chip.
    // Better to overlap than to render off-screen.
    let top = chipRect.bottom + margin;
    if (top + popRect.height > window.innerHeight - viewportPad) {
      top = chipRect.top - popRect.height - margin;
    }
    top = Math.max(
      viewportPad,
      Math.min(top, window.innerHeight - popRect.height - viewportPad)
    );

    let left = chipRect.left;
    if (left + popRect.width > window.innerWidth - viewportPad) {
      left = window.innerWidth - popRect.width - viewportPad;
    }
    if (left < viewportPad) left = viewportPad;

    position = { top, left };
  }

  function onDocumentMouseDown(e: MouseEvent): void {
    if (!popoverEl) return;
    const target = e.target as Node;
    if (popoverEl.contains(target)) return;
    // Click on the chip itself shouldn't re-close us — but the chip's
    // own click handler will dispatch a fresh open event anyway, so
    // closing then reopening is harmless. Simpler to always close.
    onClose();
  }

  onMount(() => {
    void tick().then(() => {
      computePosition();
      // Focus the popover container so keyboard nav works without an
      // initial click.
      popoverEl?.focus();
    });
    window.addEventListener('resize', computePosition);
    window.addEventListener('scroll', computePosition, true);
    document.addEventListener('mousedown', onDocumentMouseDown);
  });

  onDestroy(() => {
    window.removeEventListener('resize', computePosition);
    window.removeEventListener('scroll', computePosition, true);
    document.removeEventListener('mousedown', onDocumentMouseDown);
  });

  // -- Grid construction --
  type GridCell = { day: number; isToday: boolean; isSelected: boolean };
  const grid = $derived.by<(GridCell | null)[]>(() => {
    const cells: (GridCell | null)[] = [];
    const t = today();
    for (let i = 0; i < firstDayOffset; i++) cells.push(null);
    for (let day = 1; day <= daysInMonth; day++) {
      cells.push({
        day,
        isToday:
          viewYear === t.year && viewMonth === t.month && day === t.day,
        isSelected: day === focusedDay,
      });
    }
    // Pad to a multiple of 7 for uniform row count.
    while (cells.length % 7 !== 0) cells.push(null);
    return cells;
  });
</script>

<div
  class="date-picker-popover"
  bind:this={popoverEl}
  style:top="{position?.top ?? -9999}px"
  style:left="{position?.left ?? -9999}px"
  role="dialog"
  aria-label="Pick a date"
  tabindex="-1"
  onkeydown={onKeyDown}
>
  <header class="dp-header">
    <button
      type="button"
      class="dp-nav"
      onclick={prevMonth}
      aria-label="Previous month"
      title="Previous month"
    >‹</button>
    <span class="dp-month-label">{monthName}</span>
    <button
      type="button"
      class="dp-nav"
      onclick={nextMonth}
      aria-label="Next month"
      title="Next month"
    >›</button>
  </header>

  <div class="dp-weekdays">
    {#each WEEKDAY_HEADERS as wd}
      <span class="dp-weekday">{wd}</span>
    {/each}
  </div>

  <div class="dp-grid" role="grid">
    {#each grid as cell, i (i)}
      {#if cell === null}
        <span class="dp-cell dp-cell-empty"></span>
      {:else}
        <button
          type="button"
          class="dp-cell"
          class:is-today={cell.isToday}
          class:is-selected={cell.isSelected}
          onclick={() => selectDay(cell.day)}
          aria-label={formatIso(viewYear, viewMonth, cell.day)}
          aria-pressed={cell.isSelected}
        >
          {cell.day}
        </button>
      {/if}
    {/each}
  </div>

  <footer class="dp-footer">
    <button
      type="button"
      class="dp-action"
      onclick={goToToday}
    >Today</button>
    {#if onClear}
      <button
        type="button"
        class="dp-action dp-action-clear"
        onclick={() => {
          onClear();
          onClose();
        }}
        title="Remove the due date from this task"
      >Clear</button>
    {/if}
    <button
      type="button"
      class="dp-action"
      onclick={onClose}
    >Cancel</button>
  </footer>
</div>

<style>
  .date-picker-popover {
    position: fixed;
    z-index: 200;
    width: 280px;
    background: var(--bg-elevated);
    border: 1px solid var(--border-structural);
    border-radius: var(--radius-md);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.18);
    padding: var(--space-3);
    font-family: var(--font-body);
    color: var(--text-primary);
    outline: none;
  }

  .dp-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: var(--space-2);
  }

  .dp-month-label {
    font-weight: 600;
    font-size: 14px;
  }

  .dp-nav {
    width: 28px;
    height: 28px;
    padding: 0;
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    font-size: 18px;
    line-height: 1;
    cursor: pointer;
    transition: background var(--transition-fast), color var(--transition-fast);
  }
  .dp-nav:hover {
    background: var(--bg-surface);
    color: var(--text-primary);
  }

  .dp-weekdays {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    gap: 2px;
    margin-bottom: 4px;
  }
  .dp-weekday {
    text-align: center;
    font-size: 11px;
    font-weight: 600;
    /* text-muted on bg-elevated at 11px = 3.83:1 (fails AA). text-secondary
       clears 5.41:1; the 600 weight + uppercase still reads as quieter
       than the day numbers. */
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .dp-grid {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    gap: 2px;
  }

  .dp-cell {
    aspect-ratio: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--text-primary);
    font: inherit;
    font-size: var(--text-caption);
    cursor: pointer;
    transition: background var(--transition-fast), color var(--transition-fast);
  }
  .dp-cell:hover {
    background: var(--bg-surface);
  }
  .dp-cell.is-today {
    /* Subtle ring on today, even when not selected. Use lifted-orange
       variant for the text — raw accent-primary at 13px on bg-elevated
       fails AA (3.77:1). The ring's color stays accent-primary; rings
       are UI components (3:1 threshold) where raw accent-primary passes. */
    box-shadow: inset 0 0 0 1px var(--accent-primary);
    color: var(--accent-primary-text);
  }
  .dp-cell.is-selected {
    background: var(--accent-primary);
    color: var(--bg-base);
    font-weight: 600;
  }
  .dp-cell.is-selected.is-today {
    /* When today is also the selected day, drop the ring; the fill is enough. */
    box-shadow: none;
  }
  .dp-cell-empty {
    /* Pad slot for off-month days at the grid start. */
    visibility: hidden;
  }

  .dp-footer {
    display: flex;
    justify-content: space-between;
    gap: var(--space-2);
    margin-top: var(--space-2);
    padding-top: var(--space-2);
    border-top: 1px solid var(--border-structural);
  }
  .dp-action {
    padding: 4px 12px;
    background: transparent;
    border: 1px solid var(--border-structural);
    border-radius: var(--radius-sm);
    color: var(--text-primary);
    font: inherit;
    font-size: var(--text-caption);
    cursor: pointer;
    transition: background var(--transition-fast);
  }
  .dp-action:hover {
    background: var(--bg-surface);
  }
  /* Clear action reads as destructive — same maroon tint as the
     trash-row hover state in +page.svelte, but never at rest so it
     stays a peer of Today / Cancel until the user actually goes for
     it. Only rendered when the caller passed `onClear`. */
  .dp-action-clear:hover,
  .dp-action-clear:focus-visible {
    color: var(--brand-maroon);
    background: color-mix(in srgb, var(--brand-maroon) 12%, transparent);
    border-color: color-mix(in srgb, var(--brand-maroon) 40%, transparent);
    outline: none;
  }
</style>
