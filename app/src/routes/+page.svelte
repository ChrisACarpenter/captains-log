<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';

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

    // Parse labels: split on commas or whitespace, strip leading '#', drop empties.
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
      status = 'saved';
      title = '';
      body = '';
      labelsInput = '';

      // Clear the "saved" indicator after a moment so the next capture feels fresh.
      setTimeout(() => {
        if (status === 'saved') status = 'idle';
      }, 2500);
    } catch (err) {
      status = 'error';
      errorMessage = String(err);
    }
  }

  function handleSubmit(e: Event) {
    e.preventDefault();
    submit();
  }

  function handleKeydown(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
      e.preventDefault();
      submit();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<main>
  <section class="capture">
    <header>
      <h1>Captain's Log</h1>
      <p class="subtitle">Capture a note.</p>
    </header>

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
        rows="8"
        bind:value={body}
        autofocus
      ></textarea>

      <input
        class="labels-input"
        type="text"
        placeholder="Labels (comma-separated, # optional)"
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
        <span class="hint">⌘↩ to submit</span>
      </div>

      {#if status === 'saved'}
        <p class="status status-success">Captured. Note written to this week's file.</p>
      {/if}
      {#if status === 'error'}
        <p class="status status-error">Error: {errorMessage}</p>
      {/if}
    </form>
  </section>
</main>

<style>
  main {
    display: flex;
    justify-content: center;
    min-height: 100vh;
    padding: var(--space-8) var(--space-4);
  }

  .capture {
    width: 100%;
    max-width: 640px;
  }

  header {
    margin-bottom: var(--space-8);
  }

  .subtitle {
    margin-top: var(--space-2);
    font-size: var(--text-body);
    line-height: var(--text-body-lh);
    color: var(--text-secondary);
  }

  form {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
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
    resize: vertical;
    min-height: 160px;
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
    gap: var(--space-4);
  }

  .hint {
    font-size: var(--text-caption);
    line-height: var(--text-caption-lh);
    color: var(--text-muted);
  }

  .status {
    margin: 0;
    padding: var(--space-3);
    border-radius: var(--radius-md);
    font-size: var(--text-body);
  }

  .status-success {
    background: rgba(149, 193, 59, 0.15);
    color: var(--accent-green);
    border: 1px solid rgba(149, 193, 59, 0.4);
  }

  .status-error {
    background: rgba(235, 1, 139, 0.12);
    color: var(--accent-pink);
    border: 1px solid rgba(235, 1, 139, 0.4);
  }
</style>
