<!--
  Phase 5 — Prep Self Review wizard.
  =====================================================================

  A Modal-hosted, five-step wizard that assembles a comprehensive
  markdown "review preparation" doc from the user's journal + a small
  amount of collected context (review period, questions, OKRs). The
  doc is intended to be handed off to an LLM (Claude or otherwise)
  with the instruction "look at this and do what it says."

  ## Design pillars

  - **Cancel anywhere.** Modal backdrop / Escape / × / explicit Cancel
    button all attempt to close. If the user has entered any data
    (dirty = true), a ConfirmDialog fires first ("Discard your
    progress?"). Once the user has generated the doc, close is
    unblocked — they've finished the flow, cancel semantics no
    longer apply.

  - **No partial persistence.** Wizard state lives on this component's
    lifetime; when the modal closes, everything drops. If the user
    edited any of the personal-info fields, THOSE go back into
    settings.json on Generate (the wizard is a legitimate place to
    fix a stale name/email/manager), but questions / OKRs / date
    range are per-run and never persisted.

  - **Missing data is fine.** Every input except the review period
    dates is optional. The generator's assembly path gates each
    section on presence; the wizard's final step surfaces a "less
    useful without these" warning so the user knows what they're
    giving up.

  - **Captain's Log doesn't fetch anything.** Linked docs (Google Docs,
    Confluence pages, Jira, whatever) pass through as text. The
    generated doc's instructions tell the LLM to fetch via ITS
    connectors — a clean split between "assemble the source material
    at generate time" and "reason over it at consume time."

  ## Component shape

  Steps are inlined as {#if step === N} blocks rather than sub-
  components — each step is compact enough that per-file splitting
  would be more overhead than help. Follows the flat-$state pattern
  onboarding uses, so state survives Back/Continue without manual
  marshaling.

  Landing-page owns visibility via the `open` prop; on close, the
  wizard drops its state entirely (see `reset()`).
-->
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { save as saveDialog } from '@tauri-apps/plugin-dialog';
  import { writeTextFile } from '@tauri-apps/plugin-fs';
  import { writeText as clipboardWriteText } from '@tauri-apps/plugin-clipboard-manager';
  import { desktopDir } from '@tauri-apps/api/path';

  import Modal from '$lib/Modal.svelte';
  import ConfirmDialog from '$lib/ConfirmDialog.svelte';
  import InputField from '$lib/InputField.svelte';
  import TextAreaField from '$lib/TextAreaField.svelte';
  import Checkbox from '$lib/Checkbox.svelte';
  import StepHeader from '$lib/onboarding/StepHeader.svelte';
  import TipBubble from '$lib/onboarding/TipBubble.svelte';
  import DatePickerPopover from '$lib/DatePickerPopover.svelte';

  // Payload shape mirrors backend `ReviewPrepInput` (Rust
  // src-tauri/src/review_prep.rs). Keep in lockstep with the struct's
  // #[serde(rename_all = "camelCase")] annotation.
  type ReviewPrepInput = {
    userName: string | null;
    userEmail: string | null;
    jobTitle: string | null;
    managerName: string | null;
    managerEmail: string | null;
    jiraProjectKeys: string[];
    startDate: string;
    endDate: string;
    reviewQuestions: string | null;
    okrs: string | null;
    includeNotes: boolean;
    todayIso: string;
  };

  // Minimal shape of the Settings payload we need to preserve when
  // pushing wizard edits back via update_settings. update_settings is
  // an all-or-nothing writer — anything omitted resets to defaults —
  // so we fetch the current settings before submitting and merge our
  // handful of edited fields in.
  type LoadedSettings = {
    userName: string | null;
    userEmail: string | null;
    bambooTitle: string | null;
    jiraProjectKeys: string[];
    managerName: string | null;
    managerEmail: string | null;
    journalRoot: string;
    reminder: unknown;
    theme: string;
    customTheme: unknown;
    mailSendMode: string;
    mailBodyFormat: string;
    mailNativeHtml: boolean;
    mailOutlookFlavor: string;
    mailBodyDelivery: string;
    colorfulLabels: boolean;
    taskList: unknown;
    taskReminder: unknown;
    hideSendToManager: boolean;
  };

  let {
    open,
    onClose,
    onSettingsChanged,
  }: {
    open: boolean;
    onClose: () => void;
    onSettingsChanged?: () => void;
  } = $props();

  const TOTAL_STEPS = 5;
  let step = $state(1);

  // Wizard state. Flat $state fields (onboarding pattern) so each
  // step's UI can bind directly. Every field is nullable/optional
  // downstream except the date range.
  let userName = $state('');
  let userEmail = $state('');
  let jobTitle = $state('');
  let jiraKeys = $state(''); // Comma/space-separated in the UI; parsed at Generate.
  let managerName = $state('');
  let managerEmail = $state('');
  let startDate = $state('');
  let endDate = $state('');
  let reviewQuestions = $state('');
  let okrs = $state('');
  let includeNotes = $state(false);

  // DatePickerPopover state (step 2). Two independent anchors — one
  // per date button. `pickerOpen` names which popover is open, or
  // null when neither is. `bind:this` on the buttons keeps the anchor
  // elements live so the popover's positioning + outside-click
  // detection can attach to them.
  let startAnchorEl = $state<HTMLButtonElement | null>(null);
  let endAnchorEl = $state<HTMLButtonElement | null>(null);
  let pickerOpen = $state<'start' | 'end' | null>(null);

  function todayIsoLocal(): string {
    // Local YYYY-MM-DD, mirroring how the picker itself interprets
    // strings (calendar date, not UTC midnight). Avoids the classic
    // Date().toISOString() timezone-shift trap that would render
    // "yesterday" for late-evening users.
    const d = new Date();
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
  }

  // Loaded settings snapshot — kept so the update_settings payload
  // can preserve non-wizard fields (theme, mail, tasks, reminders,
  // ...) when we push personal-info edits back.
  let loadedSettings = $state<LoadedSettings | null>(null);
  let loadError = $state('');
  let loading = $state(false);

  // Dirty tracking. `$derived` compares live wizard state against the
  // loaded settings + the initial "empty" state of the per-run fields
  // — flipping true the moment the user edits anything meaningful.
  // Used by attemptClose to decide whether to prompt "Discard your
  // progress?". No `markDirty()` sprinkles across handlers — the
  // derived does the accounting.
  const dirty = $derived.by(() => {
    if (loading || !loadedSettings) return false;
    const initialJiraKeys = (loadedSettings.jiraProjectKeys ?? []).join(', ');
    return (
      userName !== (loadedSettings.userName ?? '')
      || userEmail !== (loadedSettings.userEmail ?? '')
      || jobTitle !== (loadedSettings.bambooTitle ?? '')
      || jiraKeys !== initialJiraKeys
      || managerName !== (loadedSettings.managerName ?? '')
      || managerEmail !== (loadedSettings.managerEmail ?? '')
      || startDate !== ''
      || endDate !== ''
      || reviewQuestions !== ''
      || okrs !== ''
      || includeNotes !== false
    );
  });

  // Post-Generate state.
  let generatedDoc = $state('');
  let generating = $state(false);
  let generateError = $state('');
  let saveMessage = $state('');
  let saveIsError = $state(false);

  // Cancel-confirmation state.
  let confirmingCancel = $state(false);

  // Initial-load pass — fetch settings when the modal opens so pre-
  // fill is fresh. Ignore an already-open re-run.
  let previousOpen = false;
  $effect(() => {
    if (open && !previousOpen) {
      previousOpen = true;
      resetAndLoad();
    } else if (!open && previousOpen) {
      previousOpen = false;
    }
  });

  async function resetAndLoad() {
    step = 1;
    generatedDoc = '';
    generating = false;
    generateError = '';
    saveMessage = '';
    saveIsError = false;
    confirmingCancel = false;
    loadError = '';
    loading = true;
    try {
      const settings = await invoke<LoadedSettings>('get_settings');
      loadedSettings = settings;
      userName = settings.userName ?? '';
      userEmail = settings.userEmail ?? '';
      jobTitle = settings.bambooTitle ?? '';
      jiraKeys = (settings.jiraProjectKeys ?? []).join(', ');
      managerName = settings.managerName ?? '';
      managerEmail = settings.managerEmail ?? '';
      // Review period + prose fields always start empty — they're
      // per-run, no persistence.
      startDate = '';
      endDate = '';
      reviewQuestions = '';
      okrs = '';
      includeNotes = false;
    } catch (e) {
      loadError = `Couldn't load your settings: ${String(e)}. You can still fill out the form manually.`;
    } finally {
      loading = false;
    }
  }

  function attemptClose() {
    // If the user has already generated the doc, close is fine — the
    // flow is essentially done. Otherwise gate on the dirty flag.
    if (generatedDoc || !dirty) {
      onClose();
      return;
    }
    confirmingCancel = true;
  }

  function confirmCancel() {
    confirmingCancel = false;
    onClose();
  }

  function abortCancel() {
    confirmingCancel = false;
  }

  function back() {
    if (step > 1) step -= 1;
  }
  function next() {
    if (step < TOTAL_STEPS) step += 1;
  }

  // Parse comma / whitespace-separated Jira keys into a normalized
  // uppercased array. Matches how Settings tab handles it (backend
  // also uppercases + dedupes server-side).
  function parseJiraKeys(raw: string): string[] {
    const tokens = raw
      .split(/[\s,]+/)
      .map((t) => t.trim().toUpperCase())
      .filter(Boolean);
    return Array.from(new Set(tokens));
  }

  // Return true when any of the six personal-info fields differ from
  // the loaded settings snapshot. Used to decide whether we need to
  // round-trip update_settings on Generate.
  function personalInfoChanged(): boolean {
    if (!loadedSettings) return false;
    const parsedKeys = parseJiraKeys(jiraKeys);
    const settingsKeys = loadedSettings.jiraProjectKeys ?? [];
    const keysDiffer =
      parsedKeys.length !== settingsKeys.length
      || parsedKeys.some((k, i) => k !== settingsKeys[i]);
    return (
      (userName.trim() || null) !== (loadedSettings.userName ?? null)
      || (userEmail.trim() || null) !== (loadedSettings.userEmail ?? null)
      || (jobTitle.trim() || null) !== (loadedSettings.bambooTitle ?? null)
      || keysDiffer
      || (managerName.trim() || null) !== (loadedSettings.managerName ?? null)
      || (managerEmail.trim() || null) !== (loadedSettings.managerEmail ?? null)
    );
  }

  async function pushSettingsEdits() {
    if (!loadedSettings || !personalInfoChanged()) return;
    // Merge wizard edits over the loaded snapshot. Non-wizard fields
    // (theme, mail, tasks, reminders, journalRoot) pass through
    // untouched. update_settings expects the full payload.
    await invoke('update_settings', {
      input: {
        userName: userName.trim() || null,
        userEmail: userEmail.trim() || null,
        journalRoot: loadedSettings.journalRoot,
        reminder: loadedSettings.reminder,
        theme: loadedSettings.theme,
        customTheme: loadedSettings.customTheme,
        managerEmail: managerEmail.trim() || null,
        managerName: managerName.trim() || null,
        bambooTitle: jobTitle.trim() || null,
        jiraProjectKeys: parseJiraKeys(jiraKeys),
        mailSendMode: loadedSettings.mailSendMode,
        mailBodyFormat: loadedSettings.mailBodyFormat,
        mailNativeHtml: loadedSettings.mailNativeHtml,
        mailOutlookFlavor: loadedSettings.mailOutlookFlavor,
        mailBodyDelivery: loadedSettings.mailBodyDelivery,
        colorfulLabels: loadedSettings.colorfulLabels,
        taskList: loadedSettings.taskList,
        taskReminder: loadedSettings.taskReminder,
        hideSendToManager: loadedSettings.hideSendToManager,
      },
    });
    onSettingsChanged?.();
  }

  async function generate() {
    generateError = '';
    saveMessage = '';
    saveIsError = false;
    generating = true;
    try {
      // Push personal-info edits back to settings.json first so the
      // change survives across the wizard's lifetime. If this fails
      // the user gets an error and can retry — we don't proceed to
      // generation on a settings write failure (the wizard has just
      // told them their edits went somewhere; better to surface the
      // mismatch than silently drop it).
      await pushSettingsEdits();

      const todayIso = new Date().toISOString().slice(0, 10);
      const input: ReviewPrepInput = {
        userName: userName.trim() || null,
        userEmail: userEmail.trim() || null,
        jobTitle: jobTitle.trim() || null,
        managerName: managerName.trim() || null,
        managerEmail: managerEmail.trim() || null,
        jiraProjectKeys: parseJiraKeys(jiraKeys),
        startDate,
        endDate,
        reviewQuestions: reviewQuestions.trim() || null,
        okrs: okrs.trim() || null,
        includeNotes,
        todayIso,
      };
      generatedDoc = await invoke<string>('generate_review_prep', { input });
    } catch (e) {
      generateError = String(e);
    } finally {
      generating = false;
    }
  }

  async function saveToFile() {
    saveMessage = '';
    saveIsError = false;
    try {
      const desktop = await desktopDir();
      const filename = defaultFilename();
      const path = await saveDialog({
        defaultPath: `${desktop}/${filename}`,
        filters: [{ name: 'Markdown', extensions: ['md'] }],
      });
      if (!path) return; // user cancelled
      await writeTextFile(path, generatedDoc);
      saveMessage = `Saved to ${path}`;
      saveIsError = false;
    } catch (e) {
      saveMessage = `Save failed: ${String(e)}`;
      saveIsError = true;
    }
  }

  async function copyToClipboard() {
    saveMessage = '';
    saveIsError = false;
    try {
      await clipboardWriteText(generatedDoc);
      saveMessage = 'Copied to clipboard';
      saveIsError = false;
    } catch (e) {
      saveMessage = `Copy failed: ${String(e)}`;
      saveIsError = true;
    }
  }

  function defaultFilename(): string {
    // review-prep-YYYY-MM-DD-to-YYYY-MM-DD.md — falls back to a
    // generic name if either date is missing (shouldn't happen post-
    // Generate; belt + suspenders).
    if (startDate && endDate) {
      return `review-prep-${startDate}-to-${endDate}.md`;
    }
    return 'review-prep.md';
  }

  // Missing-data audit for the final step's warnings. Each entry is a
  // human-readable phrase for a piece of context the user didn't
  // provide; the final step lists them so the user can decide whether
  // it's worth going back to fill in.
  const missingItems = $derived.by(() => {
    const items: string[] = [];
    if (!userName.trim()) items.push('your name');
    if (!jobTitle.trim()) items.push('your job title');
    if (parseJiraKeys(jiraKeys).length === 0) items.push('your Jira project keys');
    if (!reviewQuestions.trim()) items.push('the review questions');
    if (!okrs.trim()) items.push('the OKRs');
    return items;
  });

  // Gating for the "Continue" button on each step. Step 2 (dates)
  // requires both fields filled AND start <= end. Every other step
  // is freely skippable per Chris's "any of this data can be missing"
  // rule.
  const canAdvance = $derived.by(() => {
    if (step === 2) {
      if (!startDate || !endDate) return false;
      return startDate <= endDate;
    }
    return true;
  });
</script>

<Modal
  {open}
  onClose={attemptClose}
  title="Prep Self Review"
  ariaLabelledBy="prep-self-review-title"
  maxWidth="640px"
  focusFirstInput={true}
>
  <div class="wizard">
    {#if loading}
      <p class="loading-line">Loading your settings…</p>
    {:else}
      {#if loadError}
        <p class="error" role="alert">{loadError}</p>
      {/if}

      <!-- Step indicator dots. Same visual language as onboarding's
           WizardFrame but rendered inline here since we don't use the
           Ed-branded frame. -->
      <div class="steps-indicator" aria-hidden="true">
        {#each Array(TOTAL_STEPS) as _, i}
          <span class="dot" class:is-current={i + 1 === step} class:is-past={i + 1 < step}></span>
        {/each}
      </div>

      {#if step === 1}
        <StepHeader
          title="Confirm Your Info"
          level="h2"
          lead="These come from your Settings — edit anything that's out of date and it'll save back."
        />
        <div class="fields">
          <InputField
            id="prep-user-name"
            label="Your name"
            placeholder="e.g. Chris Carpenter"
            bind:value={userName}
            hint="How you'd like to be addressed in the review."
          />
          <InputField
            id="prep-user-email"
            type="email"
            label="Your email"
            placeholder="you@company.com"
            bind:value={userEmail}
          />
          <InputField
            id="prep-job-title"
            label="Job title"
            placeholder="e.g. QA Analyst"
            bind:value={jobTitle}
            hint="Used to calibrate the LLM's read of 'what makes a good X'."
          />
          <InputField
            id="prep-jira-keys"
            label="Jira project keys"
            placeholder="e.g. MAGE, LIVE, FENIX"
            bind:value={jiraKeys}
            hint="Comma or space separated. Used when the LLM looks up specific tickets you mention."
          />
          <InputField
            id="prep-manager-name"
            label="Manager name"
            placeholder="Optional"
            bind:value={managerName}
          />
          <InputField
            id="prep-manager-email"
            type="email"
            label="Manager email"
            placeholder="Optional"
            bind:value={managerEmail}

          />
        </div>
      {/if}

      {#if step === 2}
        <StepHeader
          title="Review Period"
          level="h2"
          lead="Which weeks should the LLM look at? A 6-month range is typical for a mid-year self-review; a 12-month range for annual."
        />
        <div class="fields">
          <div class="field">
            <label for="prep-start-date">Start date</label>
            <!-- Button-anchored DatePickerPopover — same widget the
                 in-editor date chips use. Clicking toggles the popover;
                 popover's outside-click detection uses this element. -->
            <button
              id="prep-start-date"
              type="button"
              class="date-anchor"
              class:is-empty={!startDate}
              bind:this={startAnchorEl}
              onclick={() => (pickerOpen = pickerOpen === 'start' ? null : 'start')}
            >
              {startDate || 'Pick a date'}
            </button>
          </div>
          <div class="field">
            <label for="prep-end-date">End date</label>
            <button
              id="prep-end-date"
              type="button"
              class="date-anchor"
              class:is-empty={!endDate}
              bind:this={endAnchorEl}
              onclick={() => (pickerOpen = pickerOpen === 'end' ? null : 'end')}
            >
              {endDate || 'Pick a date'}
            </button>
          </div>
          {#if startDate && endDate && startDate > endDate}
            <p class="field-hint is-warning" role="alert">Start date must be on or before end date.</p>
          {/if}
        </div>
      {/if}

      {#if step === 3}
        <StepHeader
          title="Performance Review Questions"
          level="h2"
          lead="Paste the questions you need to answer, or drop in a link to the doc they live in (Google Doc, Confluence page, spreadsheet — anything your LLM's connectors can fetch). Prose and links can mix freely."
        />
        <TextAreaField
          id="prep-review-questions"
          label="Review questions"
          placeholder={'Paste the questions here, or a link like:\nhttps://docs.google.com/document/d/...'}
          bind:value={reviewQuestions}
          rows={8}
          urlPaste
        />
        <TipBubble heading="Tip">
          Optional — but the LLM's output is only as focused as
          <strong>these questions</strong>. If you have them, share them.
        </TipBubble>
      {/if}

      {#if step === 4}
        <StepHeader
          title="Company or Team OKRs"
          level="h2"
          lead="Same rules as the questions — plain text or links. The LLM uses these to calibrate what 'good' looks like against your team's stated objectives."
        />
        <TextAreaField
          id="prep-okrs"
          label="Company or team OKRs"
          placeholder={'Paste the OKRs here, or a link like:\nhttps://prodigygame.atlassian.net/wiki/spaces/...'}
          bind:value={okrs}
          rows={8}
          urlPaste
        />
        <TipBubble heading="Tip">
          Optional — skip this if you don't have team OKRs to point at.
          Without them the LLM can't calibrate <strong>impact</strong>
          against stated objectives.
        </TipBubble>
      {/if}

      {#if step === 5}
        <StepHeader
          title="Generate Your Review Prep"
          level="h2"
          lead="One button, one big markdown doc. Hand it to Claude (or your LLM of choice) and say 'look at this and do what it says.'"
        />

        {#if missingItems.length > 0 && !generatedDoc}
          <TipBubble heading="A few things weren't filled in">
            You didn't provide <strong>{formatList(missingItems)}</strong>.
            The generated doc will still work, but the LLM will have less
            context to work with. You can go back to add them, or generate
            as-is.
          </TipBubble>
        {/if}

        <div class="options">
          <Checkbox
            bind:checked={includeNotes}
            label="Include Weekly Notes in the doc"
            description="Off by default. Weekly Summaries alone usually cover a review's needs; toggle this on to also include the raw Note entries."
          />
          <TipBubble heading="If you toggle Notes on">
            A 6-month range with Notes included can push past
            <strong>100,000 tokens</strong>. Use a
            <strong>200k-context model</strong> — Claude
            <strong>Opus</strong> or <strong>Sonnet 5</strong> — or the
            doc will get truncated before the LLM finishes reading it.
          </TipBubble>
        </div>

        {#if !generatedDoc}
          <div class="generate-cta">
            <button
              type="button"
              class="btn btn-emerald"
              onclick={generate}
              disabled={generating || !startDate || !endDate || startDate > endDate}
            >
              {generating ? 'Generating…' : 'Generate Review Prep'}
            </button>
          </div>
          {#if generateError}
            <p class="error" role="alert">Generation failed: {generateError}</p>
          {/if}
        {:else}
          <div class="preview">
            <p class="preview-title">Preview <span class="preview-meta">— {generatedDoc.length.toLocaleString()} characters</span></p>
            <textarea class="text-input preview-body" readonly rows={12}>{generatedDoc}</textarea>
          </div>
          <div class="output-actions">
            <button type="button" class="btn btn-emerald" onclick={saveToFile}>Save to file…</button>
            <button type="button" class="btn btn-marble" onclick={copyToClipboard}>Copy to clipboard</button>
          </div>
          {#if saveMessage}
            <p class="save-status" class:is-error={saveIsError}>{saveMessage}</p>
          {/if}
        {/if}
      {/if}

      <!-- Actions row. Back stays on the left (natural back = left);
           Continue + Cancel both sit on the right, matching the
           ConfirmDialog / SendToManagerButton preview convention.
           Cancel is ruby (wine) to match every other cancel button in
           the app. Once the doc has been generated, the right cluster
           collapses to a single marble "Close" — the flow is done. -->
      <div class="actions">
        {#if step > 1}
          <button type="button" class="btn btn-marble" onclick={back} disabled={generating}>Back</button>
        {:else}
          <span class="actions-spacer"></span>
        {/if}
        <div class="actions-right">
          {#if step < TOTAL_STEPS}
            <button
              type="button"
              class="btn btn-emerald"
              onclick={next}
              disabled={!canAdvance}
            >
              Continue
            </button>
            <button type="button" class="btn btn-ruby" onclick={attemptClose}>Cancel</button>
          {:else if !generatedDoc}
            <button type="button" class="btn btn-ruby" onclick={attemptClose}>Cancel</button>
          {:else}
            <button type="button" class="btn btn-marble" onclick={attemptClose}>Close</button>
          {/if}
        </div>
      </div>
    {/if}
  </div>
</Modal>

{#if confirmingCancel}
  <ConfirmDialog
    title="Discard Your Progress?"
    confirmLabel="Discard"
    confirmVariant="ruby"
    cancelLabel="Keep editing"
    onConfirm={confirmCancel}
    onCancel={abortCancel}
  >
    {#snippet body()}
      <p>You've started filling in the review-prep form. If you close now, your inputs will be lost.</p>
    {/snippet}
  </ConfirmDialog>
{/if}

<!--
  Date pickers for the review-period step. Rendered outside the
  Modal body since DatePickerPopover uses fixed positioning against
  the anchor button's bounding rect — leaving them inside the modal
  is fine but rendering at the top-level tag ensures the popover
  z-index stacks cleanly above the modal card. Each picker seeds its
  `iso` from the current selection when present, else today's local
  date so the calendar opens somewhere sensible.
-->
{#if pickerOpen === 'start' && startAnchorEl}
  <DatePickerPopover
    iso={startDate || todayIsoLocal()}
    from={0}
    to={0}
    anchorEl={startAnchorEl}
    onCommit={(iso) => { startDate = iso; pickerOpen = null; }}
    onClose={() => (pickerOpen = null)}
    onClear={startDate ? () => { startDate = ''; pickerOpen = null; } : undefined}
  />
{/if}
{#if pickerOpen === 'end' && endAnchorEl}
  <DatePickerPopover
    iso={endDate || todayIsoLocal()}
    from={0}
    to={0}
    anchorEl={endAnchorEl}
    onCommit={(iso) => { endDate = iso; pickerOpen = null; }}
    onClose={() => (pickerOpen = null)}
    onClear={endDate ? () => { endDate = ''; pickerOpen = null; } : undefined}
  />
{/if}

<script lang="ts" module>
  function formatList(items: string[]): string {
    if (items.length === 0) return '';
    if (items.length === 1) return items[0];
    if (items.length === 2) return `${items[0]} and ${items[1]}`;
    return `${items.slice(0, -1).join(', ')}, and ${items[items.length - 1]}`;
  }
  export { formatList };
</script>

<style>
  .wizard {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  .loading-line {
    color: var(--text-secondary);
    text-align: center;
    padding: var(--space-6) 0;
  }

  .error {
    color: var(--accent-pink-text);
    margin: 0;
    padding: var(--space-2) var(--space-3);
    border-left: 3px solid var(--accent-pink-text);
    background: color-mix(in srgb, var(--accent-pink-text) 8%, transparent);
    border-radius: var(--radius-sm);
  }

  .steps-indicator {
    display: flex;
    justify-content: center;
    gap: var(--space-2);
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--border-decorative);
  }
  .dot.is-past {
    background: color-mix(in srgb, var(--accent-primary) 60%, transparent);
  }
  .dot.is-current {
    background: var(--accent-primary);
    transform: scale(1.2);
  }

  .fields {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .field label {
    font-family: var(--font-display);
    font-size: var(--text-button);
    color: var(--text-primary);
  }
  /* Date-anchor buttons open the themed DatePickerPopover — same
     widget the in-editor date chips use. Chip-like presentation to
     signal "clickable date value"; falls back to a placeholder
     appearance when unset. */
  .date-anchor {
    align-self: flex-start;
    padding: var(--space-2) var(--space-3);
    background: var(--bg-elevated);
    border: 1px solid var(--border-structural);
    border-radius: var(--radius-md);
    color: var(--accent-primary-text);
    font: inherit;
    font-family: var(--font-body);
    cursor: pointer;
    min-width: 160px;
    text-align: left;
    transition: border-color var(--transition-fast);
  }
  .date-anchor:hover,
  .date-anchor:focus-visible {
    border-color: var(--accent-primary);
    outline: none;
  }
  .date-anchor:focus-visible {
    box-shadow: 0 0 0 2px var(--focus-glow);
  }
  .date-anchor.is-empty {
    color: var(--text-secondary);
    font-style: italic;
  }

  .field-hint {
    margin: 0;
    font-size: var(--text-caption);
    line-height: var(--text-caption-lh);
    color: var(--text-secondary);
  }
  .field-hint.is-warning {
    color: var(--accent-pink-text);
  }

  .options {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .generate-cta {
    display: flex;
    justify-content: center;
    padding: var(--space-3) 0;
  }

  .preview {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .preview-title {
    font-family: var(--font-display);
    color: var(--text-primary);
    margin: 0;
    font-size: var(--text-button);
  }
  .preview-meta {
    color: var(--text-secondary);
    font-size: var(--text-caption);
    font-family: var(--font-body);
    margin-left: var(--space-2);
  }
  .preview-body {
    font-family: var(--font-mono, monospace);
    font-size: var(--text-caption);
    line-height: 1.5;
    resize: vertical;
    min-height: 200px;
  }

  .output-actions {
    display: flex;
    gap: var(--space-3);
    justify-content: flex-end;
  }

  .save-status {
    margin: 0;
    font-size: var(--text-caption);
    color: var(--text-secondary);
  }
  .save-status.is-error {
    color: var(--accent-pink-text);
  }

  .actions {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: var(--space-3);
    padding-top: var(--space-3);
    border-top: 1px solid var(--border-decorative);
  }
  /* Placeholder when step 1 has no Back button — preserves the
     space-between layout so Continue + Cancel stay right-aligned. */
  .actions-spacer {
    flex: 0 0 auto;
  }
  .actions-right {
    display: flex;
    gap: var(--space-3);
  }
</style>
