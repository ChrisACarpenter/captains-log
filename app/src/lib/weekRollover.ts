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
