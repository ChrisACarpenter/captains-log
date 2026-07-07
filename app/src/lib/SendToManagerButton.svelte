<!--
  Send-to-Manager button + confirmation modal + sent-status display.

  Shared between /summary and /journal so both routes get the same
  "one-click handoff to default mail client" UX without duplicating the
  ~100 LOC of gating + compose flow + modal.

  Owns:
    - Load sentRecord + currentHash for the (year, week) on mount and
      whenever year/week changes.
    - Re-fetch hash + sentRecord when Rust emits `weekly-file-changed`
      for the same week (so saves on the OTHER route invalidate this
      route's gating state).
    - All "is the Send button enabled" derived gates.
    - The compose → open → mark-sent Tauri pipeline.
    - The confirmation modal (Backdrop + dialog + Escape).
    - The subtle "Sent 4:12 PM" / "Last sent X (edited since)" status
      line that appears below the actions row.

  Parent passes the current dirty + save state so the gates can include
  "save before sending" + "wait for save to finish".

  ## Props

      year, week  — ISO year/week the user wants to send.
      weekLabel   — human label like "Week of June 22 – June 28, 2026"
                    (used in modal copy).
      isDirty     — has-unsaved-edits flag from parent's form state.
      saveStatus  — current save status; 'saving' blocks the Send.

  Renders nothing extra in the parent's actions row beyond the button —
  the sentStatusText line sits as a sibling below; the modal portals to
  the document via `position: fixed` on the backdrop.
-->
<script lang="ts">
  import { onMount, onDestroy, untrack } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { openUrl, openPath } from '@tauri-apps/plugin-opener';
  import { writeHtml } from '@tauri-apps/plugin-clipboard-manager';
  import Modal from '$lib/Modal.svelte';
  import TipBubble from '$lib/onboarding/TipBubble.svelte';
  import type { AutoSaveStatus } from '$lib/save-status';

  type SentRecord = {
    sentAt: string;
    contentHash: string;
    sentTo: string;
  };

  // Mirrors the Rust `ComposeResult` enum (src-tauri/src/email.rs). Each
  // variant maps to a different hand-off mechanism, so the `kind`
  // discriminator doubles as the dispatch signal here in confirmSend().
  // Phase 2.9b added webUrl (Gmail / Outlook) and appleScript (Mac Mail)
  // alongside the legacy mailto + eml.
  type ComposeResult =
    | { kind: 'mailto'; value: string }
    | { kind: 'eml'; value: string }
    | { kind: 'webUrl'; url: string; truncationWarning: boolean }
    | { kind: 'appleScript'; script: string };

  type PreviewPayload = {
    subject: string;
    recipient: string;
    weekLabel: string;
    html: string;
    text: string;
    isResend: boolean;
  };

  type Props = {
    year: number;
    week: number;
    weekLabel: string;
    isDirty: boolean;
    saveStatus: AutoSaveStatus;
    /** Read-only outputs the parent can bind to so it can render the
     *  "Sent Jun 26 at 4:12 PM" / "Last sent … (edited since)" line
     *  wherever its layout calls for. The component owns the state
     *  derivation; the parent owns the placement.
     *
     *  Earlier shape rendered this status as a sibling of the button
     *  inside the actions row, which put the line beside the button
     *  instead of above it. Surfacing as bindable lets each route
     *  decide where the line lives. */
    sentStatusText?: string;
    sentStatusIsStale?: boolean;
  };

  let {
    year,
    week,
    weekLabel,
    isDirty,
    saveStatus,
    sentStatusText = $bindable(''),
    sentStatusIsStale = $bindable(false),
  }: Props = $props();

  // ----- State (loaded async on mount + on year/week change) -----

  let managerEmail = $state<string | null>(null);
  // Mail send-path settings read on mount + refreshed on settings-changed.
  // Passed to compose_weekly_email so the backend dispatch is explicit at
  // the call site. Defaults match JournalSettings::default in Rust — Gmail
  // mode with no user_email, which Gmail-side falls back to /mail/u/0.
  let mailSendMode = $state<'gmail' | 'native-mail' | 'outlook'>('gmail');
  // Native Mac Mail HTML toggle. Only meaningful when mailSendMode ===
  // 'native-mail'; drives the Preview modal's iframe-vs-<pre> render choice.
  let mailNativeHtml = $state(false);
  // Global "how does the body reach the compose window" setting.
  // 'prefilled' = embed plaintext in URL/AppleScript (default). 'clipboard-
  // paste' = open empty compose + writeHtml rich body to clipboard so user
  // can Cmd+V into the draft. Applies orthogonally to all three send modes.
  // The Native Mac '.eml' HTML toggle is an independent peer override —
  // when both are set, the .eml path wins (handled server-side).
  let mailBodyDelivery = $state<'prefilled' | 'clipboard-paste'>('prefilled');
  let userEmail = $state<string | null>(null);
  let sentRecord = $state<SentRecord | null>(null);
  let currentHash = $state('');
  let isSending = $state(false);
  let sendError = $state('');
  let showConfirmModal = $state(false);
  // Gmail truncation: set when the backend reports the encoded URL exceeded
  // ~2 KB. Replaces the normal Send button with a Send anyway / Cancel pair
  // so the user can choose to ship a possibly-truncated draft or shorten
  // their summary. Cleared on dismiss + on next click.
  let pendingTruncatedUrl = $state<string | null>(null);
  let truncationWarningText = $state('');

  // Compose+paste failure-mode UI. If writeHtml throws during a clipboard-
  // paste send (rare — clipboard manager permission denied, unsupported
  // target), we abort openUrl and surface this error block inside the
  // confirm modal with an "Open Preview" recovery affordance. Cleared
  // on next Send click and on modal dismiss.
  let clipboardPasteError = $state('');

  // Preview modal — opened from inside the confirm modal, layered above
  // it so the user can flip back to Cancel/Open-draft after peeking. The
  // payload is fetched on demand; previewStaleWarning surfaces if the
  // weekly file changes on disk while the preview is open.
  let showPreview = $state(false);
  let previewLoading = $state(false);
  let previewError = $state<string | null>(null);
  let previewPayload = $state<PreviewPayload | null>(null);
  let previewStaleWarning = $state('');
  // Monotonic counter that gates async preview responses. Every entry
  // point that should cancel an in-flight openPreview() (Escape, dismiss,
  // year/week change, settings reload, a fresh Send click) bumps this
  // counter; the awaited invoke result is dropped if the counter moved
  // while it was in flight.
  let previewToken = $state(0);

  // Clipboard state for the Preview modal's "Copy to clipboard" button.
  // copyInFlight gates re-entrancy while writeHtml is awaiting.
  // copyConfirmation flips true on success and auto-clears after 2s so the
  // inline "Copied!" pill fades without needing extra render-state plumbing.
  // copyError holds any plugin error (rare — clipboard permission denied,
  // unsupported target) and is cleared on the next attempt or modal close.
  let copyInFlight = $state(false);
  let copyConfirmation = $state(false);
  let copyError = $state('');
  let copyResetTimer: ReturnType<typeof setTimeout> | null = null;

  let weeklyFileUnlisten: UnlistenFn | null = null;
  let settingsUnlisten: UnlistenFn | null = null;

  // ----- Derived -----

  const hasManagerEmail = $derived((managerEmail ?? '').trim() !== '');

  const alreadySentUnchanged = $derived(
    sentRecord !== null &&
      currentHash !== '' &&
      sentRecord.contentHash === currentHash
  );

  const editedAfterSend = $derived(
    sentRecord !== null &&
      currentHash !== '' &&
      sentRecord.contentHash !== currentHash
  );

  const sendDisabled = $derived(
    isDirty || saveStatus === 'saving' || alreadySentUnchanged || isSending
  );

  const sendButtonLabel = $derived.by(() => {
    if (isSending) return 'Opening mail app…';
    if (editedAfterSend) return 'Send updated version';
    return 'Send to manager';
  });

  const sendTooltip = $derived.by(() => {
    if (isDirty) return 'Save your changes first.';
    if (saveStatus === 'saving') return 'Waiting for save to finish…';
    if (alreadySentUnchanged && sentRecord) {
      return `Sent ${sentRecord.sentTo} on ${formatSentAt(sentRecord.sentAt)}.`;
    }
    if (isSending) return 'Opening your mail app…';
    if (hasManagerEmail) {
      return `Opens a draft addressed to ${managerEmail} in your default mail app.`;
    }
    return 'Opens a blank draft in your mail app — type a recipient there. Set a manager email in Settings to pre-fill it.';
  });

  // One-line tip strip above the modal actions. Reminds the user what the
  // active send mode is about to do — kept terse so it reads at a glance.
  // In clipboard-paste mode the copy shifts to reflect the two-step flow
  // (compose opens empty, paste the formatted body with Cmd+V).
  const modeTip = $derived.by(() => {
    if (mailBodyDelivery === 'clipboard-paste') {
      const client =
        mailSendMode === 'gmail'
          ? 'Gmail'
          : mailSendMode === 'outlook'
            ? 'Outlook'
            : 'Mac Mail';
      return `Opens ${client} with an empty body and copies the formatted message. Press Cmd+V in the draft, then Send.`;
    }
    if (mailSendMode === 'gmail') {
      return "Opens Gmail in your browser. Sign in first if you haven't.";
    }
    if (mailSendMode === 'native-mail') {
      return 'First-time use will ask macOS for permission to control Mail.';
    }
    return 'Opens outlook.office.com (or .live.com). Plaintext only.';
  });

  // The confirm modal's primary action button label. Flips to "Copy +
  // Open <Client>" in clipboard-paste mode so the user knows the click
  // does two things atomically.
  const confirmButtonLabel = $derived.by(() => {
    if (isSending) return 'Opening…';
    if (mailBodyDelivery === 'clipboard-paste') {
      const client =
        mailSendMode === 'gmail'
          ? 'Gmail'
          : mailSendMode === 'outlook'
            ? 'Outlook'
            : 'Mac Mail';
      return `Copy + Open ${client}`;
    }
    return 'Open draft';
  });

  // Preview renders the rich HTML in a sandboxed iframe when the active
  // send path will actually deliver HTML to the recipient. Two paths:
  //   1. Native Mac Mail with the HTML toggle on — the .eml multipart
  //      body itself carries the styled HTML.
  //   2. Body delivery = Compose + paste — the user pastes rich HTML
  //      from the clipboard into the compose window; the rendered
  //      result is what the recipient gets.
  // Prefilled-plaintext modes (Gmail/Outlook URL prefill, Native Mac
  // plaintext) ship the body as text and would mislead the user with
  // a styled preview — those still get the <pre> plaintext fallback.
  const previewShowsHtml = $derived(
    (mailSendMode === 'native-mail' && mailNativeHtml) ||
      mailBodyDelivery === 'clipboard-paste'
  );

  const computedSentStatusText = $derived.by(() => {
    if (!sentRecord) return '';
    if (editedAfterSend) {
      return `Last sent ${formatSentAt(sentRecord.sentAt)} (edited since)`;
    }
    return `Sent ${formatSentAt(sentRecord.sentAt)}`;
  });

  // Push the derived text + stale flag out to the bindable props so the
  // parent route can render the status line wherever its layout calls
  // for (typically above the actions row, right-justified).
  $effect(() => {
    sentStatusText = computedSentStatusText;
    sentStatusIsStale = editedAfterSend;
  });

  // ----- Helpers -----

  function formatSentAt(rfc3339: string): string {
    const d = new Date(rfc3339);
    if (Number.isNaN(d.getTime())) return rfc3339;
    return d.toLocaleString('en-US', {
      month: 'short',
      day: 'numeric',
      hour: 'numeric',
      minute: '2-digit'
    });
  }

  function inlineLabel(label: string): string {
    if (label.length === 0) return label;
    return label.charAt(0).toLowerCase() + label.slice(1);
  }

  /// Pull the sent-log entry + content hash for the current (year, week).
  /// Called on mount, on year/week change, and after weekly-file-changed.
  async function refreshState(): Promise<void> {
    try {
      const [settings, record, hash] = await Promise.all([
        invoke<{
          managerEmail: string | null;
          userEmail: string | null;
          mailSendMode: 'gmail' | 'native-mail' | 'outlook';
          mailNativeHtml: boolean;
          mailBodyDelivery: 'prefilled' | 'clipboard-paste';
        }>('get_settings'),
        invoke<SentRecord | null>('get_sent_record', { year, week }),
        invoke<string>('get_summary_hash', { year, week })
      ]);
      managerEmail = settings.managerEmail;
      userEmail = settings.userEmail;
      mailSendMode = settings.mailSendMode;
      mailNativeHtml = settings.mailNativeHtml;
      mailBodyDelivery = settings.mailBodyDelivery;
      sentRecord = record;
      currentHash = hash;
    } catch (err) {
      // Surface as sendError so the user has SOME signal; the button
      // stays available though gates may be wrong.
      console.error('[send-to-manager] failed to refresh state:', err);
    }
  }

  // Re-fetch whenever the (year, week) the parent points us at changes.
  // Also drop any open preview — the rendered HTML/text was for the
  // previous week, so showing the Send button as a path to "the draft you
  // just saw" would be a lie. The token bump cancels an in-flight
  // openPreview() invoke too.
  $effect(() => {
    // Track year/week only — untrack the body so reads of showPreview /
    // previewPayload don't add them as dependencies (which would cause the
    // effect to re-fire when openPreview sets showPreview = true and
    // immediately close the just-opened preview).
    year;
    week;
    untrack(() => {
      void refreshState();
      if (showPreview || previewPayload) {
        previewToken += 1;
        showPreview = false;
        previewPayload = null;
        previewStaleWarning = 'Week changed — reopen preview to see latest.';
      }
    });
  });

  onMount(async () => {
    // Initial load happens via the $effect above. Set up the
    // weekly-file-changed listener so saves on OTHER routes (or
    // /capture notes) refresh our gating state.
    weeklyFileUnlisten = await listen<{ year: number; week: number }>(
      'weekly-file-changed',
      (event) => {
        if (event.payload.year === year && event.payload.week === week) {
          void refreshState();
          // Close any open preview and surface a stale warning on the
          // outer confirm modal — the rendered HTML/text we're showing
          // is now older than disk. Bump the token so an in-flight
          // openPreview() invoke from before the change drops its
          // response on return.
          if (showPreview || previewPayload) {
            previewToken += 1;
            showPreview = false;
            previewPayload = null;
            previewStaleWarning =
              'Content changed since preview — reopen to see latest.';
          }
        }
      }
    );

    // Settings edits (manager email or name) outside this component must
    // also invalidate cached state — otherwise the modal would address
    // the draft to a stale email until the route remounted. The Rust
    // settings commands emit "settings-changed" after a successful write
    // (verified in commands.rs).
    settingsUnlisten = await listen('settings-changed', () => {
      void refreshState();
      if (showPreview || previewPayload) {
        previewToken += 1;
        showPreview = false;
        previewPayload = null;
        previewStaleWarning =
          'Settings changed — reopen preview to see latest.';
      }
    });

    // Escape is owned by the shared <Modal> component's topmost-stack
    // listener — both Confirm and Preview run through Modal, and Preview's
    // onClose is closePreview() which already bumps previewToken. No local
    // window-level Escape handler needed.
  });

  onDestroy(() => {
    weeklyFileUnlisten?.();
    weeklyFileUnlisten = null;
    settingsUnlisten?.();
    settingsUnlisten = null;
    if (copyResetTimer) {
      clearTimeout(copyResetTimer);
      copyResetTimer = null;
    }
  });

  // ----- Send flow -----

  function onSendClick(): void {
    if (sendDisabled) return;
    // Clear any stale preview from a previous Send click — without this,
    // a second click would re-pop the prior preview (or its error) under
    // the confirm modal. Bumping the token also cancels any in-flight
    // openPreview() invoke from the previous round.
    previewToken += 1;
    showPreview = false;
    previewPayload = null;
    previewError = null;
    previewStaleWarning = '';
    previewLoading = false;
    sendError = '';
    pendingTruncatedUrl = null;
    truncationWarningText = '';
    showConfirmModal = true;
  }

  function dismissConfirm(): void {
    if (isSending) return;
    showConfirmModal = false;
    // Same reset as onSendClick — if the user cancels mid-flight on the
    // preview load, we don't want the awaited response to flip showPreview
    // back to true after the confirm modal is already gone.
    previewToken += 1;
    showPreview = false;
    previewPayload = null;
    previewError = null;
    previewStaleWarning = '';
    previewLoading = false;
    pendingTruncatedUrl = null;
    truncationWarningText = '';
    clipboardPasteError = '';
  }

  async function openPreview(): Promise<void> {
    if (previewLoading) return;
    // Capture the token at entry; if anything bumps it while we're
    // awaiting the invoke, our response is stale and should be dropped.
    previewToken += 1;
    const myToken = previewToken;
    previewLoading = true;
    previewError = null;
    previewStaleWarning = '';
    try {
      const payload = await invoke<PreviewPayload>(
        'render_weekly_summary_preview',
        { year, week }
      );
      if (previewToken !== myToken) return;
      previewPayload = payload;
      showPreview = true;
    } catch (err) {
      if (previewToken !== myToken) return;
      previewError = String(err);
    } finally {
      if (previewToken === myToken) previewLoading = false;
    }
  }

  function closePreview(): void {
    previewToken += 1;
    showPreview = false;
    previewPayload = null;
    // Drop any lingering copy state so reopening the preview starts clean.
    // The 2s confirmation timer is cleared explicitly because a fast user
    // could close + reopen before it fires and otherwise see a stale pill.
    if (copyResetTimer) {
      clearTimeout(copyResetTimer);
      copyResetTimer = null;
    }
    copyConfirmation = false;
    copyError = '';
  }

  /// Copy the current preview to the clipboard. Always writes BOTH the HTML
  /// rendition AND a plaintext fallback (writeHtml takes both) regardless of
  /// the active send mode. Rich-paste targets (Gmail web compose, Outlook
  /// web compose, any contenteditable) honor the HTML — so a user in Gmail
  /// mode can Preview → Copy → Cmd+V in Gmail's body to get formatted text
  /// (3 clicks total). Plaintext targets (Slack, Terminal) automatically
  /// pick the plaintext flavor — the OS pasteboard handles the negotiation.
  /// The backend's render_weekly_summary_preview always populates both
  /// previewPayload.html and previewPayload.text, so this branch was just
  /// gating off rich content in the modes that wanted it most.
  async function copyPreviewToClipboard(): Promise<void> {
    if (!previewPayload || copyInFlight) return;
    copyInFlight = true;
    copyError = '';
    copyConfirmation = false;
    if (copyResetTimer) {
      clearTimeout(copyResetTimer);
      copyResetTimer = null;
    }
    try {
      await writeHtml(previewPayload.html, previewPayload.text);
      copyConfirmation = true;
      copyResetTimer = setTimeout(() => {
        copyConfirmation = false;
        copyResetTimer = null;
      }, 2000);
    } catch (err) {
      copyError = String(err);
    } finally {
      copyInFlight = false;
    }
  }

  /// After a successful hand-off, stamp the sent-log and close the modal.
  /// Split out of confirmSend() so both the WebUrl truncation-warning path
  /// (open immediately) and the "Send anyway" path can share the
  /// post-success bookkeeping.
  async function finishSend(): Promise<void> {
    const record = await invoke<SentRecord>('mark_weekly_summary_sent', {
      year,
      week,
      contentHash: currentHash,
      sentTo: (managerEmail ?? '').trim()
    });
    sentRecord = record;
    showConfirmModal = false;
    pendingTruncatedUrl = null;
    truncationWarningText = '';
  }

  /// User confirmed "Send anyway" on a Gmail truncation warning. Open the
  /// already-built URL and run the standard post-send bookkeeping.
  async function sendTruncatedAnyway(): Promise<void> {
    if (!pendingTruncatedUrl) return;
    isSending = true;
    sendError = '';
    try {
      await openUrl(pendingTruncatedUrl);
      await finishSend();
    } catch (err) {
      sendError = String(err);
    } finally {
      isSending = false;
    }
  }

  async function confirmSend(): Promise<void> {
    isSending = true;
    sendError = '';
    pendingTruncatedUrl = null;
    truncationWarningText = '';
    clipboardPasteError = '';
    try {
      // Clipboard-paste delivery: fetch the rendered HTML+text, write
      // BOTH flavors to the clipboard, THEN open the (empty-body)
      // compose. Sequential, not parallel — writeHtml must land on the
      // pasteboard before the compose window steals focus, and if it
      // throws we must NOT open the compose (silent empty draft would
      // let the user send an empty email). The backend reads
      // mail_body_delivery from settings and emits an empty-body URL/
      // AppleScript when set, so the user's Cmd+V doesn't collide with
      // prefilled text.
      //
      // The Native Mac '.eml' HTML toggle is a peer override on the
      // backend — when both `clipboard-paste` and `native_html` are
      // set AND mailSendMode is 'native-mail', the .eml path wins
      // (recipient gets a styled body without needing the paste step).
      // The backend's check is `mode == NativeMail && native_html` —
      // the gate here intentionally does NOT mirror that. We populate
      // the clipboard whenever `clipboard-paste` is on, regardless of
      // mailNativeHtml's value. A previous version gated on
      // `!mailNativeHtml` which had a stale-state bug: a user who
      // toggled Native HTML on under Native Mac mode, then switched
      // to Gmail without turning it off (the toggle isn't visible
      // from Gmail), would skip the clipboard write entirely on the
      // Gmail path. The backend would correctly emit an empty-body
      // Gmail URL, but the clipboard would never get populated —
      // silent failure on Cmd+V. Letting the clipboard always populate
      // means at worst we waste a write in the rare Native-Mac+
      // .eml+clipboard-paste combo (where the .eml path renders the
      // body itself and the user never needs to paste); harmless.
      if (mailBodyDelivery === 'clipboard-paste') {
        try {
          const preview = await invoke<PreviewPayload>(
            'render_weekly_summary_preview',
            { year, week }
          );
          await writeHtml(preview.html, preview.text);
        } catch (copyErr) {
          clipboardPasteError = String(copyErr);
          isSending = false;
          return;
        }
      }
      // `format: 'html'` is belt-and-suspenders — the backend defaults to
      // html when the arg is missing — but being explicit here makes the
      // contract obvious at the call site. mailSendMode + userEmail come
      // from the settings snapshot loaded on mount; the backend uses them
      // to pick the per-mode builder (Gmail web URL, Outlook web URL,
      // AppleScript for Mac Mail).
      const result = await invoke<ComposeResult>('compose_weekly_email', {
        year,
        week,
        format: 'html',
        mailSendMode,
        userEmail: userEmail ?? null
      });
      // Dispatch on the variant.
      if (result.kind === 'mailto') {
        await openUrl(result.value);
        await finishSend();
      } else if (result.kind === 'eml') {
        await openPath(result.value);
        await finishSend();
      } else if (result.kind === 'webUrl') {
        if (result.truncationWarning) {
          // Hold the URL aside and replace the Send button with a
          // Send-anyway / Cancel pair. The user-approved flow is
          // warn-and-allow, not block.
          pendingTruncatedUrl = result.url;
          truncationWarningText =
            `Your draft is ${result.url.length} characters — Gmail truncates compose URLs above about 2,000. ` +
            `Send anyway, or switch the Mail mode in Settings to use a path without this cap.`;
        } else {
          await openUrl(result.url);
          await finishSend();
        }
      } else {
        // appleScript — pipe the script into osascript via the Tauri
        // command and either close the modal on success or surface the
        // Apple Events permission error with a deep-link to System
        // Settings.
        try {
          await invoke('run_applescript', { script: result.script });
          await finishSend();
        } catch (err) {
          const msg = String(err);
          if (msg.startsWith('permission_denied:')) {
            sendError = 'permission_denied';
          } else {
            sendError = msg;
          }
        }
      }
    } catch (err) {
      sendError = String(err);
    } finally {
      isSending = false;
    }
  }

  /// Deep-link to the Privacy > Automation pane in System Settings so the
  /// user can grant Captain's Log permission to control Mail.app. macOS
  /// resolves the `x-apple.systempreferences:` scheme without a confirm
  /// dialog, so this lands directly on the Automation list.
  async function openAutomationSettings(): Promise<void> {
    try {
      await openUrl(
        'x-apple.systempreferences:com.apple.preference.security?Privacy_Automation'
      );
    } catch (err) {
      console.error('[send-to-manager] failed to open Automation settings:', err);
    }
  }
</script>

<button
  class="btn btn-marble btn-send"
  onclick={onSendClick}
  disabled={sendDisabled}
  title={sendTooltip}
>
  {sendButtonLabel}
</button>

{#if showConfirmModal}
  <!-- Confirm shell is the shared $lib/Modal — backdrop dim+blur, body-
       scroll lock, topmost-Escape stack, focus management. dismissConfirm
       guards against `isSending` internally so backdrop click + Escape are
       both safe no-ops while the send is in flight. -->
  <Modal
    open={true}
    onClose={dismissConfirm}
    title="Send weekly summary?"
    maxWidth="min(520px, calc(100vw - 32px))"
  >
    <div class="send-confirm-body">
      <p>
        {#if hasManagerEmail}
          This will open your default mail app with a draft addressed to
          <strong>{managerEmail}</strong> for the {inlineLabel(weekLabel)}.
        {:else}
          This will open your default mail app with a draft for the
          {inlineLabel(weekLabel)}. The To: field will be blank — type or
          pick a recipient there.
        {/if}
        You'll review and send it from there — Captain's Log never sends mail on
        its own.
      </p>
      {#if editedAfterSend && sentRecord}
        <TipBubble>
          You previously sent this week on {formatSentAt(sentRecord.sentAt)}.
          This opens a new draft with the updated content.
        </TipBubble>
      {/if}
      {#if previewStaleWarning}
        <TipBubble>{previewStaleWarning}</TipBubble>
      {/if}
      {#if previewError}
        <p class="status status-error">Couldn't render preview: {previewError}</p>
      {/if}
      {#if truncationWarningText}
        <TipBubble>{truncationWarningText}</TipBubble>
      {/if}
      {#if sendError === 'permission_denied'}
        <p class="status status-error">
          macOS hasn't authorized Captain's Log to control Mail.
          <button
            type="button"
            class="link-button"
            onclick={() => void openAutomationSettings()}
          >Open Automation Settings →</button>
        </p>
      {:else if sendError}
        <p class="status status-error">Couldn't open mail app: {sendError}</p>
      {/if}
      {#if clipboardPasteError}
        <!-- Compose+paste failure: writeHtml threw, so we never opened
             the compose. Offer Preview as a recovery path — the existing
             Preview modal also has a Copy button the user can fall back
             to manually. -->
        <p class="status status-error">
          Couldn't copy formatted body to clipboard: {clipboardPasteError}.
          <button
            type="button"
            class="link-button"
            onclick={() => void openPreview()}
          >Open Preview →</button>
        </p>
      {/if}
      <TipBubble>{modeTip}</TipBubble>
      <div class="modal-actions">
        {#if pendingTruncatedUrl}
          <!-- Warn-and-allow: the encoded URL crossed Gmail's silent
               truncation threshold. Send anyway opens the original URL
               (Gmail will cut the body); Cancel returns to normal state
               so the user can shorten before retrying. -->
          <button
            class="btn btn-emerald"
            onclick={() => void sendTruncatedAnyway()}
            disabled={isSending}
          >
            {isSending ? 'Opening…' : 'Send anyway'}
          </button>
          <button class="btn btn-ruby" onclick={dismissConfirm} disabled={isSending}>
            Cancel
          </button>
        {:else}
          <button
            class="btn btn-emerald"
            onclick={() => void confirmSend()}
            disabled={isSending}
          >
            {confirmButtonLabel}
          </button>
          <button class="btn btn-ruby" onclick={dismissConfirm} disabled={isSending}>
            Cancel
          </button>
        {/if}
        <button
          class="btn btn-marble"
          onclick={() => void openPreview()}
          disabled={isSending || previewLoading}
        >
          {previewLoading ? 'Loading…' : 'Preview'}
        </button>
      </div>
    </div>
  </Modal>

  {#if showPreview && previewPayload}
    <!-- Preview uses the shared Modal shell — dim+blur backdrop, body-
         scroll lock, topmost-Escape, standard header bar with the
         email subject as title. zLayer="nested" so it stacks cleanly
         above the parent confirm dialog. -->
    <Modal
      open={true}
      onClose={closePreview}
      title={previewPayload.subject}
      zLayer="nested"
      maxWidth="min(640px, calc(100vw - 32px))"
    >
      {#if userEmail}
        <p class="preview-header-line">
          From: <strong>{userEmail}</strong>
        </p>
      {/if}
      <p class="preview-header-line">
        {#if previewPayload.recipient}
          To: <strong>{previewPayload.recipient}</strong>
        {:else}
          No recipient — you'll add one in Mail.
        {/if}
      </p>
      {#if previewShowsHtml}
        <iframe
          class="preview-iframe"
          srcdoc={previewPayload.html}
          sandbox=""
          title="Weekly summary preview"
        ></iframe>
      {:else}
        <pre class="preview-plaintext">{previewPayload.text}</pre>
        <p class="preview-note">Recipient will see this as plain text.</p>
      {/if}
      <div class="modal-actions preview-actions">
        <!-- Status pill is rendered first so margin-right: auto on it pushes
             Close + Copy together against the right edge, matching the
             button placement used by every other modal in the app. When
             the pill isn't present the actions row inherits .modal-actions'
             flex-end and Close+Copy still hug the right. -->
        {#if copyConfirmation}
          <span class="copy-status copy-confirmation" role="status" aria-live="polite">
            Copied!
          </span>
        {:else if copyError}
          <span class="copy-status copy-error" role="alert">
            Couldn't copy: {copyError}
          </span>
        {/if}
        <button class="btn btn-ruby" onclick={closePreview}>
          Close
        </button>
        <button
          class="btn btn-marble"
          onclick={() => void copyPreviewToClipboard()}
          disabled={copyInFlight}
        >
          {copyInFlight ? 'Copying…' : 'Copy To Clipboard'}
        </button>
      </div>
    </Modal>
  {/if}
{/if}

<style>
  /* .btn-send disabled styling handled by the shared .btn:disabled
     rule in app.css (opacity-based dim). Local override no longer
     needed — was previously the only place we used opacity for the
     dim, now it's the app-wide default. */

  /* .sent-status text styling lives in app.css as a shared utility —
     the parent route renders the <p> element above the actions row. */

  /* Confirm-modal chrome (backdrop dim+blur, centered card, header bar,
     body-scroll lock, Escape stack) is owned by $lib/Modal.svelte. Only
     the body-content rules live here. */
  .send-confirm-body p {
    margin: 0 0 var(--space-3);
    color: var(--text-secondary);
    line-height: var(--text-body-lh);
  }

  .send-confirm-body p :global(strong) {
    color: var(--text-primary);
    font-weight: normal;
    font-family: var(--font-display);
  }

  .modal-actions {
    display: flex;
    gap: var(--space-3);
    justify-content: flex-end;
    margin-top: var(--space-6);
  }

  /* Preview chrome (backdrop, modal card, z-layering) is owned by
     $lib/Modal.svelte now. Only the body-content rules live here. */
  .preview-header-line {
    margin: 0 0 var(--space-2);
    color: var(--text-secondary);
  }
  .preview-iframe {
    width: 100%;
    height: 60vh;
    border: 1px solid var(--border-structural);
    border-radius: var(--radius-md);
    background: white;
    display: block;
  }

  /* Plaintext preview — same vertical footprint as the iframe so the
     modal doesn't jump between modes. Scrolls inside its own box so
     long bodies never blow out the dialog width. */
  .preview-plaintext {
    width: 100%;
    height: 60vh;
    margin: 0;
    padding: var(--space-3);
    border: 1px solid var(--border-structural);
    border-radius: var(--radius-md);
    background: var(--bg-elevated);
    color: var(--text-primary);
    font-family: var(--font-mono, ui-monospace, SFMono-Regular, Menlo, monospace);
    font-size: 0.85rem;
    line-height: 1.5;
    overflow: auto;
    white-space: pre-wrap;
    word-break: break-word;
    box-sizing: border-box;
  }
  .preview-note {
    margin-top: var(--space-2) !important;
    font-size: 0.8rem;
    color: var(--text-muted, var(--text-secondary)) !important;
  }

  /* Mode-specific tip uses the shared TipBubble component
     ($lib/onboarding/TipBubble.svelte) — same bordered callout the rest
     of the app uses for in-context guidance. */

  /* .status + .status-error live in app.css as a shared utility. */

  /* Inline button that reads as a link inside an error message — used for
     the "Open Automation Settings" deep-link on permission denial. Inherits
     the surrounding paragraph's text color so it stays AA-contrast on
     bg-error-tint in both themes. */
  .link-button {
    display: inline;
    background: none;
    border: none;
    padding: 0;
    margin: 0 0 0 var(--space-2);
    font: inherit;
    color: inherit;
    text-decoration: underline;
    text-underline-offset: 2px;
    cursor: pointer;
  }
  .link-button:hover,
  .link-button:focus-visible {
    text-decoration: none;
    outline: none;
  }

  /* Preview modal actions row — inherits .modal-actions' flex-end so the
     buttons hug the lower-right corner like every other modal. When the
     copy-status pill is rendered it claims the leftover space via
     margin-right: auto so Close + Copy stay glued together on the right. */
  .preview-actions {
    align-items: center;
  }
  .preview-actions .copy-status {
    margin-right: auto;
  }

  /* Inline copy confirmation / error pill. Pill shape and small footprint
     keep it visually distinct from the surrounding buttons so it doesn't
     read as a third action. Both states use tokens for AA-contrast in
     light + dark themes. */
  .copy-status {
    font-size: 0.8rem;
    padding: 2px 8px;
    border-radius: var(--radius-sm, 4px);
    line-height: 1.4;
    white-space: nowrap;
  }
  .copy-confirmation {
    background: var(--bg-elevated);
    color: var(--text-primary);
    border: 1px solid var(--border-structural);
  }
  .copy-error {
    background: var(--bg-error-tint);
    color: var(--text-primary);
    border: 1px solid var(--border-error);
    white-space: normal;
  }
</style>
