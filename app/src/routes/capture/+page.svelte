<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import LabelInput from '$lib/LabelInput.svelte';
  import { reportDirty } from '$lib/dirty';

  type Status = 'idle' | 'saving' | 'saved' | 'error';

  let title = $state('');
  let body = $state('');
  let labels = $state<string[]>([]);
  let status = $state<Status>('idle');
  let errorMessage = $state('');

  // Cross-window dirty tracking: capture state persists across hide (the
  // window isn't destroyed by Esc or the red-X — see lib.rs capture
  // close handler), so a "hidden but dirty" popup must still count as
  // unsaved work for the quit confirmation guard.
  const pushDirty = reportDirty('capture', 'the quick-capture note');
  $effect(() => {
    pushDirty(title.trim() !== '' || body.trim() !== '' || labels.length > 0);
  });

  // Reset on demand. Fired by the backend's main-close handler when the
  // user picks "Hide and discard" — we drop the typed text so the popup
  // doesn't reappear with stale content next time it's shown.
  let unlistenReset: UnlistenFn | undefined;
  onMount(async () => {
    unlistenReset = await listen('capture-reset', () => {
      title = '';
      body = '';
      labels = [];
      status = 'idle';
      errorMessage = '';
    });
  });
  onDestroy(() => {
    if (unlistenReset) unlistenReset();
  });

  async function submit() {
    if (!body.trim() && !title.trim()) {
      return;
    }

    status = 'saving';
    errorMessage = '';

    try {
      await invoke('create_note', {
        input: {
          title: title.trim() || null,
          body,
          labels
        }
      });

      // Reset form and close popup. The user can re-open it via the tray for the next capture.
      title = '';
      body = '';
      labels = [];
      status = 'idle';
      await getCurrentWindow().hide();
    } catch (err) {
      status = 'error';
      errorMessage = String(err);
    }
  }

  function handleSubmit(e: Event) {
    e.preventDefault();
    submit();
  }

  async function handleKeydown(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
      e.preventDefault();
      submit();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      // Esc dismisses the popup but DOES preserve typed text. Svelte state
      // persists across hide because the window is hidden, not destroyed
      // (see lib.rs capture close handler). Unsaved text remains in the
      // dirty registry — try_quit will warn before exit.
      await getCurrentWindow().hide();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<main>
  <form onsubmit={handleSubmit}>
    <input
      class="title-input"
      type="text"
      placeholder="Title (optional)"
      bind:value={title}
    />

    <!-- svelte-ignore a11y_autofocus -->
    <textarea
      class="body-input"
      placeholder="What did you just do?"
      bind:value={body}
      autofocus
    ></textarea>

    <LabelInput bind:labels placeholder="Labels (type to search, Enter to add)" />

    <div class="actions">
      <button
        type="submit"
        class="btn btn-emerald"
        disabled={status === 'saving' || (!body.trim() && !title.trim())}
      >
        {status === 'saving' ? 'Saving…' : 'Submit'}
      </button>
      <span class="hint">⌘↩ submit · esc close</span>
    </div>

    {#if status === 'error'}
      <p class="status status-error">Error: {errorMessage}</p>
    {/if}
  </form>
</main>

<style>
  main {
    padding: var(--space-4);
    height: 100vh;
    display: flex;
    flex-direction: column;
  }

  form {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    flex: 1;
    min-height: 0;
  }

  input,
  textarea {
    width: 100%;
    padding: var(--space-3);
    background: var(--bg-surface);
    color: var(--text-primary);
    border: 1px solid var(--border-structural);
    border-radius: var(--radius-md);
    font-family: var(--font-body);
    font-size: var(--text-body);
    line-height: var(--text-body-lh);
    transition: border-color var(--transition-fast);
  }

  textarea {
    flex: 1;
    resize: none;
    min-height: 100px;
  }

  input:focus-visible,
  textarea:focus-visible {
    outline: none;
    border-color: var(--accent-primary);
    box-shadow: 0 0 0 2px var(--focus-glow);
  }

  .title-input {
    font-family: var(--font-display);
    font-size: var(--text-display-sm);
    line-height: var(--text-display-sm-lh);
  }

  .actions {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }

  .hint {
    font-size: var(--text-caption);
    line-height: var(--text-caption-lh);
    color: var(--text-secondary);
  }

  .status {
    margin: 0;
    padding: var(--space-3);
    border-radius: var(--radius-md);
    font-size: var(--text-body);
  }

  .status-error {
    background: rgba(235, 1, 139, 0.12);
    color: var(--accent-pink);
    border: 1px solid rgba(235, 1, 139, 0.4);
  }
</style>
