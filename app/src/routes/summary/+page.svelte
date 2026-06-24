<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { invoke } from '@tauri-apps/api/core';
  import { openUrl, openPath } from '@tauri-apps/plugin-opener';
  import LabelInput from '$lib/LabelInput.svelte';
  import MarkdownEditor from '$lib/MarkdownEditor.svelte';
  import { reportDirty } from '$lib/dirty';

  type YearWeek = { year: number; week: number };

  type WeeklySummary = {
    keyAccomplishments: string;
    plansAndPriorities: string;
    challengesOrRoadblocks: string;
    anythingElse: string;
    labels: string[];
    lastUpdated: string | null;
  };

  type SentRecord = {
    sentAt: string;       // RFC 3339
    contentHash: string;  // SHA-256 hex of canonical summary
    sentTo: string;
  };

  // compose_weekly_email returns an externally-tagged enum: either an opener
  // URL (mailto:) or a path (.eml). Frontend dispatches by `kind`.
  type ComposeResult =
    | { kind: 'mailto'; value: string }
    | { kind: 'eml'; value: string };

  // Auto-save status. 'idle' = settled, no unsaved edits and no recent save
  // to advertise. 'dirty' = typed something, debounce timer pending. 'saving'
  // = invoke in-flight. 'saved' = last write succeeded; show the timestamp.
  // 'error' = last save threw; show retry affordance.
  type SaveStatus = 'idle' | 'dirty' | 'saving' | 'saved' | 'error';

  const AUTOSAVE_DEBOUNCE_MS = 1500;

  // State
  let loading = $state(true);
  let loadError = $state('');
  let saveStatus = $state<SaveStatus>('idle');
  let saveErrorMessage = $state('');
  let lastSavedAt = $state<Date | null>(null);

  let yearWeek = $state<YearWeek | null>(null);
  let lastUpdated = $state<string | null>(null);

  let keyAccomplishments = $state('');
  let plansAndPriorities = $state('');
  let challengesOrRoadblocks = $state('');
  let anythingElse = $state('');
  let labels = $state<string[]>([]);

  // Last-saved snapshot. We compare the live form values against this to
  // know whether the route is "dirty" (has unsaved edits). Reset on load
  // and after a successful save.
  let snapshot = $state({
    keyAccomplishments: '',
    plansAndPriorities: '',
    challengesOrRoadblocks: '',
    anythingElse: '',
    labelsJson: '[]'
  });

  // Send-to-manager state (Phase 2.6). All four pieces are loaded once on
  // mount and updated after each successful save.
  //   - managerEmail drives "can we send at all".
  //   - sentRecord is the latest entry in .metadata/sent-log.json for this
  //     week (or null if this week has never been sent).
  //   - currentHash is the SHA-256 of the WeeklySummary as it sits on disk
  //     RIGHT NOW (after the most recent save), used to detect "edited
  //     since last send" by comparing against sentRecord.contentHash.
  //   - isSending is the in-flight flag while compose → opener → mark is
  //     happening; gates the Send button against double-clicks.
  let managerEmail = $state<string | null>(null);
  let sentRecord = $state<SentRecord | null>(null);
  let currentHash = $state('');
  let isSending = $state(false);
  let sendError = $state('');
  let showConfirmModal = $state(false);

  const isDirty = $derived(
    !loading &&
      (keyAccomplishments !== snapshot.keyAccomplishments ||
        plansAndPriorities !== snapshot.plansAndPriorities ||
        challengesOrRoadblocks !== snapshot.challengesOrRoadblocks ||
        anythingElse !== snapshot.anythingElse ||
        JSON.stringify(labels) !== snapshot.labelsJson)
  );

  const pushDirty = reportDirty('summary', 'the weekly summary');
  $effect(() => pushDirty(isDirty));

  // Auto-save: debounced 1.5s after typing stops. We touch the inputs
  // explicitly so this effect re-runs on every keystroke (a derived bool
  // like isDirty wouldn't, since once it's true it stays true).
  let autoSaveTimer: ReturnType<typeof setTimeout> | null = null;

  $effect(() => {
    keyAccomplishments;
    plansAndPriorities;
    challengesOrRoadblocks;
    anythingElse;
    labels;

    if (loading) return;
    if (!isDirty) return;

    saveStatus = 'dirty';
    if (autoSaveTimer) clearTimeout(autoSaveTimer);
    autoSaveTimer = setTimeout(() => {
      autoSaveTimer = null;
      void saveNow();
    }, AUTOSAVE_DEBOUNCE_MS);
  });

  // Computed week range label like "Week of June 22 – June 28, 2026"
  const weekLabel = $derived.by(() => {
    if (!yearWeek) return '';
    // ISO week → Monday of that week. JS doesn't have a built-in for ISO week
    // arithmetic, so do it manually: find the year's Jan 4 (always in week 1),
    // then offset.
    const { year, week } = yearWeek;
    const jan4 = new Date(year, 0, 4);
    const jan4Day = jan4.getDay() || 7; // Sunday → 7
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
  });

  onMount(async () => {
    try {
      yearWeek = await invoke<YearWeek>('get_current_year_week');
      const s = await invoke<WeeklySummary>('get_weekly_summary', {
        year: yearWeek.year,
        week: yearWeek.week
      });
      keyAccomplishments = s.keyAccomplishments;
      plansAndPriorities = s.plansAndPriorities;
      challengesOrRoadblocks = s.challengesOrRoadblocks;
      anythingElse = s.anythingElse;
      labels = s.labels ?? [];
      lastUpdated = s.lastUpdated;
      // Baseline the dirty-comparison snapshot to what we just loaded.
      snapshot = {
        keyAccomplishments,
        plansAndPriorities,
        challengesOrRoadblocks,
        anythingElse,
        labelsJson: JSON.stringify(labels)
      };
      // Load send-to-manager state in parallel. Failures here don't block
      // the summary itself — Send button just stays disabled.
      const [settings, record, hash] = await Promise.all([
        invoke<{ managerEmail: string | null }>('get_settings'),
        invoke<SentRecord | null>('get_sent_record', {
          year: yearWeek.year,
          week: yearWeek.week
        }),
        invoke<string>('get_summary_hash', {
          year: yearWeek.year,
          week: yearWeek.week
        })
      ]);
      managerEmail = settings.managerEmail;
      sentRecord = record;
      currentHash = hash;
    } catch (err) {
      loadError = String(err);
    } finally {
      loading = false;
    }
  });

  /// Save the current form to disk. Used by both the auto-save debounce and
  /// the manual Save button + Cmd+S / Cmd+↩ shortcuts. Idempotent: returns
  /// early if a save is already in flight.
  async function saveNow() {
    if (!yearWeek) return;
    if (saveStatus === 'saving') return;
    if (autoSaveTimer) {
      clearTimeout(autoSaveTimer);
      autoSaveTimer = null;
    }

    // Capture what we're about to save BEFORE the await so we can re-baseline
    // the snapshot to exactly what hit disk — even if the user keeps typing
    // during the await, isDirty will correctly flip back to true after this
    // save completes and trigger another auto-save.
    const committed = {
      keyAccomplishments,
      plansAndPriorities,
      challengesOrRoadblocks,
      anythingElse,
      labelsJson: JSON.stringify(labels),
      labels: [...labels]
    };

    saveStatus = 'saving';
    saveErrorMessage = '';
    try {
      await invoke('update_weekly_summary', {
        input: {
          year: yearWeek.year,
          week: yearWeek.week,
          keyAccomplishments: committed.keyAccomplishments,
          plansAndPriorities: committed.plansAndPriorities,
          challengesOrRoadblocks: committed.challengesOrRoadblocks,
          anythingElse: committed.anythingElse,
          labels: committed.labels
        }
      });
      // Refresh lastUpdated from the server (avoids drift from frontend clock).
      const refreshed = await invoke<WeeklySummary>('get_weekly_summary', {
        year: yearWeek.year,
        week: yearWeek.week
      });
      lastUpdated = refreshed.lastUpdated;
      snapshot = {
        keyAccomplishments: committed.keyAccomplishments,
        plansAndPriorities: committed.plansAndPriorities,
        challengesOrRoadblocks: committed.challengesOrRoadblocks,
        anythingElse: committed.anythingElse,
        labelsJson: committed.labelsJson
      };
      lastSavedAt = new Date();
      saveStatus = 'saved';
      // Recompute the content hash off the freshly-saved file. This is what
      // the Send button's "edited since last send" gate compares against
      // sentRecord.contentHash. Refreshing here keeps the gate accurate
      // immediately after a save (no delay before Send re-enables).
      if (yearWeek) {
        try {
          currentHash = await invoke<string>('get_summary_hash', {
            year: yearWeek.year,
            week: yearWeek.week
          });
        } catch {
          // If the hash refresh fails, leave currentHash stale — worst case
          // the Send button stays disabled until the next save. Better than
          // a phantom "Send updated version" that uses a hash we don't trust.
        }
      }
    } catch (err) {
      saveErrorMessage = String(err);
      saveStatus = 'error';
    }
  }

  /// Format a Date as "2:34 PM" — used in the "Saved HH:MM" status line.
  function formatTime(d: Date): string {
    return d.toLocaleTimeString('en-US', { hour: 'numeric', minute: '2-digit' });
  }

  // The save-status indicator text. Empty string when idle (don't clutter
  // the actions row with "nothing to report").
  const saveStatusText = $derived.by(() => {
    switch (saveStatus) {
      case 'saving':
        return 'Saving…';
      case 'dirty':
        return 'Unsaved changes';
      case 'saved':
        return lastSavedAt ? `Saved ${formatTime(lastSavedAt)}` : 'Saved';
      case 'error':
        return "Couldn't save — retry?";
      case 'idle':
      default:
        return '';
    }
  });

  function handleKeydown(e: KeyboardEvent) {
    // When the Send-confirmation modal is open, the window-level handler
    // gets two responsibilities: dismiss on Escape, and swallow Cmd-S /
    // Cmd-Enter so a stray hotkey can't trigger a save while the user is
    // mid-confirm. (Without this Escape branch the modal had no keyboard
    // dismiss — focus stays on the Send button when the modal opens, so
    // Escapes bubble to window and we have to catch them here.)
    if (showConfirmModal) {
      if (e.key === 'Escape') {
        e.preventDefault();
        dismissConfirm();
      } else if ((e.metaKey || e.ctrlKey) && (e.key === 's' || e.key === 'Enter')) {
        e.preventDefault();
      }
      return;
    }
    if ((e.metaKey || e.ctrlKey) && e.key === 's') {
      e.preventDefault();
      void saveNow();
    } else if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
      e.preventDefault();
      void saveNow();
    }
  }

  // ---------- Send to manager ----------
  //
  // Five layers gate the Send button. The order matters for what tooltip
  // we show, so they're evaluated separately rather than collapsed into a
  // single boolean.

  const hasManagerEmail = $derived((managerEmail ?? '').trim() !== '');

  // True when sent-log has a record for this week AND its content hash
  // matches what's currently on disk. This is the "nothing has changed
  // since the last send" state — the button stays disabled.
  const alreadySentUnchanged = $derived(
    sentRecord !== null &&
      currentHash !== '' &&
      sentRecord.contentHash === currentHash
  );

  // True when there's a sent record but the hash has drifted — user edited
  // and saved the summary after sending. Button re-enables with a different
  // label ("Send updated version") so the user knows this won't be the
  // first send.
  const editedAfterSend = $derived(
    sentRecord !== null &&
      currentHash !== '' &&
      sentRecord.contentHash !== currentHash
  );

  const sendDisabled = $derived(
    !hasManagerEmail ||
      isDirty ||
      saveStatus === 'saving' ||
      alreadySentUnchanged ||
      isSending
  );

  const sendButtonLabel = $derived.by(() => {
    if (isSending) return 'Opening mail app…';
    if (editedAfterSend) return 'Send updated version';
    return 'Send to manager';
  });

  // Tooltip explains why the button is disabled (or, when enabled, what it
  // will do). Spelled-out reasons help the user know what to do next rather
  // than face a grayed-out button with no explanation.
  const sendTooltip = $derived.by(() => {
    if (!hasManagerEmail) return 'Set a manager email in Settings to enable this.';
    if (isDirty) return 'Save your changes first.';
    if (saveStatus === 'saving') return 'Waiting for save to finish…';
    if (alreadySentUnchanged && sentRecord) {
      return `Sent ${sentRecord.sentTo} on ${formatSentAt(sentRecord.sentAt)}.`;
    }
    if (isSending) return 'Opening your mail app…';
    return `Opens a draft addressed to ${managerEmail} in your default mail app.`;
  });

  // Subtle in-row text shown next to the Send button when the week has been
  // sent at least once. Distinct from the saveStatus indicator (which is
  // about disk writes) — this one is about email lifecycle.
  const sentStatusText = $derived.by(() => {
    if (!sentRecord) return '';
    if (editedAfterSend) {
      return `Last sent ${formatSentAt(sentRecord.sentAt)} (edited since)`;
    }
    return `Sent ${formatSentAt(sentRecord.sentAt)}`;
  });

  // Lowercases ONLY the first character so weekLabel reads naturally inline
  // ("for the week of June 22…") without flattening the month names (Chris
  // flagged that `weekLabel.toLowerCase()` was making June lowercase too).
  function inlineLabel(label: string): string {
    if (label.length === 0) return label;
    return label.charAt(0).toLowerCase() + label.slice(1);
  }

  function formatSentAt(rfc3339: string): string {
    const d = new Date(rfc3339);
    if (Number.isNaN(d.getTime())) return rfc3339;
    // "Jun 24, 4:12 PM" — locale-agnostic enough for the audit-trail use case.
    return d.toLocaleString('en-US', {
      month: 'short',
      day: 'numeric',
      hour: 'numeric',
      minute: '2-digit'
    });
  }

  function onSendClick() {
    if (sendDisabled) return;
    sendError = '';
    showConfirmModal = true;
  }

  function dismissConfirm() {
    if (isSending) return;
    showConfirmModal = false;
  }

  /// Run the full send flow: compose → open with the right opener variant
  /// → stamp the sent-log entry. Closes the modal on success or surfaces
  /// the error via `sendError` on failure (modal stays open so the user
  /// can retry).
  async function confirmSend() {
    if (!yearWeek || !hasManagerEmail) return;
    isSending = true;
    sendError = '';
    try {
      const result = await invoke<ComposeResult>('compose_weekly_email', {
        year: yearWeek.year,
        week: yearWeek.week
      });
      if (result.kind === 'mailto') {
        await openUrl(result.value);
      } else {
        await openPath(result.value);
      }
      const record = await invoke<SentRecord>('mark_weekly_summary_sent', {
        year: yearWeek.year,
        week: yearWeek.week,
        contentHash: currentHash,
        sentTo: (managerEmail ?? '').trim()
      });
      sentRecord = record;
      showConfirmModal = false;
    } catch (err) {
      sendError = String(err);
    } finally {
      isSending = false;
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if loading}
  <main class="loading">
    <p>Loading…</p>
  </main>
{:else if loadError}
  <main class="loading">
    <div class="card error-card">
      <h2>Couldn't load this week.</h2>
      <p>{loadError}</p>
      <button class="btn btn-marble" onclick={() => goto('/')}>Back</button>
    </div>
  </main>
{:else}
  <main>
    <section class="page">
      <header>
        <h1>Weekly Summary</h1>
        <p class="subtitle">{weekLabel}</p>
        {#if lastUpdated}
          <p class="last-updated">Last updated: {lastUpdated}</p>
        {/if}
      </header>

      <div class="form">
        <!-- Phase 2.5 Step 4: each field is a CodeMirror MarkdownEditor.
          Native WebKit spell-check + clickable Markdown links + GFM
          parsing for free; auto-save flow stays the same because the
          editor's onChange wires straight into the existing dirty/$effect
          debounce. The `style="--md-min-height: ..."` numbers approximate
          the prior `rows={3|4|5}` initial heights (~22px line-height
          + 24px vertical padding). The editor scrolls internally when
          content exceeds; resize: vertical on the wrapper lets the user
          drag-grow each field, matching the textarea-era affordance. -->
        <div class="field">
          <label for="key-acc">Key accomplishments</label>
          <MarkdownEditor
            id="key-acc"
            value={keyAccomplishments}
            onChange={(v) => (keyAccomplishments = v)}
            placeholder="- "
            style="--md-min-height: 134px; resize: vertical; overflow: hidden;"
          />
        </div>

        <div class="field">
          <label for="plans">Plans and priorities for next week</label>
          <MarkdownEditor
            id="plans"
            value={plansAndPriorities}
            onChange={(v) => (plansAndPriorities = v)}
            placeholder="- "
            style="--md-min-height: 112px; resize: vertical; overflow: hidden;"
          />
        </div>

        <div class="field">
          <label for="challenges">Challenges or roadblocks</label>
          <MarkdownEditor
            id="challenges"
            value={challengesOrRoadblocks}
            onChange={(v) => (challengesOrRoadblocks = v)}
            placeholder="- "
            style="--md-min-height: 90px; resize: vertical; overflow: hidden;"
          />
        </div>

        <div class="field">
          <label for="else">Anything else on your mind</label>
          <MarkdownEditor
            id="else"
            value={anythingElse}
            onChange={(v) => (anythingElse = v)}
            placeholder=""
            style="--md-min-height: 90px; resize: vertical; overflow: hidden;"
          />
        </div>

        <div class="field">
          <span class="field-heading">Labels</span>
          <LabelInput bind:labels placeholder="Tag this week (type to search, Enter to add)" />
        </div>

        {#if saveStatus === 'error' && saveErrorMessage}
          <p class="status status-error">Error: {saveErrorMessage}</p>
        {/if}

        <div class="actions">
          <button
            class="btn btn-marble"
            onclick={() => goto('/')}
            disabled={saveStatus === 'saving'}
          >
            Done
          </button>
          <!-- Auto-save status indicator. Click-to-retry when in error state. -->
          {#if saveStatus === 'error'}
            <button
              type="button"
              class="save-status is-error"
              onclick={() => void saveNow()}
            >
              {saveStatusText}
            </button>
          {:else}
            <span class="save-status is-{saveStatus}">{saveStatusText}</span>
          {/if}
          <button
            class="btn btn-emerald btn-save"
            onclick={() => void saveNow()}
            disabled={saveStatus === 'saving'}
          >
            {saveStatus === 'saving' ? 'Saving…' : 'Save'}
          </button>
          <!-- Send to manager (Phase 2.6). Sits to the right of Save. Marble
            (secondary) so it doesn't compete with the primary Save action.
            Tooltip is the long form of "why disabled" / "what this does";
            sentStatusText below the row carries the at-a-glance lifecycle. -->
          <button
            class="btn btn-marble btn-send"
            onclick={onSendClick}
            disabled={sendDisabled}
            title={sendTooltip}
          >
            {sendButtonLabel}
          </button>
        </div>

        {#if sentStatusText}
          <p class="sent-status" class:is-stale={editedAfterSend}>
            {sentStatusText}
          </p>
        {/if}
      </div>
    </section>
  </main>

  {#if showConfirmModal}
    <!-- Confirmation modal for the Send-to-manager flow. Backdrop click
       dismisses; Escape dismiss is handled by handleKeydown above (focus
       stays on the Send button when the modal opens, so window-level
       capture is the only reliable place to catch Escape). The primary
       Open button calls confirmSend. -->
    <!-- svelte-ignore a11y_click_events_have_key_events — Escape is handled
       at window level by handleKeydown (focus stays on the Send button when
       the modal opens, so a backdrop-scoped keydown handler never fires).
       The backdrop's click dismissal is a mouse-only convenience on top of
       Cancel + Escape, not the primary close affordance. -->
    <div
      class="modal-backdrop"
      onclick={dismissConfirm}
      role="presentation"
    >
      <div
        class="modal"
        role="dialog"
        tabindex="-1"
        aria-modal="true"
        aria-labelledby="confirm-title"
        onclick={(e) => e.stopPropagation()}
      >
        <h2 id="confirm-title">Send weekly summary?</h2>
        <p>
          This will open your default mail app with a draft addressed to
          <strong>{managerEmail}</strong> for the {inlineLabel(weekLabel)}.
          You'll review and send it from there — Captain's Log never sends mail on
          its own.
        </p>
        {#if editedAfterSend && sentRecord}
          <p class="modal-aside">
            Heads up: you previously sent this week on {formatSentAt(sentRecord.sentAt)}.
            This opens a new draft with the updated content.
          </p>
        {/if}
        {#if sendError}
          <p class="status status-error">Couldn't open mail app: {sendError}</p>
        {/if}
        <div class="modal-actions">
          <button class="btn btn-marble" onclick={dismissConfirm} disabled={isSending}>
            Cancel
          </button>
          <button
            class="btn btn-emerald"
            onclick={() => void confirmSend()}
            disabled={isSending}
          >
            {isSending ? 'Opening…' : 'Open draft'}
          </button>
        </div>
      </div>
    </div>
  {/if}
{/if}

<style>
  main {
    display: flex;
    justify-content: center;
    padding: var(--space-8) var(--space-4);
    min-height: 100vh;
  }

  main.loading {
    align-items: center;
  }

  .page {
    width: 100%;
    max-width: 720px;
  }

  header {
    margin-bottom: var(--space-8);
  }

  .subtitle {
    color: var(--text-secondary);
    margin-top: var(--space-2);
  }

  .last-updated {
    color: var(--text-muted);
    font-size: var(--text-caption);
    line-height: var(--text-caption-lh);
    margin-top: var(--space-1);
  }

  .form {
    display: flex;
    flex-direction: column;
    gap: var(--space-6);
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .field > label,
  .field > .field-heading {
    font-family: var(--font-display);
    font-size: var(--text-button);
    color: var(--text-primary);
  }

  /* Editor chrome (background, border, focus glow, font, line-height) is
   * owned by MarkdownEditor.svelte itself. Per-field initial height +
   * user-resize affordance is set inline on each MarkdownEditor's `style`
   * via --md-min-height + `resize: vertical`. */

  .actions {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    margin-top: var(--space-4);
  }

  .btn-save {
    margin-left: auto;
  }

  /* Send button sits flush against Save with a small gap. No left-margin
   * adjustment — Save's `margin-left: auto` already pushes both buttons
   * to the right side of the actions row. */
  .btn-send:disabled {
    cursor: not-allowed;
    opacity: 0.55;
  }

  /* Sent-state lifecycle line — appears below the actions row when this
   * week has been sent at least once. .is-stale modifier marks the
   * "edited since last send" state with a warm orange to draw attention
   * that re-sending will be a different message. */
  .sent-status {
    margin: var(--space-2) 0 0;
    font-size: var(--text-caption);
    line-height: var(--text-caption-lh);
    color: var(--text-muted);
    font-style: italic;
    text-align: right;
  }

  .sent-status.is-stale {
    color: var(--accent-primary);
  }

  /* ---- Send confirmation modal ---- */

  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
    padding: var(--space-4);
  }

  .modal {
    background: var(--bg-surface);
    border: 1px solid var(--border-structural);
    border-radius: var(--radius-lg);
    padding: var(--space-6);
    max-width: 480px;
    width: 100%;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.35);
  }

  .modal h2 {
    margin: 0 0 var(--space-3);
    font-family: var(--font-display);
    font-size: var(--text-display-sm);
    line-height: var(--text-display-sm-lh);
  }

  .modal p {
    margin: 0 0 var(--space-3);
    color: var(--text-secondary);
    line-height: var(--text-body-lh);
  }

  .modal p strong {
    color: var(--text-primary);
    font-weight: normal;
    font-family: var(--font-display);
  }

  .modal-aside {
    margin-top: var(--space-3);
    padding: var(--space-3);
    background: var(--bg-elevated);
    border-radius: var(--radius-md);
    border-left: 3px solid var(--accent-primary);
    color: var(--text-secondary) !important;
  }

  .modal-actions {
    display: flex;
    gap: var(--space-3);
    justify-content: flex-end;
    margin-top: var(--space-5);
  }

  /* Auto-save indicator — small italic text between Done and Save. State is
   * encoded via .is-{idle|dirty|saving|saved|error} modifier classes. Stays
   * intentionally subtle so it doesn't compete with the form content. */
  .save-status {
    font-size: var(--text-caption);
    line-height: var(--text-caption-lh);
    font-style: italic;
    color: var(--text-muted);
    /* The error variant is a button — strip default button chrome so it
     * matches the span variants except for the underline + cursor. */
    background: none;
    border: none;
    padding: 0;
    font-family: var(--font-body);
  }

  .save-status.is-saving,
  .save-status.is-dirty {
    color: var(--text-secondary);
  }

  .save-status.is-saved {
    color: var(--text-muted);
  }

  .save-status.is-error {
    color: var(--accent-pink);
    cursor: pointer;
    text-decoration: underline;
  }

  .save-status.is-error:hover {
    filter: brightness(1.1);
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

  .card {
    max-width: 480px;
    padding: var(--space-6);
    background: var(--bg-surface);
    border: 1px solid var(--border-structural);
    border-radius: var(--radius-lg);
  }

  .card h2 {
    margin-bottom: var(--space-3);
  }

  .card p {
    color: var(--text-secondary);
    margin-bottom: var(--space-3);
  }

  .error-card {
    background: rgba(235, 1, 139, 0.08);
    border-color: rgba(235, 1, 139, 0.4);
  }
</style>
