<!--
  LabelDetailsModal — Phase 3a, Slice 9.

  Per-label details popup for the Settings → Labels tab. Surfaces:
    1. Header with chip + name + Close (×).
    2. Usage stats (get_label_stats) — total + per-surface breakdown +
       a drift hint when scanned total disagrees with labels.json's
       cached index_count. Per locked-decision #8 scanned total wins;
       this component never auto-repairs.
    3. Color section (only when colorfulLabels is on) — 28×28 swatch,
       monospace hex input, "Reset to auto" button, "Browse colors at
       htmlcolorcodes.com →" link. Per locked-decision #5 the hex
       input commits ON BLUR (not per-keystroke) so a half-typed value
       doesn't fire a Tauri write on every character.
    4. Rename — input pre-filled with current name, reuses the Rust
       is_label_char rule (alphanumeric + '_' + '-') for client-side
       validation per locked-decision #9. Confirm prompt before the
       file rewrite so the user knows roughly how many weekly files
       will be touched.
    5. Delete — destructive, separated by a top border + ruby button.
       Confirm modal explains the cascade scope per locked-decision
       #2 (explicit labels:[…] arrays only; inline #hashtag text in
       prose stays put). The hint below the button restates that
       scope so a user reading the popup quickly still sees it even
       if they skim past the confirm dialog.

  Modal mechanics:
    - role="dialog" + aria-modal="true" + aria-labelledby on the header.
    - Escape closes the modal (or dismisses an inner confirm if one is up).
    - Focus is moved to the dialog on mount so keyboard users land
      inside it; a focus trap cycles Tab/Shift-Tab between the first
      and last focusable element.
    - Clicking the backdrop closes — mouse-only convenience on top of
      Close + Escape.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { chipStyleFor, type ChipEntry } from '$lib/labelChip';
  import { generateLabelColor } from '$lib/theme';
  import ConfirmDialog from '$lib/ConfirmDialog.svelte';
  import Modal from '$lib/Modal.svelte';
  import TipBubble from '$lib/onboarding/TipBubble.svelte';

  // -------------------------------------------------------------------------
  // Types
  // -------------------------------------------------------------------------

  type LabelEntry = {
    name: string;
    count: number;
    firstUsed: string;
    lastUsed: string;
    color?: string | null;
  };

  // camelCase to match the Rust LabelStats serde rename_all.
  type LabelStats = {
    total: number;
    inNotes: number;
    inSummaries: number;
    indexCount: number;
  };

  // Rename surfaces RenameResult (camelCase) — we only read
  // files_modified / occurrences_replaced for the success toast. failed_files
  // is surfaced as a non-blocking warning so the user can retry per
  // locked-decision #7.
  type RenameResult = {
    filesModified: number;
    occurrencesReplaced: number;
    failedFiles: string[];
  };

  // Delete surfaces DeleteResult; same shape rationale as RenameResult.
  type DeleteResult = {
    filesModified: number;
    occurrencesRemoved: number;
    failedFiles: string[];
  };

  // -------------------------------------------------------------------------
  // Props
  // -------------------------------------------------------------------------

  let {
    label,
    colorfulLabels,
    theme,
    bgSurface,
    onClose,
    onLabelMutated,
  }: {
    label: LabelEntry;
    colorfulLabels: boolean;
    theme: 'light' | 'dark' | 'custom';
    bgSurface?: string;
    onClose: () => void;
    /**
     * Parent refreshes its own list after a successful rename or delete.
     * Rename leaves the popup open (with the new name reflected); delete
     * closes via onClose() first, then fires onLabelMutated().
     */
    onLabelMutated: () => void;
  } = $props();

  // -------------------------------------------------------------------------
  // Local working state
  // -------------------------------------------------------------------------
  //
  // We MIRROR `label` into a local `current` $state so a successful rename
  // can update the header + chip + rename-input prefill in place without
  // remounting. The parent's onLabelMutated() will refresh its own list;
  // the popup happens to also reflect the new name locally so the user
  // sees the change immediately rather than a stale value until close.
  //
  // `current.color` shadows the persisted hex override; the swatch + chip
  // read off it on every render. Set/Reset commits to the backend AND
  // updates `current.color` so subsequent renders use the fresh value.

  // svelte-ignore state_referenced_locally — intentional: we snapshot the
  // initial label prop into local working state so a successful rename can
  // update the header in place without depending on the parent re-passing
  // the prop. Parent re-mounts the modal when a different label row is
  // clicked (selectedLabel is set anew).
  let current = $state<LabelEntry>({ ...label });

  // Stats state — null while loading, error string on failure, LabelStats
  // on success. We don't gate the rest of the popup on stats — Color /
  // Rename / Delete are usable even if stats fail (e.g. transient FS hiccup).
  let stats = $state<LabelStats | null>(null);
  let statsLoading = $state(true);
  let statsError = $state('');

  // -------------------------------------------------------------------------
  // Color section state
  // -------------------------------------------------------------------------
  //
  // `colorInput` is the working hex string the user sees in the textbox.
  // We seed it from current.color (the persisted override) and leave the
  // placeholder showing the generated default when empty. ON-BLUR commits
  // a value that's valid AND differs from current.color — that way a
  // half-typed value (e.g. user clicked into the field then clicked away)
  // doesn't trigger a Tauri write per locked-decision #5.
  // svelte-ignore state_referenced_locally — initial seed only; see `current` above.
  let colorInput = $state(label.color ?? '');
  let colorError = $state('');

  // -------------------------------------------------------------------------
  // Rename section state
  // -------------------------------------------------------------------------

  // svelte-ignore state_referenced_locally — initial seed only; see `current` above.
  let renameInput = $state(label.name);
  let renameError = $state('');
  let renameInFlight = $state(false);
  let renameWarning = $state(''); // non-blocking — surfaces failed_files.
  let showRenameConfirm = $state(false);

  // -------------------------------------------------------------------------
  // Delete section state
  // -------------------------------------------------------------------------

  let deleteError = $state('');
  let deleteInFlight = $state(false);
  let showDeleteConfirm = $state(false);

  // -------------------------------------------------------------------------
  // Refs for focus management
  // -------------------------------------------------------------------------

  // dialogEl + closeBtnEl removed when the popup chrome moved to Modal —
  // Modal owns focus management, body-scroll lock, and the close button.

  // -------------------------------------------------------------------------
  // Validation helpers
  // -------------------------------------------------------------------------

  // Mirror of Rust's is_label_char (labels.rs) per locked-decision #9 —
  // alphanumeric + '_' + '-'. We validate client-side so the Rename button
  // can be disabled before the user clicks, but the backend re-checks on
  // every rename_label call.
  const LABEL_NAME_RE = /^[A-Za-z0-9_-]+$/;
  function isValidLabelName(s: string): boolean {
    return LABEL_NAME_RE.test(s);
  }

  // 6-digit hex, matching the backend's is_hex6 rule in commands.rs.
  const HEX6_RE = /^#[0-9a-fA-F]{6}$/;
  function isValidHex6(s: string): boolean {
    return HEX6_RE.test(s.trim());
  }

  // -------------------------------------------------------------------------
  // Derived: validation surface for the rename input
  // -------------------------------------------------------------------------

  const trimmedRename = $derived(renameInput.trim().replace(/^#+/, ''));

  const renameValidation = $derived.by(() => {
    if (trimmedRename.length === 0) {
      return { ok: false, message: '' }; // empty = silent — disable button, no error pill
    }
    if (trimmedRename === current.name) {
      return { ok: false, message: '' }; // identical = silent (same reason)
    }
    if (!isValidLabelName(trimmedRename)) {
      return {
        ok: false,
        message: 'Only letters, numbers, "_", and "-" are allowed.',
      };
    }
    return { ok: true as const, message: '' };
  });

  // -------------------------------------------------------------------------
  // Derived: chip + swatch + placeholder color
  // -------------------------------------------------------------------------

  // The chip in the header re-paints whenever `current.color` flips or the
  // theme changes. chipStyleFor handles the colorful-on / off branch.
  const chipStyle = $derived.by(() => {
    const entry: ChipEntry = { name: current.name, color: current.color ?? null };
    return chipStyleFor(entry, colorfulLabels, theme, bgSurface);
  });

  // Auto-generated default for the placeholder + reset target. Falls back
  // to a deterministic per-name hex against the current theme.
  const generatedColor = $derived(generateLabelColor(current.name, theme, bgSurface));

  // The 28×28 swatch shows whatever the user is currently looking at — the
  // committed override OR (when no override) the auto-generated default.
  // It does NOT track the half-typed colorInput because that would cause
  // a perceived "preview" before the on-blur commit and confuse users into
  // thinking the change had landed.
  const swatchColor = $derived(current.color ?? generatedColor);

  // -------------------------------------------------------------------------
  // Stats fetch on mount
  // -------------------------------------------------------------------------

  onMount(async () => {
    // Modal owns focus management — no manual closeBtnEl.focus() here.
    try {
      const s = await invoke<LabelStats>('get_label_stats', { name: label.name });
      stats = s;
    } catch (err) {
      statsError = String(err);
    } finally {
      statsLoading = false;
    }
  });

  // Escape + focus-trap handling moved to Modal. Modal's stack-aware
  // Escape listener routes Esc to the topmost open Modal, so a nested
  // ConfirmDialog (which is also a Modal under the hood) dismisses
  // the confirm first; another Esc closes the parent details modal.

  // -------------------------------------------------------------------------
  // Color commit handlers
  // -------------------------------------------------------------------------

  async function commitColor(): Promise<void> {
    const candidate = colorInput.trim().toLowerCase();
    // Empty input on blur = no-op (Reset-to-auto is the explicit clear path).
    if (candidate === '') {
      colorError = '';
      return;
    }
    if (!isValidHex6(candidate)) {
      colorError = 'Not a 6-digit hex color. Use the form #rrggbb.';
      return;
    }
    // Same value already on disk → don't send another write.
    if (candidate === (current.color ?? '').toLowerCase()) {
      colorError = '';
      return;
    }
    try {
      await invoke('set_label_color', { name: current.name, color: candidate });
      current.color = candidate;
      colorInput = candidate;
      colorError = '';
      // Refresh parent so its row chip picks up the new color.
      onLabelMutated();
    } catch (err) {
      colorError = String(err);
    }
  }

  async function resetColor(): Promise<void> {
    try {
      await invoke('set_label_color', { name: current.name, color: null });
      current.color = null;
      colorInput = '';
      colorError = '';
      onLabelMutated();
    } catch (err) {
      colorError = String(err);
    }
  }

  async function openColorIdeas(): Promise<void> {
    try {
      await openUrl('https://htmlcolorcodes.com/');
    } catch {
      // Best-effort — no error surface required; the user can navigate
      // there manually if the opener fails.
    }
  }

  // -------------------------------------------------------------------------
  // Rename handlers
  // -------------------------------------------------------------------------

  function requestRename(): void {
    if (!renameValidation.ok || renameInFlight) return;
    renameError = '';
    showRenameConfirm = true;
  }

  async function confirmRename(): Promise<void> {
    if (!renameValidation.ok || renameInFlight) return;
    const oldName = current.name;
    const newName = trimmedRename;
    showRenameConfirm = false;
    renameError = '';
    renameWarning = '';
    renameInFlight = true;
    try {
      const result = await invoke<RenameResult>('rename_label', {
        oldName,
        newName,
      });
      current.name = newName;
      renameInput = newName;
      if (result.failedFiles && result.failedFiles.length > 0) {
        // Locked decision #7: surface failed files; retries are idempotent.
        const sample = result.failedFiles.slice(0, 3).join(', ');
        const more = result.failedFiles.length > 3
          ? ` (+${result.failedFiles.length - 3} more)`
          : '';
        renameWarning = `Renamed in ${result.filesModified} files but ${result.failedFiles.length} couldn't be touched: ${sample}${more}. Retry — renames are idempotent.`;
      }
      onLabelMutated();
    } catch (err) {
      renameError = String(err);
    } finally {
      renameInFlight = false;
    }
  }

  // -------------------------------------------------------------------------
  // Delete handlers
  // -------------------------------------------------------------------------

  function requestDelete(): void {
    if (deleteInFlight) return;
    deleteError = '';
    showDeleteConfirm = true;
  }

  async function confirmDelete(): Promise<void> {
    if (deleteInFlight) return;
    deleteInFlight = true;
    deleteError = '';
    try {
      const result = await invoke<DeleteResult>('delete_label_cascade', {
        name: current.name,
      });
      showDeleteConfirm = false;
      // Close BEFORE firing onLabelMutated so the parent renders its
      // refreshed list with our row gone — no flash of stale chip after
      // a successful cascade.
      onClose();
      onLabelMutated();
      // If some files failed, surface via window-level alert? No — the
      // popup is gone. Stash on the parent? Out of scope for Slice 9; the
      // partial-failure shape is already visible in the next get_labels
      // result (the label may still appear with a reduced count).
      void result;
    } catch (err) {
      deleteError = String(err);
      deleteInFlight = false;
    }
  }

  // -------------------------------------------------------------------------
  // Stats summary helpers
  // -------------------------------------------------------------------------

  // "1 note" vs "5 notes". Plain English, no abbreviations — this is the
  // Captain's Log microcopy register.
  function pluralize(n: number, singular: string, plural: string): string {
    return n === 1 ? `${n} ${singular}` : `${n} ${plural}`;
  }
</script>

<!-- Modal shell handles backdrop dim+blur, body-scroll lock, Escape, and
     the standard header bar. The chip + name "hero" lives at the top of
     the body so the chosen color stays visually prominent. -->
<Modal open={true} {onClose} title="Label Details" maxWidth="min(560px, calc(100vw - 32px))">
  <div class="label-hero">
    <span class="header-chip" style={chipStyle}>{current.name}</span>
    <h2 class="header-name">{current.name}</h2>
  </div>

  <!-- Usage stats. -->
  <section class="section">
    <h3 class="section-title">Usage</h3>
      {#if statsLoading}
        <div class="stats-loading" role="status" aria-live="polite">
          <span class="spinner" aria-hidden="true"></span>
          <span>Loading stats…</span>
        </div>
      {:else if statsError}
        <p class="hint hint-warning">Couldn't load stats: {statsError}</p>
      {:else if stats}
        <p class="stats-line">Used {pluralize(stats.total, 'time', 'times')} total</p>
        <p class="stats-line stats-sub">in {pluralize(stats.inNotes, 'note', 'notes')}</p>
        <p class="stats-line stats-sub">
          in {pluralize(stats.inSummaries, 'weekly summary', 'weekly summaries')}
        </p>
        {#if stats.total !== stats.indexCount}
          <p class="stats-drift">
            (label index shows {stats.indexCount} — drift detected, will
            reconcile on next rebuild)
          </p>
        {/if}
      {/if}
    </section>

    <!-- Color section — Colorful Labels gate. -->
    {#if colorfulLabels}
      <section class="section">
        <h3 class="section-title">Color</h3>
        <div class="color-row">
          <span
            class="color-swatch"
            style="background-color: {swatchColor}"
            aria-label="Current label color: {swatchColor}"
          ></span>
          <input
            class="text-input color-input"
            type="text"
            inputmode="text"
            spellcheck="false"
            autocomplete="off"
            placeholder={generatedColor}
            bind:value={colorInput}
            onblur={() => void commitColor()}
            aria-label="Label color hex"
          />
          <button
            type="button"
            class="btn btn-marble btn-sm"
            disabled={current.color === null || current.color === undefined}
            onclick={() => void resetColor()}
          >
            Reset
          </button>
        </div>
        {#if colorError}
          <p class="hint hint-warning">{colorError}</p>
        {/if}
        <p class="hint">
          Need color ideas?
          <button
            type="button"
            class="link-button"
            onclick={() => void openColorIdeas()}
          >
            Browse colors at htmlcolorcodes.com →
          </button>
        </p>
      </section>
    {/if}

    <!-- Rename section. -->
    <section class="section">
      <h3 class="section-title">Rename</h3>
      <div class="rename-row">
        <input
          class="text-input rename-input"
          class:invalid={renameInput.trim() !== '' && renameInput.trim() !== current.name && !renameValidation.ok}
          type="text"
          spellcheck="false"
          autocomplete="off"
          bind:value={renameInput}
          aria-label="New label name"
          disabled={renameInFlight}
        />
        <button
          type="button"
          class="btn btn-emerald btn-sm"
          disabled={!renameValidation.ok || renameInFlight}
          onclick={requestRename}
        >
          {renameInFlight ? 'Renaming…' : 'Rename'}
        </button>
      </div>
      {#if renameValidation.message}
        <p class="hint hint-warning">{renameValidation.message}</p>
      {/if}
      {#if renameError}
        <p class="hint hint-warning">{renameError}</p>
      {/if}
      {#if renameWarning}
        <p class="hint hint-warning">{renameWarning}</p>
      {/if}
    </section>

    <!-- Delete section — destructive; section-title bottom border + ruby
         button + a TipBubble explaining the cascade scope so users know
         what gets touched (and what doesn't). -->
    <section class="section section-delete">
      <h3 class="section-title section-title-delete">Delete</h3>
      <button
        type="button"
        class="btn btn-ruby btn-sm"
        disabled={deleteInFlight}
        onclick={requestDelete}
      >
        {deleteInFlight ? 'Deleting…' : 'Delete this label'}
      </button>
      <TipBubble heading="How delete works">
        Removes this label from every Note and Weekly Summary's labels list.
        Inline <code>#hashtag</code> text in note bodies is left alone —
        clean those up by hand if you want to.
      </TipBubble>
      {#if deleteError}
        <p class="hint hint-warning">{deleteError}</p>
      {/if}
    </section>
</Modal>

  <!-- Rename + Delete confirms — both via the reusable ConfirmDialog.
       Sit OUTSIDE the Modal so they layer cleanly via ConfirmDialog's
       own nested z-index. The window-keydown handler in the script block
       checks showRenameConfirm / showDeleteConfirm first so Escape
       dismisses the confirm before falling through to Modal's onClose. -->
  {#if showRenameConfirm}
    <ConfirmDialog
      title="Rename Label?"
      confirmLabel="Rename"
      confirmVariant="emerald"
      cancelVariant="ruby"
      onConfirm={() => void confirmRename()}
      onCancel={() => (showRenameConfirm = false)}
      body={renameConfirmBody}
    />
  {/if}

  {#if showDeleteConfirm}
    <ConfirmDialog
      title="Delete Label?"
      confirmLabel="Delete"
      confirmVariant="ruby"
      cancelVariant="marble"
      onConfirm={() => void confirmDelete()}
      onCancel={() => (showDeleteConfirm = false)}
      body={deleteConfirmBody}
    />
  {/if}

{#snippet renameConfirmBody()}
  <p>
    Rename <strong>{current.name}</strong> to <strong>{trimmedRename}</strong>?
    This rewrites the label across every Note and Weekly Summary
    {#if stats}({pluralize(stats.total, 'use', 'uses')} on disk){/if}.
  </p>
{/snippet}

{#snippet deleteConfirmBody()}
  <p>
    Delete <strong>{current.name}</strong>?
    {#if stats}
      Used {pluralize(stats.total, 'time', 'times')}.
    {/if}
    This removes the label from every Note and Weekly Summary that
    references it. This cannot be undone.
  </p>
{/snippet}

<style>
  /* Modal shell (backdrop, dim+blur, scroll lock, header bar, close
     button) lives in $lib/Modal.svelte — this file only styles the
     content INSIDE the body. */

  /* Hero block: chip + name as the visual lead of the popup body so
     the user sees the label they clicked on as the obvious "this is
     about X" anchor before the sections below. */
  .label-hero {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    margin-bottom: var(--space-4);
    padding-bottom: var(--space-3);
    border-bottom: 1px solid var(--border-decorative);
  }

  .header-chip {
    display: inline-flex;
    align-items: center;
    padding: 2px var(--space-2);
    background: transparent;
    border: 1.5px solid var(--chip-color);
    color: var(--chip-color);
    border-radius: var(--radius-pill);
    font-family: var(--font-body);
    font-size: var(--text-caption);
    line-height: var(--text-caption-lh);
    white-space: nowrap;
    max-width: 180px;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .header-name {
    margin: 0;
    font-family: var(--font-display);
    font-size: var(--text-display-sm);
    line-height: var(--text-display-sm-lh);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* Section + section-title styled to match the General tab of /settings
     (display-font title, bottom border, breathing room) so the popup's
     internal structure reads as one design language with the rest of
     the app. Chris specifically asked for parity here. */
  .section {
    margin-top: var(--space-5);
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }
  .section-title {
    font-family: var(--font-display);
    font-size: var(--text-button);
    color: var(--text-primary);
    margin: 0 0 var(--space-2);
    padding-bottom: var(--space-2);
    border-bottom: 1px solid var(--border-decorative);
  }
  .section-title-delete {
    color: var(--accent-pink-text, var(--accent-pink));
    border-bottom-color: var(--accent-pink-text, var(--accent-pink));
  }

  /* Stats text — lead line is normal weight; the indented sub-lines feel
     like list items under it. */
  .stats-line {
    margin: 0 0 var(--space-1);
    color: var(--text-primary);
    line-height: var(--text-body-lh);
  }
  .stats-sub {
    padding-left: var(--space-4);
    color: var(--text-secondary);
  }
  .stats-drift {
    margin-top: var(--space-2);
    color: var(--text-muted, var(--text-secondary));
    font-size: var(--text-caption);
    line-height: var(--text-caption-lh);
  }
  .stats-loading {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    color: var(--text-secondary);
  }
  .spinner {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: 2px solid var(--border-structural);
    border-top-color: var(--accent-primary);
    animation: spin 0.9s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  /* Color row: 28×28 swatch + monospace hex input + reset button. */
  .color-row {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }
  .color-swatch {
    display: inline-block;
    width: 28px;
    height: 28px;
    border-radius: var(--radius-sm, 4px);
    border: 1px solid var(--border-structural);
    flex-shrink: 0;
  }
  .color-input {
    /* Monospace so the hex digits sit in a stable column — matches the
       custom-theme token-input pattern in /settings. */
    font-family: var(--font-mono, ui-monospace, SFMono-Regular, Menlo, monospace);
    flex: 1;
    min-width: 0;
  }

  /* Rename row: input flexes, button shrinks to fit. */
  .rename-row {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }
  .rename-input {
    flex: 1;
    min-width: 0;
  }
  .rename-input.invalid {
    border-color: var(--border-error, var(--accent-pink));
  }

  /* Hint utility — local copy of the .hint class used elsewhere in the app,
     scoped here so the component is self-contained. */
  .hint {
    margin: var(--space-2) 0 0;
    color: var(--text-secondary);
    font-size: var(--text-caption);
    line-height: var(--text-caption-lh);
  }
  .hint-warning {
    color: var(--text-primary);
    padding: var(--space-2) var(--space-3);
    background: var(--bg-error-tint-soft, var(--bg-error-tint));
    border-radius: var(--radius-md);
    border-left: 3px solid var(--border-error, var(--accent-pink));
  }
  /* (.hint-delete-scope removed — the delete-scope hint moved to
   * TipBubble for a consistent tip widget across the app.) */

  .link-button {
    display: inline;
    background: none;
    border: none;
    padding: 0;
    margin: 0;
    font: inherit;
    color: var(--accent-primary-text);
    text-decoration: underline;
    text-underline-offset: 2px;
    cursor: pointer;
  }
  .link-button:hover,
  .link-button:focus-visible {
    text-decoration: none;
    outline: none;
  }

  /* Inner confirm modal markup + styles moved to $lib/ConfirmDialog.svelte
   * so any future "are you sure?" prompt reuses the same shape + a11y
   * wiring. */

  /* Local copy of the .text-input affordance so the modal doesn't depend on
     a global stylesheet for its inputs. Matches the shape used by .text-input
     in /settings. */
  .text-input {
    width: 100%;
    padding: var(--space-2) var(--space-3);
    background: var(--bg-surface);
    border: 1px solid var(--border-structural);
    border-radius: var(--radius-md);
    color: var(--text-primary);
    font-family: var(--font-body);
    font-size: var(--text-body);
    line-height: var(--text-body-lh);
    box-sizing: border-box;
  }
  .text-input:focus-visible {
    outline: none;
    border-color: var(--accent-primary);
    box-shadow: 0 0 0 2px var(--focus-glow);
  }
</style>
