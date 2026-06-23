<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { confirm } from '@tauri-apps/plugin-dialog';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import LabelInput from '$lib/LabelInput.svelte';
  import SpellcheckTextarea from '$lib/SpellcheckTextarea.svelte';
  import { reportDirty } from '$lib/dirty';

  // Submit lifecycle — distinct from auto-save status. 'submitting' is the
  // brief window while create_note is in flight; everything else (typing,
  // draft saves) is tracked separately via DraftStatus.
  type SubmitStatus = 'idle' | 'submitting' | 'error';

  // Auto-save (draft persistence) lifecycle. Same shape as the /summary
  // route's saveStatus for consistency.
  type DraftStatus = 'idle' | 'dirty' | 'saving' | 'saved' | 'error';

  const AUTOSAVE_DEBOUNCE_MS = 1500;

  type CaptureDraft = {
    title: string | null;
    body: string;
    labels: string[];
  };

  // ---------- state ----------
  let title = $state('');
  let body = $state('');
  let labels = $state<string[]>([]);
  let submitStatus = $state<SubmitStatus>('idle');
  let submitErrorMessage = $state('');

  let draftStatus = $state<DraftStatus>('idle');
  let draftErrorMessage = $state('');
  let lastSavedAt = $state<Date | null>(null);
  let initialLoadDone = $state(false);

  // Snapshot of the last-saved-to-disk draft. Compared against the live
  // form values to derive isDirty. Reset on (a) initial draft load,
  // (b) successful auto-save, (c) successful Submit, (d) capture-reset event.
  let snapshot = $state({
    title: '',
    body: '',
    labelsJson: '[]'
  });

  const isDirty = $derived(
    initialLoadDone &&
      ((title || '') !== snapshot.title ||
        (body || '') !== snapshot.body ||
        JSON.stringify(labels) !== snapshot.labelsJson)
  );

  // Cross-window dirty tracking. Once auto-save lands a draft to disk the
  // capture is no longer "at risk" — isDirty flips false and the quit guard
  // doesn't prompt about it.
  const pushDirty = reportDirty('capture', 'the quick-capture note');
  $effect(() => pushDirty(isDirty));

  // ---------- auto-save plumbing ----------
  let autoSaveTimer: ReturnType<typeof setTimeout> | null = null;

  // Re-runs on every keystroke. Reschedules a 1.5s debounce — when it fires
  // we persist the in-flight draft to .metadata/capture-draft.json. Same
  // pattern as /summary, scaled down for the popup's narrower surface.
  $effect(() => {
    title;
    body;
    labels;

    if (!initialLoadDone) return;
    if (!isDirty) return;

    draftStatus = 'dirty';
    if (autoSaveTimer) clearTimeout(autoSaveTimer);
    autoSaveTimer = setTimeout(() => {
      autoSaveTimer = null;
      void saveDraft();
    }, AUTOSAVE_DEBOUNCE_MS);
  });

  async function saveDraft() {
    if (draftStatus === 'saving') return;
    if (autoSaveTimer) {
      clearTimeout(autoSaveTimer);
      autoSaveTimer = null;
    }

    // Capture the snapshot BEFORE the await so subsequent keystrokes during
    // the in-flight save correctly mark the form as dirty again.
    const committed = {
      title: title || '',
      body: body || '',
      labelsJson: JSON.stringify(labels),
      labelsCopy: [...labels]
    };

    draftStatus = 'saving';
    draftErrorMessage = '';
    try {
      await invoke('save_capture_draft', {
        draft: {
          title: committed.title.trim() ? committed.title : null,
          body: committed.body,
          labels: committed.labelsCopy
        }
      });
      snapshot = {
        title: committed.title,
        body: committed.body,
        labelsJson: committed.labelsJson
      };
      lastSavedAt = new Date();
      draftStatus = 'saved';
    } catch (err) {
      draftErrorMessage = String(err);
      draftStatus = 'error';
    }
  }

  // ---------- load / restore ----------
  onMount(async () => {
    // Restore the saved draft (if any) before enabling auto-save — otherwise
    // the load itself would look like "the user typed something" and we'd
    // immediately write the same content back.
    try {
      const draft = await invoke<CaptureDraft | null>('load_capture_draft');
      if (draft) {
        title = draft.title ?? '';
        body = draft.body ?? '';
        labels = draft.labels ?? [];
      }
    } catch (err) {
      console.error('[capture] load_capture_draft failed:', err);
    }

    // Baseline the snapshot to whatever we just loaded (or empty if no draft).
    snapshot = {
      title: title || '',
      body: body || '',
      labelsJson: JSON.stringify(labels)
    };
    initialLoadDone = true;
  });

  onDestroy(() => {
    if (autoSaveTimer) clearTimeout(autoSaveTimer);
  });

  function resetFormState() {
    title = '';
    body = '';
    labels = [];
    snapshot = { title: '', body: '', labelsJson: '[]' };
    submitStatus = 'idle';
    submitErrorMessage = '';
    draftStatus = 'idle';
    draftErrorMessage = '';
    lastSavedAt = null;
  }

  // ---------- submit (commit draft to a real Note) ----------
  async function submit() {
    if (!body.trim() && !title.trim()) {
      return;
    }

    // Cancel any pending auto-save — the draft is about to become a real
    // Note and we're about to clear the draft file. A stale auto-save
    // firing after clear would resurrect the just-submitted content.
    if (autoSaveTimer) {
      clearTimeout(autoSaveTimer);
      autoSaveTimer = null;
    }

    submitStatus = 'submitting';
    submitErrorMessage = '';
    try {
      await invoke('create_note', {
        input: {
          title: title.trim() || null,
          body,
          labels
        }
      });
      await invoke('clear_capture_draft').catch(() => {});
      resetFormState();
      await getCurrentWindow().hide();
    } catch (err) {
      submitStatus = 'error';
      submitErrorMessage = String(err);
    }
  }

  function handleSubmit(e: Event) {
    e.preventDefault();
    void submit();
  }

  /// Discard the in-flight note: confirm with the user, then cancel any
  /// pending auto-save, delete the draft file, clear form state, and hide
  /// the popup. Destructive — there's no undo, hence the confirmation.
  async function discardWithConfirm() {
    if (!hasContent) return;
    const ok = await confirm(
      "This will delete the in-progress note and clear the saved draft. " +
        "This can't be undone.",
      {
        title: 'Discard this note?',
        kind: 'warning',
        okLabel: 'Discard',
        cancelLabel: 'Keep editing'
      }
    );
    if (!ok) return;

    if (autoSaveTimer) {
      clearTimeout(autoSaveTimer);
      autoSaveTimer = null;
    }
    await invoke('clear_capture_draft').catch(() => {});
    resetFormState();
    await getCurrentWindow().hide();
  }

  // True when there's anything worth keeping/discarding. Mirrors the
  // Submit-button enablement check so both actions are disabled on an
  // empty form.
  const hasContent = $derived(
    title.trim() !== '' || body.trim() !== '' || labels.length > 0
  );

  async function handleKeydown(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
      e.preventDefault();
      void submit();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      // Esc dismisses the popup but DOES preserve typed text. The draft
      // already auto-saved (or will on the next tick), so closing isn't
      // destructive — the popup reopens with the same content.
      await getCurrentWindow().hide();
    }
  }

  // ---------- indicator text ----------
  function formatTime(d: Date): string {
    return d.toLocaleTimeString('en-US', { hour: 'numeric', minute: '2-digit' });
  }

  const draftStatusText = $derived.by(() => {
    switch (draftStatus) {
      case 'saving':
        return 'Saving draft…';
      case 'dirty':
        return 'Unsaved changes';
      case 'saved':
        return lastSavedAt ? `Draft saved ${formatTime(lastSavedAt)}` : 'Draft saved';
      case 'error':
        return "Couldn't save draft";
      case 'idle':
      default:
        return '';
    }
  });
</script>

<svelte:window onkeydown={handleKeydown} />

<main>
  <form onsubmit={handleSubmit}>
    <input
      class="title-input"
      type="text"
      placeholder="Title (optional)"
      spellcheck="true"
      bind:value={title}
    />

    <!-- Body wraps in SpellcheckTextarea so misspellings get a wavy-red
      underline. The wrapper picks up `.sq-grow` to flex-grow like the
      original textarea did; see :global() rules at the bottom of <style>
      that route the look to the inner textarea. -->
    <!-- svelte-ignore a11y_autofocus -->
    <SpellcheckTextarea
      class="body-input"
      placeholder="What did you just do?"
      bind:value={body}
      style="flex: 1; min-height: 100px;"
      autofocus
    />


    <LabelInput bind:labels placeholder="Labels (type to search, Enter to add)" />

    <div class="actions">
      <!-- Ruby (destructive) on the left, primary Emerald on the right —
        per the RPG style-guide convention for modal footers. -->
      <button
        type="button"
        class="btn btn-ruby"
        onclick={() => void discardWithConfirm()}
        disabled={!hasContent || submitStatus === 'submitting'}
      >
        Discard
      </button>
      <span class="hint">⌘↩ submit · esc close</span>
      <button
        type="submit"
        class="btn btn-emerald btn-submit"
        disabled={submitStatus === 'submitting' || !hasContent}
      >
        {submitStatus === 'submitting' ? 'Submitting…' : 'Submit'}
      </button>
    </div>

    <!-- Draft auto-save indicator — kept subtle in this tight popup. -->
    {#if draftStatusText}
      <p class="draft-status is-{draftStatus}">{draftStatusText}</p>
    {/if}

    {#if submitStatus === 'error'}
      <p class="status status-error">Error: {submitErrorMessage}</p>
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

  /* Title input is still rendered directly by this template, so the
   * unscoped element selector applies normally. */
  input {
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

  input:focus-visible {
    outline: none;
    border-color: var(--accent-primary);
    box-shadow: 0 0 0 2px var(--focus-glow);
  }

  /* Body textarea now lives inside <SpellcheckTextarea>. Reach across
   * the component boundary with :global() to give it the same chrome
   * the unwrapped textarea had. flex: 1 makes it fill the remaining
   * vertical space in the form column. */
  :global(textarea.sq-textarea.body-input) {
    width: 100%;
    flex: 1;
    resize: none;
    min-height: 100px;
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

  :global(textarea.sq-textarea.body-input:focus-visible) {
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

  /* Pushes Submit to the right; hint absorbs the remaining horizontal
   * space between Discard (left) and Submit (right). */
  .btn-submit {
    margin-left: auto;
  }

  .hint {
    font-size: var(--text-caption);
    line-height: var(--text-caption-lh);
    color: var(--text-secondary);
  }

  /* Draft auto-save indicator — small italic line below the actions row.
   * Matches the /summary route's .save-status pattern but compacted for
   * the popup's smaller surface. */
  .draft-status {
    margin: 0;
    font-size: var(--text-caption);
    line-height: var(--text-caption-lh);
    font-style: italic;
    color: var(--text-muted);
  }

  .draft-status.is-dirty,
  .draft-status.is-saving {
    color: var(--text-secondary);
  }

  .draft-status.is-error {
    color: var(--accent-pink);
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
