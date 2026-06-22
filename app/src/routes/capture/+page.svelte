<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { getCurrentWindow } from '@tauri-apps/api/window';

  type Status = 'idle' | 'saving' | 'saved' | 'error';

  let title = $state('');
  let body = $state('');
  let labelsInput = $state('');
  let status = $state<Status>('idle');
  let errorMessage = $state('');

  async function submit() {
    if (!body.trim() && !title.trim()) {
      return;
    }

    status = 'saving';
    errorMessage = '';

    const labels = labelsInput
      .split(/[\s,]+/)
      .map((l) => l.trim().replace(/^#+/, ''))
      .filter((l) => l.length > 0);

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
      labelsInput = '';
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
      // Esc dismisses the popup. If there's unsaved content, we still close
      // (the popup re-opens with empty form on next tray click — explicit draft
      // persistence is a Phase 3 polish item).
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

    <input
      class="labels-input"
      type="text"
      placeholder="e.g. release, journal-app"
      bind:value={labelsInput}
    />

    <div class="actions">
      <button
        type="submit"
        class="btn btn-emerald"
        disabled={status === 'saving' || (!body.trim() && !title.trim())}
      >
        {status === 'saving' ? 'Saving...' : 'Submit'}
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
    border: 1px solid var(--border-subtle);
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
    box-shadow: 0 0 0 2px rgba(255, 92, 8, 0.25);
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
