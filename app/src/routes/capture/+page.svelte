<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import LabelInput from '$lib/LabelInput.svelte';
  import MarkdownEditor from '$lib/MarkdownEditor.svelte';
  import SaveStatus from '$lib/SaveStatus.svelte';
  import ConfirmDialog from '$lib/ConfirmDialog.svelte';
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
  // Phase 2.8 follow-on: drives chip rendering in LabelInput. Read on mount
  // from get_settings and refreshed on 'settings-changed' so toggling the
  // Theme tab's switch updates this popup live without a remount.
  let colorfulLabels = $state(false);
  let unlistenSettings: UnlistenFn | undefined;
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

  // Pull only the one bool we need; broader settings live in /settings.
  async function refreshColorfulLabels(): Promise<void> {
    try {
      const s = await invoke<{ colorfulLabels?: boolean }>('get_settings');
      colorfulLabels = s.colorfulLabels ?? false;
    } catch {
      // Pre-storage / first-run — leave at default false.
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

    await refreshColorfulLabels();
    unlistenSettings = await listen('settings-changed', () => {
      void refreshColorfulLabels();
    });

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
    if (unlistenSettings) unlistenSettings();
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

  // Discard flow — two steps so the confirmation lives in our shared
  // ConfirmDialog component (consistent shell + a11y + dim/blur backdrop
  // with the rest of the app) instead of a native OS confirm sheet. The
  // Discard button flips showDiscardConfirm; the user picks Confirm or
  // Cancel inside the dialog; performDiscard runs only on Confirm.
  let showDiscardConfirm = $state(false);

  function requestDiscard(): void {
    if (!hasContent) return;
    showDiscardConfirm = true;
  }

  async function performDiscard(): Promise<void> {
    showDiscardConfirm = false;
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

  // formatTime + draftStatusText now live inside <SaveStatus>; the
  // draft-flavor copy ("Saving draft…", "Draft saved", "Couldn't save
  // draft") is passed via savingText / savedPrefix / errorText props.
</script>

<svelte:window onkeydown={handleKeydown} />

<main>
  <form onsubmit={handleSubmit}>
    <input
      class="text-input title-input"
      type="text"
      placeholder="Title (optional)"
      spellcheck="true"
      bind:value={title}
    />

    <!-- Body uses MarkdownEditor (CodeMirror 6) with live-preview decorations:
       markers (`**`, `*`, `~~`, `` ` ``, `#`, `-`, `>`, `[`/`](url)`) hide
       as atomic ranges so the user sees rendered rich text while the
       buffer stays canonical markdown on disk. WebKit handles spell-check
       on the contenteditable surface natively. The editor's `value` is
       one-way + `onChange` push-back; the auto-save $effect on `body`
       handles the debounce. -->
    <!-- svelte-ignore a11y_autofocus -->
    <MarkdownEditor
      class="body-input"
      placeholder="What did you just do?"
      value={body}
      onChange={(v) => (body = v)}
      style="flex: 1; min-height: 100px;"
      autofocus
      livePreview
    />


    <LabelInput
      bind:labels
      placeholder="Labels (type to search, Enter to add)"
      {colorfulLabels}
    />

    <!-- Primary on the left (Submit), destructive ruby on the right
       (Discard) — per the app-wide convention finalized in Phase 2.7.
       Save status sits leftmost in the same row, matching /journal +
       /summary so users find autosave indicators in one consistent spot. -->
    <div class="actions">
      <SaveStatus
        status={draftStatus}
        lastSavedAt={lastSavedAt}
        savingText="Saving draft…"
        savedPrefix="Draft saved"
        errorText="Couldn't save draft"
      />
      <span class="hint">⌘↩ submit · esc close</span>
      <button
        type="submit"
        class="btn btn-emerald btn-submit"
        disabled={submitStatus === 'submitting' || !hasContent}
      >
        {submitStatus === 'submitting' ? 'Submitting…' : 'Submit'}
      </button>
      <button
        type="button"
        class="btn btn-ruby"
        onclick={requestDiscard}
        disabled={!hasContent || submitStatus === 'submitting'}
      >
        Discard
      </button>
    </div>

    {#if submitStatus === 'error'}
      <p class="status status-error">Error: {submitErrorMessage}</p>
    {/if}
  </form>
</main>

{#if showDiscardConfirm}
  <ConfirmDialog
    title="Discard This Note?"
    confirmLabel="Discard"
    confirmVariant="ruby"
    cancelLabel="Keep Editing"
    cancelVariant="marble"
    onConfirm={() => void performDiscard()}
    onCancel={() => (showDiscardConfirm = false)}
    body={discardConfirmBody}
  />
{/if}

{#snippet discardConfirmBody()}
  <p>
    This will delete the in-progress note and clear the saved draft.
    This can't be undone.
  </p>
{/snippet}

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

  /* Title input uses the shared .text-input utility from app.css and
     adds a .title-input modifier for the display-font + bigger size. */
  .title-input {
    font-family: var(--font-display);
    font-size: var(--text-display-sm);
    line-height: var(--text-display-sm-lh);
  }

  /* Body editor (MarkdownEditor / CodeMirror 6) — chrome (background,
   * border, focus glow) lives inside the component itself. Only flex
   * sizing is set on the wrapper via inline `style` on the element. */

  .actions {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }

  /* Pushes the button cluster (Submit + Discard) to the right; save
   * status + ⌘↩ hint absorb the remaining horizontal space on the left. */
  .btn-submit {
    margin-left: auto;
  }

  .hint {
    font-size: var(--text-caption);
    line-height: var(--text-caption-lh);
    color: var(--text-secondary);
  }

  /* Draft auto-save indicator now uses the shared <SaveStatus>
     component — the local .draft-status class went away with the
     extraction. */

  /* .status + .status-error live in app.css as a shared utility — same
     error-pill is used here, on /summary, and inside SendToManagerButton. */
</style>
