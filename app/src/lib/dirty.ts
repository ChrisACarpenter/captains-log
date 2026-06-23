// Cross-window dirty-state reporter.
//
// Each form-bearing route publishes its dirty state to the backend's
// DirtyRegistry via the `set_window_dirty` invoke command. The backend's
// try_quit handler reads that registry at quit time and prompts before
// exiting if any surface has unsaved work.
//
// Use:
//
//   import { reportDirty } from '$lib/dirty';
//   const push = reportDirty('summary', 'the weekly summary');
//   // somewhere reactive:
//   $effect(() => push(isDirty));
//
// `push` is edge-triggered (skips no-op repeats) and 150ms-debounced, so
// it's safe to call inside a $effect that runs on every keystroke. The
// onDestroy hook clears the bit when the component unmounts (e.g. you
// navigate away from /summary), so leaving the route resets cleanly.

import { invoke } from '@tauri-apps/api/core';
import { onDestroy } from 'svelte';

type DirtyKey = 'summary' | 'capture';

export function reportDirty(key: DirtyKey, what: string): (dirty: boolean) => void {
  let lastDirty: boolean | null = null;
  let timer: ReturnType<typeof setTimeout> | null = null;

  function flush(dirty: boolean) {
    invoke('set_window_dirty', { key, entry: { dirty, what } }).catch(() => {
      // Backend not reachable — silently drop. This only happens during
      // shutdown or in tests; the quit guard is best-effort either way.
    });
  }

  function push(dirty: boolean) {
    if (dirty === lastDirty) return;
    lastDirty = dirty;
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => flush(dirty), 150);
  }

  // Clear the bit when the consuming component is destroyed. Without this,
  // navigating away from /summary would leave a stale `dirty: true` in the
  // registry and the next quit attempt would prompt about a surface that
  // isn't even open.
  onDestroy(() => {
    if (timer) clearTimeout(timer);
    flush(false);
  });

  return push;
}
