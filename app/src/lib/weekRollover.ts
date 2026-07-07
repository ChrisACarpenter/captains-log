// Week-rollover detection helper.
//
// /summary and /journal each cache the current ISO (year, week) once on
// mount. The autosave debounce keys writes off that cached value, so after
// a weekend (or any long sleep / system-clock advance) the user types into
// Monday's session and silently writes into LAST week's file. This helper
// re-queries Rust's get_current_year_week and either swaps cleanly or
// surfaces a dirty-edit conflict for the caller to resolve via the existing
// externalUpdate banner pattern.
//
// Caller owns the state. We just ask the question and report.

import { invoke } from '@tauri-apps/api/core';

export type YearWeek = { year: number; week: number };

/**
 * Format an ISO (year, week) as a human-readable range label:
 * `"Week of June 22 – June 28, 2026"`. Cross-year weeks (rare — happens
 * when week 1 or 52/53 straddles a New Year boundary) render as
 * `"Week of December 29, 2025 – January 4, 2026"`.
 *
 * Uses ISO 8601 semantics: week 1 is the week containing the year's
 * first Thursday, so Jan 4 is always in week 1. JS lacks a built-in
 * for ISO-week arithmetic, so we compute Monday-of-week-1 from Jan 4's
 * weekday and offset from there.
 *
 * Consolidated from identical inline implementations that used to live
 * in `/journal` (as `formatWeekRange`) and `/summary` (as a `$derived`
 * `weekLabel` computation).
 */
export function formatWeekRange(yw: YearWeek): string {
  const { year, week } = yw;
  const jan4 = new Date(year, 0, 4);
  const jan4Day = jan4.getDay() || 7; // Sunday → 7 so we can offset uniformly
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

export type RefreshArgs = {
  /** The (year, week) the caller currently has loaded. May be null pre-mount. */
  currentYearWeek: YearWeek | null;
  /** True when the caller has unsaved edits in the current week. */
  isDirty: boolean;
  /** Clean swap — caller adopts the new week (re-fetch, rebaseline). */
  onWeekChange: (next: YearWeek) => void | Promise<void>;
  /** Dirty conflict — caller surfaces the bespoke banner. */
  onDirtyConflict: (info: { oldYearWeek: YearWeek; newYearWeek: YearWeek }) => void | Promise<void>;
};

export async function refreshCurrentWeek(args: RefreshArgs): Promise<void> {
  const { currentYearWeek, isDirty, onWeekChange, onDirtyConflict } = args;
  // No cached week yet — caller is still mounting; nothing to reconcile.
  if (!currentYearWeek) return;
  let now: YearWeek;
  try {
    now = await invoke<YearWeek>('get_current_year_week');
  } catch (err) {
    console.error('[weekRollover] get_current_year_week failed:', err);
    return;
  }
  if (now.year === currentYearWeek.year && now.week === currentYearWeek.week) {
    return;
  }
  if (isDirty) {
    await onDirtyConflict({ oldYearWeek: currentYearWeek, newYearWeek: now });
  } else {
    await onWeekChange(now);
  }
}
