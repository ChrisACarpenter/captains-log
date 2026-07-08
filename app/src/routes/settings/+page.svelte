<!--
  Settings — Phase 2.7 tabbed redesign.

  Three top-level tabs to keep the page navigable as the data model
  grows past the single-scroll comfort zone:

    General  — Your details / Manager / Journal location (section
               breaks inside one tab)
    Reminders — multi-day picker (toggle pills Mon-Sun), time, the
               persistent-notification "Tip" bubble (moved here from
               the General area where it used to live alongside the
               reminder controls)
    Theme    — Light / Dark / Custom radio. Custom surfaces a
               12-token editor (Phase 2.8) for fine-grained color
               control with WCAG AA contrast validation; Light and
               Dark are the preset palettes. Colorful Labels toggle
               (Phase 2.8b) also lives here.

  Active tab is persisted in localStorage so reopening Settings lands
  the user where they left off.

  All form fields are bound to module-scope $state; clicking Done
  submits the WHOLE form regardless of which tab is showing — there's
  one Save button at the bottom that persists every field on the page.
-->
<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { goto } from '$app/navigation';
  import { invoke } from '@tauri-apps/api/core';
  import { open, save as saveDialog } from '@tauri-apps/plugin-dialog';
  import { readTextFile, writeTextFile } from '@tauri-apps/plugin-fs';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { formatHex, parse } from 'culori';
  import TipBubble from '$lib/onboarding/TipBubble.svelte';
  import ConfirmDialog from '$lib/ConfirmDialog.svelte';
  import Modal from '$lib/Modal.svelte';
  import InputField from '$lib/InputField.svelte';
  import Checkbox from '$lib/Checkbox.svelte';
  import LabelDetailsModal from '$lib/LabelDetailsModal.svelte';
  import LoadingOverlay from '$lib/LoadingOverlay.svelte';
  import PathPickerField from '$lib/PathPickerField.svelte';
  import { chipStyleFor, type ChipEntry } from '$lib/labelChip';
  import {
    applyCustomTheme,
    clearCustomTheme,
    contrastRatio,
    deriveTokens,
    isAmbiguousBaseSurface,
    isSaturatedSurface,
    SHIPPING_DARK_PRIMARIES,
    SHIPPING_LIGHT_PRIMARIES,
    type PrimaryTokens,
    type ThemeBase,
  } from '$lib/theme';

  type Theme = 'dark' | 'light' | 'custom';

  // Phase 2.8 — User-editable Custom theme primaries (12 tokens).
  // Mirrors Rust's CustomTheme struct and PrimaryTokens in $lib/theme.
  // All values are 6-digit hex `#rrggbb`.
  type CustomTheme = {
    bgBase: string;
    bgSurface: string;
    bgElevated: string;
    textPrimary: string;
    textSecondary: string;
    textMuted: string;
    borderStructural: string;
    borderDecorative: string;
    accentPrimary: string;
    accentGreen: string;
    accentPink: string;
    btnSapphire: string;
  };

  // Phase 2.9b — Mail tab. Three send paths the user can pick between.
  // Wire format matches Rust's serde(rename_all = "kebab-case") on the
  // MailSendMode enum.
  type MailSendMode = 'gmail' | 'native-mail' | 'outlook';
  type MailBodyFormat = 'clean-text' | 'markdown-source';
  type OutlookFlavor = 'business' | 'personal';
  // Phase 2.9c — global Body delivery setting. Orthogonal to mail send
  // mode. 'prefilled' is the default-safe path (body embedded in URL or
  // AppleScript). 'clipboard-paste' opens an empty draft and writes
  // rich HTML to the clipboard so the user can Cmd+V into the compose
  // body for a formatted send.
  type MailBodyDelivery = 'prefilled' | 'clipboard-paste';

  type ReminderSettings = {
    enabled: boolean;
    daysOfWeek: number[];
    hour: number;
    minute: number;
  };

  // Phase 3c Slice 4 — display prefs for the landing-page task list.
  // Mirrors the Rust TaskListSettings struct in settings.rs.
  type TaskListSettings = {
    showCompleted: boolean;
    openTasksFirst: boolean;
    showCompletedTimestamp: boolean;
    hideTaskList: boolean;
    autoRolloverEnabled: boolean;
  };

  type Settings = {
    firstRun: boolean;
    journalRoot: string;
    defaultJournalRoot: string;
    userName: string | null;
    userEmail: string | null;
    reminder: ReminderSettings;
    theme: Theme;
    customTheme: CustomTheme | null;
    managerEmail: string | null;
    managerName: string | null;
    bambooTitle: string | null;
    jiraProjectKeys: string[];
    mailSendMode: MailSendMode;
    mailBodyFormat: MailBodyFormat;
    mailNativeHtml: boolean;
    mailOutlookFlavor: OutlookFlavor;
    mailBodyDelivery: MailBodyDelivery;
    // Phase 2.8+ Colorful Labels. Persisted in JournalSettings; the
    // Theme tab surfaces a toggle bound to this. Mirrored here so the
    // frontend Settings type stays in lockstep with the backend
    // SettingsBundle shape.
    colorfulLabels: boolean;
    // Slice 4 — display prefs for the landing-page task list.
    taskList: TaskListSettings;
  };

  type TabKey = 'general' | 'reminders' | 'mail' | 'theme' | 'labels' | 'tasks';
  const TABS: Array<{ key: TabKey; label: string }> = [
    { key: 'general', label: 'General' },
    { key: 'reminders', label: 'Reminders' },
    { key: 'mail', label: 'Mail' },
    { key: 'theme', label: 'Theme' },
    { key: 'labels', label: 'Labels' },
    { key: 'tasks', label: 'Tasks' },
  ];
  const TAB_STORAGE_KEY = 'captainslog:settingsTab';

  // Phase 3a — Label Manager (Slice 8).
  // LabelEntry mirrors the Rust struct in labels.rs.
  type LabelEntry = {
    name: string;
    count: number;
    firstUsed: string; // ISO date YYYY-MM-DD
    lastUsed: string;
    color?: string | null;
  };

  // Mon=0 … Sun=6 — matches the Rust day_of_week ordering.
  const DAYS: Array<{ value: number; short: string; long: string }> = [
    { value: 0, short: 'Mon', long: 'Monday' },
    { value: 1, short: 'Tue', long: 'Tuesday' },
    { value: 2, short: 'Wed', long: 'Wednesday' },
    { value: 3, short: 'Thu', long: 'Thursday' },
    { value: 4, short: 'Fri', long: 'Friday' },
    { value: 5, short: 'Sat', long: 'Saturday' },
    { value: 6, short: 'Sun', long: 'Sunday' }
  ];

  // ---------- State ----------

  let loading = $state(true);
  let loadError = $state('');
  let saving = $state(false);
  let saveError = $state('');

  // Slice 8 — always open to General on mount, overriding any persisted
  // last-active value. See loadActiveTab() comment for rationale.
  let activeTab = $state<TabKey>('general');

  // Form fields
  let nameInput = $state('');
  let userEmailInput = $state('');
  let journalRootInput = $state('');
  let originalJournalRoot = $state('');
  let originalTheme = $state<Theme>('dark');
  let currentTheme = $state<Theme>('dark');
  let reminderEnabled = $state(false);
  // Phase 2.7 multi-day. Loaded from s.reminder.daysOfWeek which the
  // backend serde shim normalizes (sorted, deduped, ≤6 only).
  let reminderDays = $state<number[]>([]);
  let reminderTime = $state('16:00');
  let managerEmailInput = $state('');
  let managerNameInput = $state('');
  let bambooTitleInput = $state('');
  // Jira keys are stored as a Vec<String> server-side. The form binds a
  // comma-separated string for ergonomics; backend tokenizes + uppercases
  // on save (same path the wizard uses).
  let jiraKeysInput = $state('');

  // Mail tab (Phase 2.9b). All four bind directly to the wire shape — no
  // form-state vs settings-state split needed because every control is
  // either a dropdown or a radio.
  let mailSendMode = $state<MailSendMode>('gmail');
  let mailBodyFormat = $state<MailBodyFormat>('clean-text');
  let mailNativeHtml = $state(false);
  let mailOutlookFlavor = $state<OutlookFlavor>('business');
  let mailBodyDelivery = $state<MailBodyDelivery>('prefilled');

  // Phase 2.8+ Colorful Labels. Bound to the checkbox in the Theme tab
  // (below the Custom editor block, but outside the Custom-only
  // conditional — it's a labels preference that applies under every
  // theme). Read on load and round-tripped on save so unrelated edits
  // (e.g. reminder time) never silently clobber the value.
  let colorfulLabels = $state(false);

  // Phase 3c Slice 4 — Task list display prefs. Bound to the three
  // checkboxes in the Tasks tab. Defaults mirror the Rust
  // TaskListSettings::default() impl so an existing settings.json
  // upgrades cleanly.
  let taskShowCompleted = $state(true);
  let taskOpenTasksFirst = $state(true);
  let taskShowCompletedTimestamp = $state(false);
  let taskHideTaskList = $state(false);
  let taskAutoRolloverEnabled = $state(true);

  // Rebuild-task-index state — mirrors isRebuildingIndex from the
  // Labels tab. `taskRebuildReceipt` renders inline in the tab body
  // after a successful rebuild so the user sees what changed.
  let isRebuildingTasks = $state(false);
  let taskRebuildError = $state('');
  let taskRebuildReceipt = $state<{
    filesScanned: number;
    tasksScanned: number;
    entriesBackfilled: number;
    entriesPruned: number;
    tasksSweptForward: number;
    durationMs: number;
    failedFiles: string[];
  } | null>(null);

  // ---------- Custom theme editor (Phase 2.8 — Slice 4) ----------
  //
  // The Custom radio surfaces a 12-token editor (3 bg, 3 text, 2 borders,
  // 4 accents incl. sapphire). State lives here so navigating away (Cancel
  // or Done) can revert / persist the live preview cleanly.
  //
  //   - customEditor: the working palette the user is editing.
  //   - customEditorBase: 'dark' | 'light' — passed to deriveTokens so the
  //     pipeline picks the right per-base branches (text walks, focus host,
  //     btn-shadow, etc.). Inferred from bgBase luminance on each rebuild
  //     so a user editing a light-seeded palette dark gets a coherent
  //     theme even if they don't re-seed.
  //   - editorSeeded: false until the user activates Custom for the first
  //     time this mount. Drives the inline first-activation hint.
  //   - seededFromLabel: 'Dark' / 'Light' for the hint copy.
  //   - persistedCustomTheme: the on-disk payload (if any) — used by
  //     Cancel to restore live-preview state and by initial activation to
  //     re-load the user's last-saved palette instead of re-seeding.
  let customEditor = $state<CustomTheme | null>(null);
  let customEditorBase = $state<ThemeBase>('dark');
  let editorSeeded = $state(false);
  let seededFromLabel = $state<'Dark' | 'Light'>('Dark');
  let persistedCustomTheme = $state<CustomTheme | null>(null);
  // True once the user has made any edit (typing into a token input, an
  // import, or a Reset-to-{Light,Dark}) since the editor was last seeded /
  // persisted. Drives two no-clobber guards: re-clicking Custom while
  // already on Custom won't re-seed; Cancel after an Import warns before
  // discarding. Resets to false on Done (save) and on Cancel (after the
  // user confirms a discard).
  let customEditorDirty = $state(false);
  // True between a successful Import and the next Done/Cancel. Pairs with
  // customEditorDirty so the cancel() guard knows "this dirty state came
  // from an import the user explicitly clicked" rather than ad-hoc typing.
  let importPending = $state(false);

  // Slice 6 — when bg-surface lands in the ambiguous OKLCH-L band (0.50 –
  // 0.60) the auto-inferred base flips between light and dark on tiny edits
  // and the user can't get a stable theme. forceBaseOverride lets them pin
  // it. null = use whatever inferBase() decides. When the surface moves out
  // of the ambiguous band we leave the override in place (it'll silently
  // win against an unambiguous inference too) so quick A/B tweaks don't
  // lose the user's choice.
  let forceBaseOverride = $state<ThemeBase | null>(null);

  // Stable, ordered metadata for the editor — keys match CustomTheme fields
  // 1:1 so the per-row markup can be a single each loop instead of 12 hand-
  // rolled blocks. Order is the same as the slice plan: 3 bg, 3 text, 2
  // borders, 4 accents.
  type TokenKey = keyof CustomTheme;
  type TokenMeta = { key: TokenKey; label: string; hint: string };
  type TokenSection = { title: string; tokens: TokenMeta[] };
  const TOKEN_SECTIONS: TokenSection[] = [
    {
      title: 'Backgrounds',
      tokens: [
        {
          key: 'bgBase',
          label: 'Base',
          hint:
            'Page background behind every surface — the body color at window edges and behind cards.',
        },
        {
          key: 'bgSurface',
          label: 'Surface',
          hint:
            'Cards, inputs, modal header bars, and the main content slab on /summary and /journal.',
        },
        {
          key: 'bgElevated',
          label: 'Elevated',
          hint:
            'Hovered rows, dropdowns, popup card fills (Help, Nerds Only, Label Details, all modals), the Tip bubble fill, and the loading overlay’s scrim card.',
        },
      ],
    },
    {
      title: 'Text',
      tokens: [
        {
          key: 'textPrimary',
          label: 'Primary',
          hint: 'Body copy, headings, label chip text, and modal title text.',
        },
        {
          key: 'textSecondary',
          label: 'Secondary',
          hint:
            'Subtitles, Tip bubble body, modal close-button color, and the bullet glyph in the markdown editor.',
        },
        {
          key: 'textMuted',
          label: 'Muted',
          hint:
            'Captions, placeholder text, hint copy, and the scrollbar thumb on every scrollable surface.',
        },
      ],
    },
    {
      title: 'Borders',
      tokens: [
        {
          key: 'borderStructural',
          label: 'Structural',
          hint:
            'Dividers between sections, form-field outlines, modal card borders, and the chip outlines when Colorful Labels is off.',
        },
        {
          key: 'borderDecorative',
          label: 'Decorative',
          hint:
            'Accent rules under section titles + the chip-and-name dividers at the top of popups.',
        },
      ],
    },
    {
      title: 'Accents',
      tokens: [
        {
          key: 'accentPrimary',
          label: 'Primary (orange)',
          hint:
            'Brand button fill, focus ring, week stripe, day-pill active state, Noot’s reminder dots, and the loading spinner accent ring.',
        },
        {
          key: 'accentGreen',
          label: 'Green',
          hint: 'Save / Confirm / Open Draft button fills and success affordances.',
        },
        {
          key: 'accentPink',
          label: 'Pink',
          hint:
            'Cancel / Delete button fills, error tint backgrounds, and the invalid-input border on hex fields.',
        },
        {
          key: 'btnSapphire',
          label: 'Sapphire',
          hint: 'Sapphire button variant — used on the onboarding wizard’s secondary actions.',
        },
      ],
    },
  ];

  // Parse anything CSS-valid (#hex, rgb, rgba, hsl, color-mix, named) and
  // flatten to a 6-digit hex string. Used when seeding the editor from
  // computed CSS variables — the shipping rules use rgba for borders and
  // we need to round-trip through hex for the editor's input shape.
  function colorToHex6(input: string): string | null {
    if (!input) return null;
    const trimmed = input.trim();
    if (!trimmed) return null;
    try {
      const parsed = parse(trimmed);
      if (!parsed) return null;
      const hex = formatHex(parsed);
      if (!hex) return null;
      // formatHex sometimes returns 8-digit hex when alpha is present.
      // Truncate to 6 digits (drop alpha) so the value satisfies the
      // editor's `^#[0-9a-fA-F]{6}$` pattern and the Rust validator.
      return hex.length === 9 ? hex.slice(0, 7).toLowerCase() : hex.toLowerCase();
    } catch {
      return null;
    }
  }

  // Read the 12 primary tokens off the live :root computed style and
  // convert each to hex6. Falls back to the matching SHIPPING constant
  // when a token resolves to something culori can't parse — keeps the
  // first-activation flow from showing empty inputs.
  function seedFromComputedTheme(base: ThemeBase): CustomTheme {
    const fallback =
      base === 'dark' ? SHIPPING_DARK_PRIMARIES : SHIPPING_LIGHT_PRIMARIES;
    if (typeof document === 'undefined') {
      // SSR safety: emit the shipping primaries flattened to hex6.
      return primariesToCustom(fallback);
    }
    const cs = getComputedStyle(document.documentElement);
    const read = (cssVar: string, fb: string): string => {
      const raw = cs.getPropertyValue(cssVar);
      return colorToHex6(raw) ?? colorToHex6(fb) ?? '#000000';
    };
    return {
      bgBase: read('--bg-base', fallback.bgBase),
      bgSurface: read('--bg-surface', fallback.bgSurface),
      bgElevated: read('--bg-elevated', fallback.bgElevated),
      textPrimary: read('--text-primary', fallback.textPrimary),
      textSecondary: read('--text-secondary', fallback.textSecondary),
      textMuted: read('--text-muted', fallback.textMuted),
      borderStructural: read('--border-structural', fallback.borderStructural),
      borderDecorative: read('--border-decorative', fallback.borderDecorative),
      accentPrimary: read('--accent-primary', fallback.accentPrimary),
      accentGreen: read('--accent-green', fallback.accentGreen),
      accentPink: read('--accent-pink', fallback.accentPink),
      btnSapphire: read('--btn-sapphire', fallback.btnSapphire),
    };
  }

  // Coerce a PrimaryTokens (shipping constants use rgba for borders) into
  // a CustomTheme (every field hex6). Used as the SSR fallback path above.
  function primariesToCustom(p: PrimaryTokens): CustomTheme {
    return {
      bgBase: colorToHex6(p.bgBase) ?? '#000000',
      bgSurface: colorToHex6(p.bgSurface) ?? '#000000',
      bgElevated: colorToHex6(p.bgElevated) ?? '#000000',
      textPrimary: colorToHex6(p.textPrimary) ?? '#000000',
      textSecondary: colorToHex6(p.textSecondary) ?? '#000000',
      textMuted: colorToHex6(p.textMuted) ?? '#000000',
      borderStructural: colorToHex6(p.borderStructural) ?? '#000000',
      borderDecorative: colorToHex6(p.borderDecorative) ?? '#000000',
      accentPrimary: colorToHex6(p.accentPrimary) ?? '#000000',
      accentGreen: colorToHex6(p.accentGreen) ?? '#000000',
      accentPink: colorToHex6(p.accentPink) ?? '#000000',
      btnSapphire: colorToHex6(p.btnSapphire) ?? '#000000',
    };
  }

  // Hex6 validator — matches the Rust deserializer's regex.
  const HEX6 = /^#[0-9a-fA-F]{6}$/;
  function isValidHex6(s: string): boolean {
    return HEX6.test(s.trim());
  }

  // Infer 'dark' | 'light' from the bgBase luminance. Cheap heuristic, but
  // accurate for any realistic theme — light themes have light bg-base
  // (cream, white) and dark themes have dark bg-base. Lets deriveTokens
  // pick the right branch without persisting a separate base field.
  function inferBase(bgBaseHex: string): ThemeBase {
    const c = parse(bgBaseHex);
    if (!c || c.mode !== 'rgb') return 'dark';
    // Quick perceptual approximation — sum of channels. Mid-line at 1.5
    // because each channel is 0..1 in culori's RGB.
    const sum = (c.r ?? 0) + (c.g ?? 0) + (c.b ?? 0);
    return sum >= 1.5 ? 'light' : 'dark';
  }

  // ---- Live-preview pipeline ----
  //
  // Whenever the editor or its base changes, re-derive and re-apply the
  // override token set on :root. Debounced by a microtask + 100ms timer
  // so a fast typist doesn't trigger 30 reflows per keystroke.
  //
  // Skipped entirely when currentTheme !== 'custom' so toggling away from
  // Custom restores the Light/Dark stylesheet cleanly.
  let derivePending: ReturnType<typeof setTimeout> | null = null;
  function schedulePreview(): void {
    if (derivePending) clearTimeout(derivePending);
    derivePending = setTimeout(() => {
      derivePending = null;
      runPreview();
    }, 100);
  }
  function runPreview(): void {
    if (currentTheme !== 'custom' || !customEditor) return;
    // Every primary must be a valid hex6 before we attempt derivation —
    // a malformed value would throw in culori's parse(). Show the inline
    // warning per-row instead.
    for (const key of TOKEN_KEYS) {
      if (!isValidHex6(customEditor[key])) return;
    }
    const primaries: PrimaryTokens = { ...customEditor };
    // forceBaseOverride wins when set (Slice 6 — user pinned the polarity
    // because their surface is in the ambiguous mid-grey band). Otherwise
    // fall back to the bgBase luminance heuristic.
    const base: ThemeBase = forceBaseOverride ?? inferBase(customEditor.bgBase);
    customEditorBase = base;
    try {
      const derived = deriveTokens(primaries, base);
      applyCustomTheme(derived);
    } catch {
      // Bad input slipped through; leave the previous derivation applied.
    }
  }
  const TOKEN_KEYS: TokenKey[] = [
    'bgBase', 'bgSurface', 'bgElevated',
    'textPrimary', 'textSecondary', 'textMuted',
    'borderStructural', 'borderDecorative',
    'accentPrimary', 'accentGreen', 'accentPink', 'btnSapphire',
  ];

  // $effect — fire schedulePreview whenever any editor field changes.
  $effect(() => {
    if (!customEditor) return;
    // Read every field so Svelte 5 tracks the full record. untrack on the
    // schedule call so the timer setup doesn't re-enter tracking.
    for (const key of TOKEN_KEYS) {
      // eslint-disable-next-line @typescript-eslint/no-unused-expressions
      customEditor[key];
    }
    untrack(() => schedulePreview());
  });

  // ---- Per-row AA contrast checks ----
  //
  // The most user-visible "did I just break my theme?" failures are:
  //   - text-primary on bg-surface < 4.5:1 (body copy unreadable)
  //   - text-secondary on bg-surface < 4.5:1
  //   - text-muted on bg-surface < 4.5:1 (it's 13px — no large-text exemption)
  //   - accent-primary on bg-base < 3:1 (focus ring SC 1.4.11)
  //   - accent-pink on bg-base < 3:1 (used decoratively but should still
  //     be distinguishable)
  //
  // Each warning fires only when the primary value is itself valid hex —
  // partial typing never paints "Contrast is 1.0:1" mid-input.
  type ContrastWarning = { ratio: number; targetLabel: string; minRatio: number };
  function checkContrast(tokenKey: TokenKey): ContrastWarning | null {
    if (!customEditor) return null;
    const v = customEditor[tokenKey];
    if (!isValidHex6(v)) return null;
    // Bail if dependent value is itself invalid — we'd be measuring against
    // garbage. The user will see warnings on the dependent rows instead.
    const surface = customEditor.bgSurface;
    const base = customEditor.bgBase;
    if (tokenKey === 'textPrimary' && isValidHex6(surface)) {
      const r = contrastRatio(v, surface);
      if (r < 4.5) return { ratio: r, targetLabel: 'Surface', minRatio: 4.5 };
    }
    if (tokenKey === 'textSecondary' && isValidHex6(surface)) {
      const r = contrastRatio(v, surface);
      if (r < 4.5) return { ratio: r, targetLabel: 'Surface', minRatio: 4.5 };
    }
    if (tokenKey === 'textMuted' && isValidHex6(surface)) {
      const r = contrastRatio(v, surface);
      if (r < 4.5) return { ratio: r, targetLabel: 'Surface', minRatio: 4.5 };
    }
    if (tokenKey === 'accentPrimary' && isValidHex6(base)) {
      const r = contrastRatio(v, base);
      if (r < 3.0) return { ratio: r, targetLabel: 'Base', minRatio: 3.0 };
    }
    if (tokenKey === 'accentPink' && isValidHex6(base)) {
      const r = contrastRatio(v, base);
      if (r < 3.0) return { ratio: r, targetLabel: 'Base', minRatio: 3.0 };
    }
    return null;
  }

  // ---- Activation: switching the Theme radio TO 'custom' ----
  //
  // First-time activation:
  //   1. If the user already has a persisted custom_theme on disk, load it
  //      verbatim — they expect their last-saved palette back.
  //   2. Otherwise seed from the current Light/Dark computed style and
  //      show the inline "Started from your current {Dark|Light} theme"
  //      hint until the user makes the first edit.
  function activateCustom(): void {
    const fromLabel: 'Dark' | 'Light' = currentTheme === 'light' ? 'Light' : 'Dark';
    // Don't clobber unsaved edits. If the editor is already populated
    // (e.g. the user clicked away to Light to compare, then back to
    // Custom) keep their working palette in place. Only seed when there's
    // no editor state at all.
    if (customEditor) {
      // Already populated — preserve the working palette. Whether it's
      // dirty or matches the persisted theme, the user has been here
      // before in this session.
      currentTheme = 'custom';
      runPreview();
      return;
    }
    if (persistedCustomTheme) {
      customEditor = { ...persistedCustomTheme };
      customEditorBase = inferBase(customEditor.bgBase);
      editorSeeded = false; // not a "fresh seed" — it's their saved theme.
      customEditorDirty = false;
    } else {
      const seedBase: ThemeBase = currentTheme === 'light' ? 'light' : 'dark';
      customEditor = seedFromComputedTheme(seedBase);
      customEditorBase = seedBase;
      seededFromLabel = fromLabel;
      editorSeeded = true;
      customEditorDirty = false;
    }
    // Reset the Slice 6 force-base override on every fresh activation —
    // the new payload (or seeded state) gets to use plain inference first.
    forceBaseOverride = null;
    currentTheme = 'custom';
    // Apply preview immediately — don't wait for the debounce on first paint.
    runPreview();
  }

  // ---------- Custom theme toolbar (Phase 2.8 — Slice 5) ----------
  //
  // Reset-to-{Light,Dark} flatten the shipping primary table back into the
  // editor (live-applies via the existing $effect → schedulePreview path).
  // Export / Import use the @tauri-apps/plugin-fs commands. Both surface
  // their result inline via toolbarStatus — a tiny status line that auto-
  // clears the "Saved." confirmation after 2s but pins errors until the
  // next attempt so the user sees what went wrong.

  type ToolbarStatus =
    | { kind: 'none' }
    | { kind: 'success'; message: string }
    | { kind: 'error'; message: string };
  let toolbarStatus = $state<ToolbarStatus>({ kind: 'none' });
  let toolbarStatusTimer: ReturnType<typeof setTimeout> | null = null;

  function setToolbarStatus(next: ToolbarStatus, autoClearMs: number | null = null): void {
    if (toolbarStatusTimer) {
      clearTimeout(toolbarStatusTimer);
      toolbarStatusTimer = null;
    }
    toolbarStatus = next;
    if (autoClearMs !== null) {
      toolbarStatusTimer = setTimeout(() => {
        toolbarStatus = { kind: 'none' };
        toolbarStatusTimer = null;
      }, autoClearMs);
    }
  }

  // Last attempted export payload — held so the Retry affordance on a
  // failed save can re-issue the write without re-opening the dialog.
  let lastExportAttempt = $state<{ path: string; contents: string } | null>(null);


  async function openColorIdeas(): Promise<void> {
    try {
      await openUrl('https://htmlcolorcodes.com/');
    } catch (err) {
      setToolbarStatus({ kind: 'error', message: `Couldn't open browser: ${String(err)}` });
    }
  }

  // Build the canonical on-disk payload from the current editor state.
  // Mirrors the schema documented in the slice plan: $schema is the only
  // version identifier (no separate "version" key); base is inferred from
  // bgBase luminance; name/author are v1 stubs.
  function buildExportPayload(): string | null {
    if (!customEditor) return null;
    const base: ThemeBase = inferBase(customEditor.bgBase);
    const payload = {
      $schema: 'captheme/v1' as const,
      name: 'Custom',
      author: null as string | null,
      base,
      tokens: {
        bgBase: customEditor.bgBase,
        bgSurface: customEditor.bgSurface,
        bgElevated: customEditor.bgElevated,
        textPrimary: customEditor.textPrimary,
        textSecondary: customEditor.textSecondary,
        textMuted: customEditor.textMuted,
        borderStructural: customEditor.borderStructural,
        borderDecorative: customEditor.borderDecorative,
        accentPrimary: customEditor.accentPrimary,
        accentGreen: customEditor.accentGreen,
        accentPink: customEditor.accentPink,
        btnSapphire: customEditor.btnSapphire,
      },
    };
    return JSON.stringify(payload, null, 2) + '\n';
  }

  function defaultExportFilename(): string {
    // v1: theme name is hard-coded "Custom" in the payload. Use it (lower-
    // cased) in the filename too, alongside today's date, so multiple
    // exports from one editing session don't collide.
    const d = new Date();
    const yyyy = d.getFullYear();
    const mm = String(d.getMonth() + 1).padStart(2, '0');
    const dd = String(d.getDate()).padStart(2, '0');
    return `custom-${yyyy}-${mm}-${dd}.captheme.json`;
  }

  async function exportTheme(): Promise<void> {
    if (!customEditor) {
      setToolbarStatus({ kind: 'error', message: 'Nothing to export — Custom is not active.' });
      return;
    }
    // Validate up front. If we let the dialog open on broken input the user
    // would pick a destination only to see a hex-format error.
    for (const key of TOKEN_KEYS) {
      if (!isValidHex6(customEditor[key])) {
        setToolbarStatus({
          kind: 'error',
          message: `Can't export — "${key}" is not a 6-digit hex color.`,
        });
        return;
      }
    }
    const contents = buildExportPayload();
    if (!contents) {
      setToolbarStatus({ kind: 'error', message: 'Nothing to export.' });
      return;
    }
    try {
      const path = await saveDialog({
        defaultPath: defaultExportFilename(),
        filters: [
          { name: "Captain's Log Theme", extensions: ['captheme.json'] },
        ],
      });
      if (!path) {
        // User cancelled the dialog — no status update, just bail.
        return;
      }
      lastExportAttempt = { path, contents };
      await writeTextFile(path, contents);
      lastExportAttempt = null;
      setToolbarStatus({ kind: 'success', message: 'Saved.' }, 2000);
    } catch (err) {
      setToolbarStatus({ kind: 'error', message: `Couldn't save theme: ${String(err)}` });
    }
  }

  async function retryExport(): Promise<void> {
    if (!lastExportAttempt) {
      // Lost context (e.g. user navigated tabs since the failure) — fall
      // back to opening the save dialog again.
      await exportTheme();
      return;
    }
    try {
      await writeTextFile(lastExportAttempt.path, lastExportAttempt.contents);
      lastExportAttempt = null;
      setToolbarStatus({ kind: 'success', message: 'Saved.' }, 2000);
    } catch (err) {
      setToolbarStatus({ kind: 'error', message: `Couldn't save theme: ${String(err)}` });
    }
  }

  // Strict validator for an incoming .captheme.json. Returns an error
  // string on the first problem found, or null on success. The shape is
  // intentionally narrow: unknown keys are tolerated (forward-compat for
  // future "name"/"author" fields) but the 12 tokens + $schema + base
  // discriminator are non-negotiable.
  function validateImportedTheme(
    raw: unknown,
  ): { ok: true; base: ThemeBase; tokens: CustomTheme } | { ok: false; error: string } {
    if (typeof raw !== 'object' || raw === null) {
      return { ok: false, error: 'File is not a JSON object.' };
    }
    const obj = raw as Record<string, unknown>;
    if (obj.$schema !== 'captheme/v1') {
      return { ok: false, error: 'Unsupported theme file version.' };
    }
    if (obj.base !== 'light' && obj.base !== 'dark') {
      return { ok: false, error: 'Missing or invalid "base" — expected "light" or "dark".' };
    }
    const tokens = obj.tokens;
    if (typeof tokens !== 'object' || tokens === null) {
      return { ok: false, error: 'Missing "tokens" object.' };
    }
    const t = tokens as Record<string, unknown>;
    const out = {} as CustomTheme;
    for (const key of TOKEN_KEYS) {
      const v = t[key];
      if (typeof v !== 'string') {
        return { ok: false, error: `Missing required token: ${key}` };
      }
      if (!HEX6.test(v)) {
        return { ok: false, error: `Invalid hex format for ${key}: ${v}` };
      }
      out[key] = v.toLowerCase();
    }
    return { ok: true, base: obj.base as ThemeBase, tokens: out };
  }

  async function importTheme(): Promise<void> {
    try {
      const picked = await open({
        multiple: false,
        directory: false,
        filters: [
          { name: "Captain's Log Theme", extensions: ['captheme.json'] },
        ],
      });
      if (!picked || typeof picked !== 'string') {
        // Cancelled (or multi-select shape we explicitly disabled).
        return;
      }
      let text: string;
      try {
        text = await readTextFile(picked);
      } catch (err) {
        setToolbarStatus({
          kind: 'error',
          message: `Couldn't read file: ${String(err)}`,
        });
        return;
      }
      let parsed: unknown;
      try {
        parsed = JSON.parse(text);
      } catch (err) {
        setToolbarStatus({
          kind: 'error',
          message: `Not valid JSON: ${String(err)}`,
        });
        return;
      }
      const result = validateImportedTheme(parsed);
      if (!result.ok) {
        setToolbarStatus({ kind: 'error', message: result.error });
        return;
      }
      // Replace editor state. Reuse the activateCustom code path's
      // conventions: set base, drop the "Started from your current X" hint
      // (the user just told us they want THIS theme, not the seeded one),
      // and fire runPreview eagerly so the swap reads as instant.
      customEditor = { ...result.tokens };
      customEditorBase = result.base;
      editorSeeded = false;
      // An import is a deliberate edit — mark dirty + flag importPending so
      // the cancel() guard can warn before discarding the loaded theme.
      customEditorDirty = true;
      importPending = true;
      // Imported file declares its own base — honor that authoritatively,
      // overriding any in-session force-base picker state.
      forceBaseOverride = result.base;
      if (currentTheme !== 'custom') currentTheme = 'custom';
      runPreview();
      // Wording reminds the user that the load is preview-only until they
      // commit it with Done — otherwise Cancel discards it.
      setToolbarStatus(
        { kind: 'success', message: 'Theme loaded — click Done to keep it.' },
        4000,
      );
    } catch (err) {
      setToolbarStatus({ kind: 'error', message: `Import failed: ${String(err)}` });
    }
  }

  // ---- Slice 6 — ambiguous-base + saturated-surface diagnostics ----
  //
  // Both flags read off the live editor's bgSurface. They're $derived
  // (recomputed only when the editor's surface field changes) so the UI
  // bands appear/disappear in step with edits without firing on every
  // unrelated rerender. Each check is guarded by isValidHex6 — a half-
  // typed hex shouldn't flash a banner.
  const surfaceIsAmbiguous = $derived.by(() => {
    if (!customEditor) return false;
    const s = customEditor.bgSurface;
    if (!isValidHex6(s)) return false;
    return isAmbiguousBaseSurface(s);
  });
  const surfaceIsSaturated = $derived.by(() => {
    if (!customEditor) return false;
    const s = customEditor.bgSurface;
    if (!isValidHex6(s)) return false;
    return isSaturatedSurface(s);
  });

  function setForceBase(base: ThemeBase): void {
    forceBaseOverride = base;
    runPreview();
  }

  // Light client-side check: looks_like_an_email, not the full RFC 5322
  // monster. Empty is fine (it disables the Send button); only a non-empty
  // value that fails the shape gets the warning. Backend re-trims and persists.
  const managerEmailLooksValid = $derived.by(() => {
    const v = managerEmailInput.trim();
    if (v === '') return true;
    return /.+@.+\..+/.test(v);
  });
  // Same loose shape for the user's own email — warn-don't-block.
  const userEmailLooksValid = $derived.by(() => {
    const v = userEmailInput.trim();
    if (v === '') return true;
    return /.+@.+\..+/.test(v);
  });

  // ---------- Label Manager (Phase 3a — Slice 8) ----------
  //
  // The Labels tab walks every weekly file to rebuild the in-memory index
  // on its FIRST click per Settings session. Subsequent clicks within the
  // same session reuse the already-loaded list (no walk). Closing and
  // re-opening Settings resets the per-session guard, so the user gets a
  // fresh scan after they've been editing notes elsewhere.

  // List of labels currently shown in the Labels tab.
  let labels = $state<LabelEntry[]>([]);
  // True between user clicking Labels for the first time and the rebuild
  // resolving. Drives the loading overlay inside the panel.
  let isRebuildingIndex = $state(false);
  // Per-session guard — flips true on the first successful rebuild. Reset
  // naturally when Settings unmounts (state lives on the component).
  let labelIndexRebuiltThisSession = $state(false);
  // Rebuild / fetch error surfaced inline so the user can retry without
  // closing Settings.
  let labelsError = $state('');
  // Free-text filter (case-insensitive substring on label name).
  let labelFilter = $state('');
  // Currently-selected label for the details modal (wired in Slice 10).
  let selectedLabel = $state<LabelEntry | null>(null);

  // Theme nonce — re-read on any data-theme/data-cap-custom-keys change so
  // chip colors recompute when the user flips themes mid-Settings session.
  // Matches the pattern in LabelInput.svelte.
  let labelThemeNonce = $state(0);

  function currentThemeMode(): 'light' | 'dark' | 'custom' {
    if (typeof document === 'undefined') return 'dark';
    const root = document.documentElement;
    if (root.hasAttribute('data-cap-custom-keys')) return 'custom';
    const t = root.getAttribute('data-theme');
    return t === 'light' ? 'light' : 'dark';
  }

  function currentBgSurface(): string | undefined {
    if (typeof document === 'undefined') return undefined;
    const v = getComputedStyle(document.documentElement)
      .getPropertyValue('--bg-surface')
      .trim();
    return v.length > 0 ? v : undefined;
  }

  function labelChipStyle(entry: LabelEntry): string {
    // Touch the nonce so Svelte tracks chip styles against theme changes —
    // the DOM reads inside chipStyleFor are otherwise invisible to the
    // reactivity system.
    // eslint-disable-next-line @typescript-eslint/no-unused-expressions
    labelThemeNonce;
    const chip: ChipEntry = { name: entry.name, color: entry.color ?? null };
    return chipStyleFor(chip, colorfulLabels, currentThemeMode(), currentBgSurface());
  }

  async function fetchLabels(): Promise<void> {
    const list = await invoke<LabelEntry[]>('get_labels');
    labels = list ?? [];
  }

  async function rebuildLabelIndex(): Promise<void> {
    labelsError = '';
    isRebuildingIndex = true;
    // Flip the session gate SYNCHRONOUSLY before any await. A re-click on
    // the Labels tab during the rebuild's filesystem walk would otherwise
    // re-enter this function and fire a duplicate rebuild_label_index
    // (the rebuild can take seconds on large journals; tab-switch-and-back
    // is realistic). If the rebuild ultimately fails, the catch block
    // clears the gate so the next click retries.
    labelIndexRebuiltThisSession = true;
    try {
      await invoke('rebuild_label_index');
      await fetchLabels();
    } catch (err) {
      labelsError = String(err);
      // Clear the gate so a fresh click triggers a retry. The user might
      // also surface a Retry button bound to rebuildLabelIndex directly.
      labelIndexRebuiltThisSession = false;
    } finally {
      isRebuildingIndex = false;
    }
  }

  type TaskRebuildResult = {
    filesScanned: number;
    tasksScanned: number;
    entriesBackfilled: number;
    entriesPruned: number;
    tasksSweptForward: number;
    durationMs: number;
    failedFiles: string[];
  };

  // Slice 4 — Rebuild task index. Same posture as rebuildLabelIndex:
  // the button is disabled while in-flight, errors surface inline in
  // the Tasks tab, and a success receipt renders under the button
  // reporting what changed on disk (backfills + prunes + failed
  // files). Clicking again after success just re-runs the rebuild;
  // there's no session gate because tasks aren't wired to the "auto-
  // rebuild on tab open" pattern the Labels tab uses.
  async function rebuildTaskIndex(): Promise<void> {
    if (isRebuildingTasks) return;
    taskRebuildError = '';
    taskRebuildReceipt = null;
    isRebuildingTasks = true;
    try {
      const result = await invoke<TaskRebuildResult>('rebuild_task_completions_index');
      taskRebuildReceipt = result;
    } catch (err) {
      taskRebuildError = String(err);
    } finally {
      isRebuildingTasks = false;
    }
  }

  async function onLabelsTabClicked(): Promise<void> {
    activeTab = 'labels';
    // Gate is "in-flight OR done" — both states mean "don't start another
    // rebuild." The gate flips true synchronously inside rebuildLabelIndex
    // before any await, so a tab-switch-and-back during the walk hits this
    // branch and the overlay (driven by isRebuildingIndex, which is also
    // true while in-flight) keeps showing until the first walk completes.
    if (labelIndexRebuiltThisSession || isRebuildingIndex) {
      // Already rebuilt (or rebuilding) this session — just refresh the
      // list in case a newer label landed via another path. Skip the
      // refresh if a rebuild is in-flight; it will populate the list
      // itself when it finishes.
      if (!isRebuildingIndex) {
        try {
          await fetchLabels();
        } catch (err) {
          labelsError = String(err);
        }
      }
      return;
    }
    await rebuildLabelIndex();
  }

  // Slice 10 — modal wiring. onLabelMutated re-fetches get_labels and
  // updates the popup's selectedLabel reference so:
  //   - Rename: the modal stays open with the new name (we re-resolve
  //     selectedLabel by the latest name held in the popup — the popup
  //     mirrors a working `current.name` and calls onLabelMutated after
  //     a successful rename, by which point its prefill already shows
  //     the new value).
  //   - Delete cascade: the popup has already called onClose() before
  //     firing onLabelMutated(), so selectedLabel is null when we land
  //     here. fetchLabels just refreshes the list.
  //   - Color set/reset: the chip in the row needs to repaint; the
  //     selected label reference also has to track the new color so
  //     the popup's swatch keeps matching the row.
  //
  // The popup's `current.name` isn't observable from here — we re-resolve
  // by matching the OLD selectedLabel.name against the new list first
  // (covers color changes + delete). If we don't find it (rename case)
  // we fall back to a name-set diff: any new entry not in the previous
  // list MUST be the renamed-to entry, since rename is the only mutation
  // that adds a name and removes one in a single operation.
  async function onLabelMutated(): Promise<void> {
    const prevName = selectedLabel?.name ?? null;
    const prevNames = new Set(labels.map((l) => l.name));
    try {
      await fetchLabels();
    } catch (err) {
      labelsError = String(err);
      return;
    }
    // Prune the bulk-selection set — labels renamed or deleted via the
    // Details modal should drop out so they don't ghost in the toolbar
    // count or resurface in a subsequent bulk op.
    const currentNames = new Set(labels.map((l) => l.name));
    let pruned: Set<string> | null = null;
    for (const name of selectedLabelNames) {
      if (!currentNames.has(name)) {
        if (!pruned) pruned = new Set(selectedLabelNames);
        pruned.delete(name);
      }
    }
    if (pruned) selectedLabelNames = pruned;
    if (!selectedLabel) return;
    // Try same-name lookup first — covers color updates + the "no-op"
    // refresh after rename when the popup is still open with the OLD name
    // momentarily (shouldn't happen — popup updates current.name BEFORE
    // calling onLabelMutated — but defend anyway).
    const sameName = labels.find((l) => l.name === prevName);
    if (sameName) {
      selectedLabel = sameName;
      return;
    }
    // Same-name gone — either deleted (popup already closed itself, but
    // be defensive) or renamed. Look for a label that wasn't there before.
    const newcomer = labels.find((l) => !prevNames.has(l.name));
    if (newcomer) {
      selectedLabel = newcomer;
      return;
    }
    // Couldn't reconcile — clear so we don't render the modal against a
    // stale row.
    selectedLabel = null;
  }

  function closeLabelModal(): void {
    selectedLabel = null;
  }

  // Slice 10 — Esc closes the modal first (modal's own window listener
  // owns that path and stops propagation by preventDefault'ing), then
  // closes Settings. This page-level handler runs only when the modal
  // isn't open. We listen on window so focus inside any input still
  // routes here.
  function onSettingsKeydown(e: KeyboardEvent): void {
    if (e.key !== 'Escape') return;
    if (selectedLabel) return; // modal's own handler owns this Esc.
    e.preventDefault();
    void cancel();
  }

  $effect(() => {
    if (typeof window === 'undefined') return;
    window.addEventListener('keydown', onSettingsKeydown);
    return () => window.removeEventListener('keydown', onSettingsKeydown);
  });

  // Sorted + filtered view. Count desc, then name asc on ties.
  const visibleLabels = $derived.by(() => {
    const q = labelFilter.trim().toLowerCase();
    const filtered = q === ''
      ? labels.slice()
      : labels.filter((l) => l.name.toLowerCase().includes(q));
    filtered.sort((a, b) => {
      if (b.count !== a.count) return b.count - a.count;
      return a.name.localeCompare(b.name);
    });
    return filtered;
  });

  // -------------------------------------------------------------------------
  // Phase 3a Slice 2 — multi-select + bulk delete / bulk merge
  // -------------------------------------------------------------------------

  // Selection is a Set of label names, not indices — resilient to filter
  // changes, sort changes, and mutations from the Details modal. Prune on
  // every onLabelMutated so stale entries (renamed/deleted-out-from-under-us
  // via Details) don't linger.
  let selectedLabelNames = $state<Set<string>>(new Set());

  // Bulk-delete UI state.
  let showBulkDeleteConfirm = $state(false);
  let bulkDeleteInFlight = $state(false);

  // Bulk-merge UI state. Picker uses shared Modal (not ConfirmDialog)
  // because we need a disabled state on the primary action when no
  // canonical target has been chosen yet.
  let showBulkMergePicker = $state(false);
  let bulkMergeCanonical = $state<string | null>(null);
  let bulkMergeInFlight = $state(false);

  // Result / error banner. Persists above the list until the user acts
  // again or explicitly dismisses. Cleared whenever a new bulk op starts.
  let bulkOpMessage = $state('');
  let bulkOpError = $state('');

  // Derived count for cheap template reads without recomputing Set.size
  // in three places.
  const selectionCount = $derived(selectedLabelNames.size);

  // "Select all visible" checkbox state — true only when every visible
  // label is currently selected (partial selection reads as unchecked
  // for MVP; we don't set the indeterminate DOM property to keep this
  // simple. Two-state toggle: click to select all, click again to clear.)
  const allVisibleSelected = $derived(
    visibleLabels.length > 0 &&
      visibleLabels.every((l) => selectedLabelNames.has(l.name))
  );

  function toggleLabelSelection(name: string): void {
    // Svelte 5 tracks Set identity, not internal mutation — assign a new
    // Set so the reactive derivations recompute.
    const next = new Set(selectedLabelNames);
    if (next.has(name)) {
      next.delete(name);
    } else {
      next.add(name);
    }
    selectedLabelNames = next;
    // Clear any lingering result banner as soon as the user modifies
    // their selection — otherwise "Deleted 3 labels" text hangs around
    // while they build the next batch.
    bulkOpMessage = '';
    bulkOpError = '';
  }

  function toggleSelectAllVisible(): void {
    if (allVisibleSelected) {
      // Deselect only the currently-visible labels — keep any selection
      // that's outside the filter (rare but possible; user filters,
      // selects, unfilters, filters again, hits "clear filtered").
      const next = new Set(selectedLabelNames);
      for (const l of visibleLabels) next.delete(l.name);
      selectedLabelNames = next;
    } else {
      const next = new Set(selectedLabelNames);
      for (const l of visibleLabels) next.add(l.name);
      selectedLabelNames = next;
    }
    bulkOpMessage = '';
    bulkOpError = '';
  }

  function clearBulkSelection(): void {
    selectedLabelNames = new Set();
    bulkOpMessage = '';
    bulkOpError = '';
  }

  function startBulkDelete(): void {
    if (selectionCount === 0) return;
    bulkOpError = '';
    showBulkDeleteConfirm = true;
  }

  async function confirmBulkDelete(): Promise<void> {
    if (bulkDeleteInFlight) return;
    bulkDeleteInFlight = true;
    bulkOpError = '';
    const names = Array.from(selectedLabelNames);
    const failed: string[] = [];
    // Sequential — one Tauri command per label. Continue past failures
    // to match the per-label commands' locked posture (Phase 2.8b
    // decision #7: don't roll back on partial failure, surface what
    // couldn't be touched).
    for (const name of names) {
      try {
        await invoke('delete_label_cascade', { name });
      } catch (err) {
        failed.push(`${name} (${String(err)})`);
      }
    }
    const succeeded = names.length - failed.length;
    bulkOpMessage =
      failed.length === 0
        ? `Deleted ${succeeded} label${succeeded === 1 ? '' : 's'}.`
        : `Deleted ${succeeded} of ${names.length}. Failed: ${failed.join('; ')}.`;
    if (failed.length > 0) bulkOpError = bulkOpMessage;
    showBulkDeleteConfirm = false;
    bulkDeleteInFlight = false;
    selectedLabelNames = new Set();
    try {
      await fetchLabels();
    } catch (err) {
      labelsError = String(err);
    }
  }

  function startBulkMerge(): void {
    if (selectionCount < 2) return;
    // Default the canonical pick to the highest-count label so the
    // "obvious" answer is pre-selected; user can flip it before hitting
    // Merge. Ties broken by name asc (stable + predictable).
    const selectedList = labels.filter((l) => selectedLabelNames.has(l.name));
    selectedList.sort((a, b) => {
      if (b.count !== a.count) return b.count - a.count;
      return a.name.localeCompare(b.name);
    });
    bulkMergeCanonical = selectedList[0]?.name ?? null;
    bulkOpError = '';
    showBulkMergePicker = true;
  }

  function cancelBulkMerge(): void {
    if (bulkMergeInFlight) return;
    showBulkMergePicker = false;
    bulkMergeCanonical = null;
  }

  async function confirmBulkMerge(): Promise<void> {
    if (bulkMergeInFlight || !bulkMergeCanonical) return;
    const canonical = bulkMergeCanonical;
    const sources = Array.from(selectedLabelNames).filter(
      (n) => n !== canonical
    );
    if (sources.length === 0) {
      bulkOpError = 'Pick a canonical different from the merge sources.';
      return;
    }
    bulkMergeInFlight = true;
    bulkOpError = '';
    const failed: string[] = [];
    // rename_label auto-merges when the target name already exists
    // (Phase 2.8b behavior — dedup happens inside the Rust command).
    // Loop each source into canonical; continue past failures.
    for (const src of sources) {
      try {
        await invoke('rename_label', { oldName: src, newName: canonical });
      } catch (err) {
        failed.push(`${src} (${String(err)})`);
      }
    }
    const succeeded = sources.length - failed.length;
    bulkOpMessage =
      failed.length === 0
        ? `Merged ${succeeded} label${succeeded === 1 ? '' : 's'} into "${canonical}".`
        : `Merged ${succeeded} of ${sources.length} into "${canonical}". Failed: ${failed.join('; ')}.`;
    if (failed.length > 0) bulkOpError = bulkOpMessage;
    showBulkMergePicker = false;
    bulkMergeInFlight = false;
    bulkMergeCanonical = null;
    selectedLabelNames = new Set();
    try {
      await fetchLabels();
    } catch (err) {
      labelsError = String(err);
    }
  }

  function dismissBulkOpBanner(): void {
    bulkOpMessage = '';
    bulkOpError = '';
  }

  // ---------- Tab persistence ----------

  // Phase 3a — Settings now ALWAYS opens to General regardless of the
  // last-active value in localStorage (see initial activeTab assignment).
  // Reasoning: the Labels tab triggers a rebuild_label_index call (walks
  // every weekly file) on first click per Settings session — firing on
  // every Settings open just because the user last left that tab open
  // would be wasteful. We still persist the active tab below so mid-
  // session tab switches feel sticky; only the initial value on mount is
  // overridden.
  $effect(() => {
    try {
      localStorage.setItem(TAB_STORAGE_KEY, activeTab);
    } catch {
      // Quota exceeded or disabled — silent failure is fine.
    }
  });

  // ---------- Load ----------

  onMount(async () => {
    try {
      const s = await invoke<Settings>('get_settings');
      nameInput = s.userName ?? '';
      userEmailInput = s.userEmail ?? '';
      journalRootInput = s.journalRoot;
      originalJournalRoot = s.journalRoot;
      originalTheme = s.theme;
      currentTheme = s.theme;
      reminderEnabled = s.reminder.enabled;
      reminderDays = [...(s.reminder.daysOfWeek ?? [])];
      reminderTime = `${String(s.reminder.hour).padStart(2, '0')}:${String(s.reminder.minute).padStart(2, '0')}`;
      managerEmailInput = s.managerEmail ?? '';
      managerNameInput = s.managerName ?? '';
      bambooTitleInput = s.bambooTitle ?? '';
      jiraKeysInput = (s.jiraProjectKeys ?? []).join(', ');
      mailSendMode = s.mailSendMode ?? 'gmail';
      mailBodyFormat = s.mailBodyFormat ?? 'clean-text';
      mailNativeHtml = s.mailNativeHtml ?? false;
      mailOutlookFlavor = s.mailOutlookFlavor ?? 'business';
      mailBodyDelivery = s.mailBodyDelivery ?? 'prefilled';
      colorfulLabels = s.colorfulLabels ?? false;
      // Slice 4/5 — Task list toggles.
      taskShowCompleted = s.taskList?.showCompleted ?? true;
      taskOpenTasksFirst = s.taskList?.openTasksFirst ?? true;
      taskShowCompletedTimestamp = s.taskList?.showCompletedTimestamp ?? false;
      taskHideTaskList = s.taskList?.hideTaskList ?? false;
      taskAutoRolloverEnabled = s.taskList?.autoRolloverEnabled ?? true;
      // Phase 2.8 — load the persisted custom palette (if any) so toggling
      // into Custom restores the user's last-saved theme verbatim.
      persistedCustomTheme = s.customTheme ?? null;
      // If app boots already on Custom, the layout-level apply has set
      // data-theme to the seed base and applied inline overrides. Hydrate
      // the editor state so the Theme tab opens with the live colors
      // visible in its inputs.
      if (s.theme === 'custom' && persistedCustomTheme) {
        customEditor = { ...persistedCustomTheme };
        customEditorBase = inferBase(customEditor.bgBase);
        editorSeeded = false;
      }
    } catch (err) {
      loadError = String(err);
    } finally {
      loading = false;
    }
  });

  // ---------- Theme preview ----------

  function setTheme(t: Theme) {
    // Re-clicking the already-active radio is a no-op — without this guard,
    // activateCustom() would re-enter and (pre-fix) re-seed the editor,
    // wiping any unsaved edits the user just made.
    if (t === currentTheme) return;
    if (t === 'custom') {
      activateCustom();
      return;
    }
    currentTheme = t;
    // Leaving Custom — strip the inline override block so the chosen
    // Light/Dark CSS rules win cleanly.
    clearCustomTheme();
    document.documentElement.setAttribute('data-theme', t);
  }

  // ---------- Reminder day toggling ----------

  function toggleDay(day: number): void {
    if (reminderDays.includes(day)) {
      reminderDays = reminderDays.filter((d) => d !== day);
    } else {
      reminderDays = [...reminderDays, day].sort((a, b) => a - b);
    }
  }

  function isDaySelected(day: number): boolean {
    return reminderDays.includes(day);
  }

  // Hint shown when the reminder is enabled but no days are selected —
  // saving in that state persists a no-op (nothing fires until the user
  // picks at least one day).
  const remindersEmptyHint = $derived(
    reminderEnabled && reminderDays.length === 0
  );

  // ---------- External actions ----------

  async function openNotificationSettings() {
    try {
      await openUrl(
        'x-apple.systempreferences:com.apple.preference.notifications?id=com.prodigygame.captainslog'
      );
    } catch (err) {
      saveError = String(err);
    }
  }

  // Native Mac Mail mode needs Automation permission for Captain's Log to
  // drive Mail.app via AppleScript. The first send triggers the prompt;
  // if the user denied it, this URL jumps them straight to the Privacy
  // & Security > Automation pane where they can re-enable us.
  async function openAutomationSettings() {
    try {
      await openUrl(
        'x-apple.systempreferences:com.apple.preference.security?Privacy_Automation'
      );
    } catch (err) {
      saveError = String(err);
    }
  }

  // Journal-location hint flips when the user has staged a new root —
  // makes the "applies on Done" promise explicit without needing an extra
  // status line. Computed at render time and passed into PathPickerField's
  // `hint` slot.
  const journalLocationHint = $derived(
    journalRootInput !== originalJournalRoot
      ? 'The change applies as soon as you click Done — existing notes stay at the old location.'
      : 'Plain markdown on your machine.'
  );

  // ---------- Save ----------

  async function save() {
    saveError = '';
    saving = true;
    // Gate Custom saves on every primary parsing as hex6. The Rust side
    // also validates, but surfacing the error here lets the user fix it
    // without losing their other Settings edits.
    if (currentTheme === 'custom') {
      if (!customEditor) {
        saveError = 'Custom theme has no payload — pick a theme first.';
        saving = false;
        return;
      }
      for (const key of TOKEN_KEYS) {
        if (!isValidHex6(customEditor[key])) {
          saveError = `Custom theme: "${key}" is not a 6-digit hex color.`;
          saving = false;
          return;
        }
      }
    }
    try {
      const [hourStr, minuteStr] = reminderTime.split(':');
      // Always send the editor state if we have one — the Rust side
      // preserves it across Light/Dark switches (locked decision #1) so
      // a user can toggle away and back without losing their palette.
      const customThemePayload: CustomTheme | null = customEditor
        ? { ...customEditor }
        : persistedCustomTheme;
      await invoke('update_settings', {
        input: {
          userName: nameInput.trim() || null,
          userEmail: userEmailInput.trim() || null,
          journalRoot: journalRootInput,
          reminder: {
            enabled: reminderEnabled,
            daysOfWeek: [...reminderDays].sort((a, b) => a - b),
            hour: Number.parseInt(hourStr, 10),
            minute: Number.parseInt(minuteStr, 10)
          },
          theme: currentTheme,
          customTheme: customThemePayload,
          managerEmail: managerEmailInput.trim() || null,
          managerName: managerNameInput.trim() || null,
          bambooTitle: bambooTitleInput.trim() || null,
          jiraProjectKeys: jiraKeysInput.trim() ? [jiraKeysInput] : [],
          mailSendMode,
          mailBodyFormat,
          mailNativeHtml,
          mailOutlookFlavor,
          mailBodyDelivery,
          colorfulLabels,
          taskList: {
            showCompleted: taskShowCompleted,
            openTasksFirst: taskOpenTasksFirst,
            showCompletedTimestamp: taskShowCompletedTimestamp,
            hideTaskList: taskHideTaskList,
            autoRolloverEnabled: taskAutoRolloverEnabled
          }
        }
      });
      // Storage, reminder, and theme all hot-swap in-process — no restart needed.
      // Clear in-session dirty/import flags now that the payload is persisted.
      customEditorDirty = false;
      importPending = false;
      await goto('/');
    } catch (err) {
      saveError = String(err);
    } finally {
      saving = false;
    }
  }

  // Cancel-with-imported-theme guard. The old shape used window.confirm —
  // a native browser modal that broke the dim/blur consistency with the
  // rest of the app. Now: cancel() checks the guard condition; if hit,
  // it flips showCancelImportConfirm so the shared ConfirmDialog renders
  // (and performCancel() runs only when the user picks Discard). Otherwise
  // performCancel() runs straight through.
  let showCancelImportConfirm = $state(false);

  async function cancel() {
    if (importPending && customEditorDirty && currentTheme === 'custom') {
      showCancelImportConfirm = true;
      return;
    }
    await performCancel();
  }

  async function performCancel() {
    showCancelImportConfirm = false;
    // Revert any live theme preview before navigating away. Three flavors:
    //   1. Was Custom, now still Custom (user edited): restore the
    //      persisted custom_theme if any; otherwise drop overrides and
    //      fall back to originalTheme's base stylesheet.
    //   2. Was Light/Dark, now Custom (user toggled mid-edit): drop the
    //      inline overrides and switch data-theme back to originalTheme.
    //   3. Was X, now X with no edits: nothing to do.
    if (currentTheme === 'custom') {
      clearCustomTheme();
      if (originalTheme === 'custom' && persistedCustomTheme) {
        // Re-apply the on-disk palette so the rest of the app keeps the
        // Custom look the user came in with.
        try {
          const base = inferBase(persistedCustomTheme.bgBase);
          const derived = deriveTokens(
            { ...persistedCustomTheme } as PrimaryTokens,
            base,
          );
          document.documentElement.setAttribute('data-theme', base);
          applyCustomTheme(derived);
        } catch {
          document.documentElement.setAttribute('data-theme', 'dark');
        }
      } else {
        document.documentElement.setAttribute('data-theme', originalTheme);
      }
    } else if (currentTheme !== originalTheme) {
      // Was Custom or other base, now plain Light/Dark — restore original.
      clearCustomTheme();
      if (originalTheme === 'custom' && persistedCustomTheme) {
        try {
          const base = inferBase(persistedCustomTheme.bgBase);
          const derived = deriveTokens(
            { ...persistedCustomTheme } as PrimaryTokens,
            base,
          );
          document.documentElement.setAttribute('data-theme', base);
          applyCustomTheme(derived);
        } catch {
          document.documentElement.setAttribute('data-theme', 'dark');
        }
      } else {
        document.documentElement.setAttribute('data-theme', originalTheme);
      }
    }
    // Clear in-session dirty/import flags — the user is leaving Settings.
    customEditorDirty = false;
    importPending = false;
    await goto('/');
  }
</script>

{#if loading}
  <main class="loading">
    <p>Loading…</p>
  </main>
{:else if loadError}
  <main class="loading">
    <div class="card is-narrow error-card">
      <h2>Couldn't load settings.</h2>
      <p>{loadError}</p>
      <button class="btn btn-marble" onclick={() => goto('/')}>Back</button>
    </div>
  </main>
{:else}
  <main>
    <section class="page">
      <header>
        <h1>Settings</h1>
        <p class="subtitle">
          Tweak the bits of Captain's Log that make it feel like yours.
        </p>
      </header>

      <!-- Tab bar: <div> rather than <nav> because tablist is an
           interactive ARIA role and <nav> is non-interactive — Svelte's
           a11y linter rejects the combination. -->
      <div class="tabs" role="tablist" aria-label="Settings sections">
        {#each TABS as tab (tab.key)}
          <button
            type="button"
            class="tab"
            class:active={activeTab === tab.key}
            role="tab"
            aria-selected={activeTab === tab.key}
            aria-controls="panel-{tab.key}"
            id="tab-{tab.key}"
            onclick={() => {
              if (tab.key === 'labels') {
                void onLabelsTabClicked();
              } else {
                activeTab = tab.key;
              }
            }}
          >
            {tab.label}
          </button>
        {/each}
      </div>

      {#snippet managerEmailHint()}
        Pre-fills the To: line on the <strong>Send to manager</strong>
        button on the weekly summary. Leave blank and the button still
        works — it'll open a draft with a blank To: line.
      {/snippet}

      <!-- ============================== General ============================== -->
      {#if activeTab === 'general'}
        <div
          class="form"
          role="tabpanel"
          id="panel-general"
          aria-labelledby="tab-general"
        >
          <!--
            User Information — a single section holding every "who am I"
            input (you + your manager). Field-order groups you-fields
            first, then manager-fields; the InputField labels carry the
            grouping signal, so an intermediate "You" / "Manager"
            sub-heading would just be noise.
          -->
          <div class="section">
            <h2 class="section-title">User Information</h2>

            <InputField
              id="name"
              label="Name"
              placeholder="Chris"
              bind:value={nameInput}
              hint="Used in the reminder notification body."
            />

            <InputField
              id="user-email"
              label="Email"
              type="email"
              placeholder="you@prodigygame.com"
              autocomplete="email"
              bind:value={userEmailInput}
              warning={!userEmailLooksValid
                ? "That doesn't look like an email address. Save anyway if it's intentional."
                : undefined}
              hint={userEmailLooksValid
                ? 'Used to route the right Gmail account in Gmail mode, and as the sender in Mac Mail mode. Not shown to your manager.'
                : undefined}
            />

            <InputField
              id="bamboo-title"
              label="Job Title"
              placeholder="Staff QA Analyst"
              bind:value={bambooTitleInput}
              hint="As it appears on BambooHR. Used in your weekly email signature."
            />

            <InputField
              id="jira-keys"
              label="Jira Project Key(s)"
              placeholder="MAGE, LIVE"
              bind:value={jiraKeysInput}
              hint="Comma-separated. Captain's Log uppercases them on save."
            />

            <InputField
              id="manager-name"
              label="Manager Name"
              placeholder="Arthur"
              bind:value={managerNameInput}
              hint={'Used as the greeting in the email ("Hello Arthur,"). Leave blank for a plain "Hello,".'}
            />

            <InputField
              id="manager-email"
              label="Manager Email"
              type="email"
              placeholder="manager@prodigygame.com"
              autocomplete="email"
              bind:value={managerEmailInput}
              warning={!managerEmailLooksValid
                ? "That doesn't look like an email address. Save anyway if it's intentional."
                : undefined}
              hintSnippet={managerEmailLooksValid ? managerEmailHint : undefined}
            />
          </div>

          <!-- File Location -->
          <div class="section">
            <h2 class="section-title">File Location</h2>

            <PathPickerField
              id="root"
              label="Folder"
              bind:value={journalRootInput}
              hint={journalLocationHint}
            />
          </div>
        </div>
      {/if}

      <!-- ============================== Reminders ============================== -->
      {#if activeTab === 'reminders'}
        <div
          class="form"
          role="tabpanel"
          id="panel-reminders"
          aria-labelledby="tab-reminders"
        >
          <div class="section">
            <h2 class="section-title">Weekly Reminder</h2>
            <div class="checkbox-stack">
              <Checkbox
                bind:checked={reminderEnabled}
                label="Send Me a Weekly Reminder"
                description="Get a macOS notification when it's time to fill in your Weekly Summary. Pick the day and time below."
              />
            </div>

            {#if reminderEnabled}
              <div class="field">
                <span class="field-heading">Days</span>
                <!--
                  Toggle pills, one per ISO weekday. Order Mon-Sun matches
                  the chrono Weekday::num_days_from_monday() the backend
                  uses, so the visual order is the storage order — no
                  remapping at the wire.
                -->
                <div class="day-pills" role="group" aria-label="Reminder days">
                  {#each DAYS as day (day.value)}
                    <button
                      type="button"
                      class="day-pill"
                      class:active={isDaySelected(day.value)}
                      aria-pressed={isDaySelected(day.value)}
                      aria-label={day.long}
                      onclick={() => toggleDay(day.value)}
                    >
                      {day.short}
                    </button>
                  {/each}
                </div>
                {#if remindersEmptyHint}
                  <p class="hint hint-warning">
                    Reminders are on, but no days are picked — nothing will fire
                    until you select at least one.
                  </p>
                {/if}
              </div>

              <div class="field">
                <label for="reminder-time" class="field-heading">Time</label>
                <input
                  id="reminder-time"
                  class="text-input time-input"
                  type="time"
                  bind:value={reminderTime}
                />
              </div>

              <TipBubble heading="Tip">
                macOS sets new apps to <strong>Temporary</strong> notifications by
                default, which auto-dismiss and hide the Write button behind a hover.
                <button
                  type="button"
                  class="link-button"
                  onclick={openNotificationSettings}
                >
                  Open Notification settings
                </button>
                and switch <strong>Alert Style</strong> to <strong>Persistent</strong>
                so the reminder stays on screen with buttons visible.
              </TipBubble>
            {/if}
          </div>
        </div>
      {/if}

      <!-- ============================== Mail ============================== -->
      {#if activeTab === 'mail'}
        <div
          class="form"
          role="tabpanel"
          id="panel-mail"
          aria-labelledby="tab-mail"
        >
          <!-- "How should Send work?" — three orthogonal choices that
               together describe the send path: which client (Send-to-
               manager path), how the body reaches it (Body delivery),
               and what the plaintext flavor looks like (Body format).
               Body format is only meaningful for the Prefilled path —
               Compose + paste hand-delivers rich HTML via the clipboard,
               so plaintext flavor doesn't matter and the radio is hidden. -->
          <div class="section">
            <h2 class="section-title">Mail Delivery</h2>

            <div class="field">
              <label for="mail-send-mode" class="field-heading">
                Send Method
              </label>
              <select
                id="mail-send-mode"
                class="text-input mode-select"
                bind:value={mailSendMode}
              >
                <option value="gmail">Gmail (recommended)</option>
                <option value="native-mail">Native Mac Mail</option>
                <option value="outlook">Outlook (beta)</option>
              </select>
              <p class="hint">
                {#if mailSendMode === 'gmail'}
                  Opens a pre-filled Gmail compose tab in your default
                  browser. No extra permissions, no setup — works as long as
                  you're signed in to Gmail.
                {:else if mailSendMode === 'native-mail'}
                  Drives Mail.app via AppleScript. Needs Automation
                  permission the first time, and macOS Mail must be your
                  configured mail client. Supports rich HTML.
                {:else}
                  Opens Outlook web compose in your default browser. Pick
                  Business for Microsoft 365 / work accounts, Personal for
                  outlook.com / hotmail.com.
                {/if}
              </p>
            </div>

            <div class="field">
              <span class="field-heading">Body Delivery</span>
              <div class="radio-stack" role="radiogroup" aria-label="Body Delivery">
                <label class="radio-row">
                  <input
                    type="radio"
                    name="mail-body-delivery"
                    value="prefilled"
                    bind:group={mailBodyDelivery}
                  />
                  <span>
                    <span class="radio-row-label">Prefilled Draft</span>
                    <span class="radio-row-detail">
                      Body is rendered to plaintext and embedded in the
                      draft. One click to send — but the recipient sees a
                      plaintext message (no bold, no headings).
                    </span>
                  </span>
                </label>
                <label class="radio-row">
                  <input
                    type="radio"
                    name="mail-body-delivery"
                    value="clipboard-paste"
                    bind:group={mailBodyDelivery}
                  />
                  <span>
                    <span class="radio-row-label">Compose + Paste (Formatted)</span>
                    <span class="radio-row-detail">
                      Opens an empty draft in your chosen client and copies
                      the formatted message to your clipboard. Paste with
                      Cmd+V before sending. One extra keystroke, full
                      formatting preserved across all clients.
                    </span>
                  </span>
                </label>
              </div>
            </div>

            {#if mailBodyDelivery === 'prefilled'}
              <!-- Body format is only meaningful for the Prefilled path.
                   Compose + paste copies rich HTML to the clipboard and
                   the prefill body is empty, so plaintext flavor is moot. -->
              <div class="field">
                <span class="field-heading">Body Format</span>
                <div class="radio-stack" role="radiogroup" aria-label="Body Format">
                  <label class="radio-row">
                    <input
                      type="radio"
                      name="mail-body-format"
                      value="clean-text"
                      bind:group={mailBodyFormat}
                    />
                    <span>
                      <span class="radio-row-label">Clean Text</span>
                      <span class="radio-row-detail">
                        Markdown stripped — reads naturally in any mail
                        client.
                      </span>
                    </span>
                  </label>
                  <label class="radio-row">
                    <input
                      type="radio"
                      name="mail-body-format"
                      value="markdown-source"
                      bind:group={mailBodyFormat}
                    />
                    <span>
                      <span class="radio-row-label">Markdown Source</span>
                      <span class="radio-row-detail">
                        Raw <code>**bold**</code> and <code>- bullets</code>
                        preserved.
                      </span>
                    </span>
                  </label>
                </div>
              </div>
            {/if}
          </div>

          <!-- Per-mode controls -->
          {#if mailSendMode === 'gmail'}
            <div class="section">
              <TipBubble>
                <ul class="tip-list">
                  <li>
                    Gmail compose accepts <strong>plaintext only</strong> in
                    the URL — no rich formatting from the prefill itself.
                  </li>
                  <li>
                    <strong>Formatting:</strong> switch
                    <em>Body delivery</em> above to
                    <em>Compose + paste (formatted)</em> for one-step
                    paste-in. (Or use Preview → Copy manually from the
                    Send modal.)
                  </li>
                  <li>
                    Set your <strong>Email</strong> on the General tab so
                    multi-account Gmail lands the draft in the right inbox.
                    Without it, Gmail uses your default signed-in account.
                  </li>
                  <li>
                    Very long summaries can hit Gmail's URL length limit.
                    Captain's Log warns you and lets you send anyway if
                    that happens.
                  </li>
                  <li>
                    Use Markdown source if your manager already reads your
                    raw weekly notes — otherwise Clean text is friendlier.
                  </li>
                </ul>
              </TipBubble>

            </div>
          {/if}

          {#if mailSendMode === 'native-mail'}
            <div class="section">
              <TipBubble>
                <ul class="tip-list">
                  <li>
                    Captain's Log talks to Mail.app via AppleScript. The
                    first send triggers a macOS prompt asking permission to
                    control Mail — accept it, or your send will silently
                    fail.
                  </li>
                  <li>
                    If you denied it by mistake,
                    <button
                      type="button"
                      class="link-button"
                      onclick={openAutomationSettings}
                    >
                      open Automation settings
                    </button>
                    and re-enable Captain's Log for Mail.
                  </li>
                  <li>
                    Styled HTML mode renders a real-looking weekly summary
                    in your sent folder. Clean text and Markdown source
                    fall back to plaintext.
                  </li>
                  <li>
                    Set your <strong>Email</strong> on the General tab to
                    pin the outgoing sender — otherwise Mail uses its
                    default account.
                  </li>
                </ul>
              </TipBubble>

              <!-- Styled HTML .eml is an independent peer override on
                   top of Body delivery: when checked, Mac Mail receives
                   a multipart/alternative .eml with a rich-HTML body —
                   no Cmd+V paste required, recipient sees styled output
                   directly. Trade-off: Mail.app opens .eml read-only, so
                   the sender must click "Message → Edit as New Message"
                   before sending. Wins over Body delivery when both are
                   set (handled server-side). -->
              <div class="checkbox-stack">
                <Checkbox
                  bind:checked={mailNativeHtml}
                  label="Send as Styled HTML Draft (.eml)"
                  description="Mac Mail only — recipient sees a fully styled message with no paste step required. Independent of Body Delivery; this option wins when both are set. The draft opens read-only — click Message → Edit as New Message before sending."
                />
              </div>
            </div>
          {/if}

          {#if mailSendMode === 'outlook'}
            <div class="section">
              <TipBubble>
                <ul class="tip-list">
                  <li>
                    Outlook support is <strong>beta</strong> — the URL
                    scheme works but Microsoft hasn't documented body-
                    length limits, so very long summaries may truncate
                    silently.
                  </li>
                  <li>
                    Pick the right flavor: <strong>Business</strong> for
                    Microsoft 365 work/school accounts (outlook.office.com);
                    <strong>Personal</strong> for outlook.com or hotmail.com.
                  </li>
                  <li>
                    Outlook web compose accepts <strong>plaintext only</strong>
                    in the URL — no rich formatting from the prefill itself.
                  </li>
                  <li>
                    <strong>Formatting:</strong> switch
                    <em>Body delivery</em> above to
                    <em>Compose + paste (formatted)</em> for one-step
                    paste-in. (Or use Preview → Copy manually from the
                    Send modal.)
                  </li>
                  <li>
                    Multi-account routing uses Microsoft's account picker —
                    no need to set your Email on the General tab for that.
                  </li>
                </ul>
              </TipBubble>

              <div class="field">
                <span class="field-heading">Outlook Flavor</span>
                <div class="radio-stack" role="radiogroup" aria-label="Outlook Flavor">
                  <label class="radio-row">
                    <input
                      type="radio"
                      name="mail-outlook-flavor"
                      value="business"
                      bind:group={mailOutlookFlavor}
                    />
                    <span>
                      <span class="radio-row-label">Business</span>
                      <span class="radio-row-detail">
                        Microsoft 365 — work or school account.
                      </span>
                    </span>
                  </label>
                  <label class="radio-row">
                    <input
                      type="radio"
                      name="mail-outlook-flavor"
                      value="personal"
                      bind:group={mailOutlookFlavor}
                    />
                    <span>
                      <span class="radio-row-label">Personal</span>
                      <span class="radio-row-detail">
                        outlook.com, hotmail.com, live.com.
                      </span>
                    </span>
                  </label>
                </div>
              </div>

            </div>
          {/if}
        </div>
      {/if}

      <!-- ============================== Theme ============================== -->
      {#if activeTab === 'theme'}
        <div
          class="form"
          role="tabpanel"
          id="panel-theme"
          aria-labelledby="tab-theme"
        >
          <div class="section">
            <h2 class="section-title">Main Theme</h2>
            <p class="hint">
              Applies immediately as you click; saves when you click Done.
            </p>

            <div class="theme-radio-group" role="radiogroup" aria-label="Main theme">
              <button
                type="button"
                class="theme-radio"
                class:active={currentTheme === 'dark'}
                role="radio"
                aria-checked={currentTheme === 'dark'}
                onclick={() => setTheme('dark')}
              >
                <span class="radio-dot" aria-hidden="true"></span>
                <span class="radio-label">Dark</span>
                <span class="radio-detail">Default for new installs.</span>
              </button>

              <button
                type="button"
                class="theme-radio"
                class:active={currentTheme === 'light'}
                role="radio"
                aria-checked={currentTheme === 'light'}
                onclick={() => setTheme('light')}
              >
                <span class="radio-dot" aria-hidden="true"></span>
                <span class="radio-label">Light</span>
                <span class="radio-detail">Warm-tinted neutrals.</span>
              </button>

              <button
                type="button"
                class="theme-radio"
                class:active={currentTheme === 'custom'}
                role="radio"
                aria-checked={currentTheme === 'custom'}
                onclick={() => setTheme('custom')}
              >
                <span class="radio-dot" aria-hidden="true"></span>
                <span class="radio-label">Custom</span>
                <span class="radio-detail">
                  Pick your own hex values for the key UI tokens.
                </span>
              </button>
            </div>
          </div>

          <!-- Editor panel — only when Custom is active. Hidden under
               Light/Dark so the tab stays calm. -->
          {#if currentTheme === 'custom' && customEditor}
            {#if editorSeeded}
              <p class="hint custom-seed-hint">
                Started from your current {seededFromLabel} theme.
                Edit any token below — changes apply live.
              </p>
            {/if}

            <!-- Slice 6: surface advisories. Saturated-surface banner is
                 non-blocking — sits above the editor as an FYI. Ambiguous-
                 base picker only renders when bgSurface is in the OKLCH-L
                 mid-grey band (0.50–0.60) where auto-inference flips
                 unpredictably on tiny edits. -->
            {#if surfaceIsSaturated}
              <p class="hint custom-warning-banner" role="status">
                Some derived colours may look unusual on saturated
                surfaces. Switch to a less saturated background if
                anything looks off.
              </p>
            {/if}

            {#if surfaceIsAmbiguous}
              <div class="custom-force-base" role="group" aria-label="Force base polarity">
                <span class="custom-force-base-label">Force base:</span>
                <button
                  type="button"
                  class="force-base-pill"
                  class:active={(forceBaseOverride ?? customEditorBase) === 'dark'}
                  aria-pressed={(forceBaseOverride ?? customEditorBase) === 'dark'}
                  onclick={() => setForceBase('dark')}
                >
                  Dark
                </button>
                <button
                  type="button"
                  class="force-base-pill"
                  class:active={(forceBaseOverride ?? customEditorBase) === 'light'}
                  aria-pressed={(forceBaseOverride ?? customEditorBase) === 'light'}
                  onclick={() => setForceBase('light')}
                >
                  Light
                </button>
                <p class="hint custom-force-base-hint">
                  Your surface is in the mid-grey range where text polarity
                  is a coin-flip. Pick which side to derive against.
                </p>
              </div>
            {/if}

            {#each TOKEN_SECTIONS as section (section.title)}
              <div class="section">
                <h2 class="section-title">{section.title}</h2>
                {#each section.tokens as token (token.key)}
                  {@const value = customEditor[token.key]}
                  {@const valid = isValidHex6(value)}
                  {@const warning = checkContrast(token.key)}
                  <div class="field token-field">
                    <label for="token-{token.key}">{token.label}</label>
                    <div class="token-row">
                      <button
                        type="button"
                        class="token-swatch"
                        style="background-color: {valid ? value : 'transparent'}"
                        aria-label="Focus {token.label} input"
                        onclick={() => {
                          const el = document.getElementById(`token-${token.key}`);
                          if (el instanceof HTMLInputElement) {
                            el.focus();
                            el.select();
                          }
                        }}
                      ></button>
                      <input
                        id="token-{token.key}"
                        class="text-input token-input"
                        class:invalid={!valid}
                        type="text"
                        inputmode="text"
                        spellcheck="false"
                        autocomplete="off"
                        pattern="^#[0-9a-fA-F]{'{6}'}$"
                        bind:value={customEditor[token.key]}
                        oninput={() => {
                          editorSeeded = false;
                          customEditorDirty = true;
                        }}
                      />
                    </div>
                    <p class="hint token-hint">{token.hint}</p>
                    {#if !valid}
                      <p class="hint hint-warning">
                        Not a 6-digit hex color. Use the form #rrggbb.
                      </p>
                    {/if}
                    {#if warning}
                      <p class="hint hint-warning">
                        Contrast with {warning.targetLabel} is
                        {warning.ratio.toFixed(2)}:1 — below AA's
                        {warning.minRatio.toFixed(1)}:1.
                      </p>
                    {/if}
                  </div>
                {/each}
              </div>
            {/each}

            <!-- Reset / Export / Import toolbar. Sits below the 4 token
                 sections as a footer band — separate from .actions at the
                 page bottom (Done/Cancel) since these operate on the editor
                 state, not on Settings as a whole. -->
            <div class="custom-toolbar">
              <div class="custom-toolbar-row">
                <!-- Reset-to-Light / Reset-to-Dark buttons used to live here
                     but were dropped: selecting the Light or Dark radio
                     above already swaps clean — clearCustomTheme() removes
                     the inline overrides and the data-theme attribute flip
                     lets the preset CSS take over. -->
                <button
                  type="button"
                  class="btn btn-marble btn-sm"
                  onclick={importTheme}
                >
                  Import theme…
                </button>
                <button
                  type="button"
                  class="btn btn-marble btn-sm"
                  onclick={exportTheme}
                >
                  Export theme…
                </button>
              </div>
              <p class="hint custom-toolbar-hint">
                Need color ideas?
                <button
                  type="button"
                  class="link-button"
                  onclick={openColorIdeas}
                >
                  Browse colors at htmlcolorcodes.com →
                </button>
              </p>
              {#if toolbarStatus.kind === 'success'}
                <p class="hint custom-toolbar-status custom-toolbar-status-ok" role="status">
                  {toolbarStatus.message}
                </p>
              {:else if toolbarStatus.kind === 'error'}
                <p
                  class="hint hint-warning custom-toolbar-status"
                  role="alert"
                >
                  {toolbarStatus.message}
                  {#if lastExportAttempt}
                    <button
                      type="button"
                      class="link-button"
                      onclick={retryExport}
                    >
                      Retry
                    </button>
                  {/if}
                </p>
              {/if}
            </div>
          {/if}

          <!-- Label Theme — a separate section so labels read as their
               own coloring axis, independent of the Main Theme above.
               Two options: Standard (theme accent palette cycle, the
               default) and Colorful (per-name deterministic hue). The
               underlying `colorfulLabels` boolean stays the same — the
               two radios just write false / true to it. Persisted hex
               overrides on disk are unaffected by toggling between
               Standard and Colorful; they only apply in Colorful mode. -->
          <div class="section">
            <h2 class="section-title">Label Theme</h2>
            <div class="theme-radio-group" role="radiogroup" aria-label="Label theme">
              <button
                type="button"
                class="theme-radio"
                class:active={!colorfulLabels}
                role="radio"
                aria-checked={!colorfulLabels}
                onclick={() => (colorfulLabels = false)}
              >
                <span class="radio-dot" aria-hidden="true"></span>
                <span class="radio-label">Standard</span>
                <span class="radio-detail">
                  Labels cycle through the theme accent palette.
                </span>
              </button>

              <button
                type="button"
                class="theme-radio"
                class:active={colorfulLabels}
                role="radio"
                aria-checked={colorfulLabels}
                onclick={() => (colorfulLabels = true)}
              >
                <span class="radio-dot" aria-hidden="true"></span>
                <span class="radio-label">Colorful</span>
                <span class="radio-detail">
                  Each label gets its own per-name color.
                </span>
              </button>
            </div>

            <TipBubble heading="Tip">
              Colors are deterministic by name and adapt to the active
              theme. Override individual label colors from the Label
              Details popup in the Labels tab.
            </TipBubble>
          </div>
        </div>
      {/if}

      <!-- ============================== Labels ============================== -->
      {#if activeTab === 'labels'}
        <div
          class="form"
          role="tabpanel"
          id="panel-labels"
          aria-labelledby="tab-labels"
        >
          <div class="section label-panel">
            {#if isRebuildingIndex}
              <!-- LoadingOverlay sits inside the panel so the rest of
                   Settings (tabs, header, Done/Cancel) stays interactive.
                   Generic reusable component — any future blocking op can
                   drop it in with a custom message. -->
              <LoadingOverlay message="Rebuilding label index…" />
            {:else if labelsError}
              <div class="label-error-card" role="alert">
                <p class="hint hint-warning">{labelsError}</p>
                <button
                  type="button"
                  class="btn btn-marble btn-sm"
                  onclick={() => void rebuildLabelIndex()}
                >
                  Retry
                </button>
              </div>
            {:else}
              <div class="field">
                <input
                  type="text"
                  class="text-input"
                  placeholder="Filter labels…"
                  bind:value={labelFilter}
                  aria-label="Filter labels"
                />
              </div>

              {#if labels.length === 0}
                <p class="hint">
                  No labels yet. Add labels to a note or weekly summary,
                  and they'll show up here.
                </p>
              {:else if visibleLabels.length === 0}
                <p class="hint">
                  No labels match "{labelFilter.trim()}".
                </p>
              {:else}
                <!-- Bulk actions toolbar. Left side owns selection state
                     (select-all + counter + clear); right side owns the
                     batch actions and stays empty until N > 0 so the
                     toolbar doesn't shout at users who aren't selecting. -->
                <div class="bulk-actions" role="toolbar" aria-label="Bulk label actions">
                  <div class="bulk-select-all">
                    <Checkbox
                      checked={allVisibleSelected}
                      onchange={toggleSelectAllVisible}
                      ariaLabel="Select all visible labels"
                    >
                      <span class="bulk-select-all-text">
                        {#if selectionCount > 0}
                          {selectionCount} selected
                        {:else}
                          Select all
                        {/if}
                      </span>
                    </Checkbox>
                  </div>
                  {#if selectionCount > 0}
                    <button
                      type="button"
                      class="link-button bulk-clear"
                      onclick={clearBulkSelection}
                    >
                      Clear
                    </button>
                  {/if}
                  <div class="bulk-actions-spacer"></div>
                  {#if selectionCount > 0}
                    <button
                      type="button"
                      class="btn btn-marble btn-sm"
                      disabled={selectionCount < 2}
                      title={selectionCount < 2
                        ? 'Pick at least 2 labels to merge'
                        : ''}
                      onclick={startBulkMerge}
                    >
                      Merge into…
                    </button>
                    <button
                      type="button"
                      class="btn btn-ruby btn-sm"
                      onclick={startBulkDelete}
                    >
                      Delete {selectionCount}
                    </button>
                  {/if}
                </div>

                <!-- Result / error banner from the most recent bulk op.
                     Persists until the user dismisses OR modifies the
                     selection (a new op is about to start; the old
                     receipt is stale). -->
                {#if bulkOpMessage}
                  <div
                    class="bulk-op-banner"
                    class:is-error={bulkOpError !== ''}
                    role={bulkOpError ? 'alert' : 'status'}
                  >
                    <span>{bulkOpMessage}</span>
                    <button
                      type="button"
                      class="link-button"
                      onclick={dismissBulkOpBanner}
                      aria-label="Dismiss"
                    >
                      ×
                    </button>
                  </div>
                {/if}

                <ul class="label-list" role="list">
                  {#each visibleLabels as entry (entry.name)}
                    <li class="label-row">
                      <Checkbox
                        checked={selectedLabelNames.has(entry.name)}
                        onchange={() => toggleLabelSelection(entry.name)}
                        ariaLabel={`Select ${entry.name}`}
                      />
                      <span
                        class="label-chip"
                        style={labelChipStyle(entry)}
                      >
                        {entry.name}
                      </span>
                      <span class="label-name-text">{entry.name}</span>
                      <span class="label-count" aria-label="{entry.count} uses">
                        {entry.count}
                      </span>
                      <button
                        type="button"
                        class="btn btn-marble btn-sm"
                        onclick={() => (selectedLabel = entry)}
                      >
                        Details
                      </button>
                    </li>
                  {/each}
                </ul>
              {/if}
            {/if}
          </div>
        </div>
      {/if}

      <!-- ============================== Tasks tab ============================== -->
      {#if activeTab === 'tasks'}
        <div class="form" role="tabpanel" id="panel-tasks" aria-labelledby="tab-tasks">
          <div class="section">
            <h2 class="section-title">Display</h2>
            <div class="checkbox-stack">
              <Checkbox
                bind:checked={taskShowCompleted}
                label="Show Completed Tasks"
                description="Keep finished tasks in view alongside the open ones. Turn this off to focus only on what's left to do."
              />
              <Checkbox
                bind:checked={taskOpenTasksFirst}
                label="Open Tasks First"
                description="Open tasks appear at the top; completed ones sink to the bottom. When off, tasks show in the order they were written in your Weekly Summary."
              />
              <Checkbox
                bind:checked={taskShowCompletedTimestamp}
                label="Show Completion Timestamp"
                description={'Adds a subtle “checked 2h ago” chip next to completed tasks. Off by default to keep the list tight.'}
              />
              <Checkbox
                bind:checked={taskAutoRolloverEnabled}
                label="Auto-Roll Over Incomplete Tasks"
                description="At the start of each week, any tasks you didn't finish last week are copied into this week's list. Rolled-over tasks show a small chip so you can see where they came from. Turn off to start each week with a clean list."
              />
              <Checkbox
                bind:checked={taskHideTaskList}
                label="Hide the Task List"
                description="Removes the task list section entirely from the main page. Useful if you don't use the task feature."
              />
            </div>
          </div>

          <div class="section">
            <h2 class="section-title">Rebuild Task Index</h2>
            <TipBubble>
              Scans every weekly file and syncs task state. Backfills
              missing completion timestamps for tasks that were
              checked outside Captain's Log, prunes stale entries
              whose task no longer exists, and copies any stranded
              incomplete tasks from older weeks directly into this
              week's Plans section (with a "from Wxx" chip so you
              see where each came from). Safe to run any time.
            </TipBubble>
            <div class="field">
              <button
                type="button"
                class="btn btn-marble"
                onclick={rebuildTaskIndex}
                disabled={isRebuildingTasks}
              >
                {isRebuildingTasks ? 'Rebuilding…' : 'Rebuild task index'}
              </button>
            </div>
            {#if taskRebuildError}
              <p class="hint hint-warning">{taskRebuildError}</p>
            {/if}
            {#if taskRebuildReceipt}
              <!--
                aria-live="polite" so a screen reader announces the
                receipt when it appears; role="status" gives it a
                landmark. aria-atomic on the wrapper so the whole
                summary is read as one unit rather than three
                fragments.
              -->
              <div
                class="rebuild-receipt"
                role="status"
                aria-live="polite"
                aria-atomic="true"
              >
                <p class="hint">
                  Scanned <strong>{taskRebuildReceipt.filesScanned}</strong>
                  {taskRebuildReceipt.filesScanned === 1 ? 'file' : 'files'}
                  ({taskRebuildReceipt.tasksScanned}
                  {taskRebuildReceipt.tasksScanned === 1 ? 'task' : 'tasks'}) in
                  {taskRebuildReceipt.durationMs}ms.
                </p>
                <p class="hint">
                  Backfilled <strong>{taskRebuildReceipt.entriesBackfilled}</strong>,
                  pruned <strong>{taskRebuildReceipt.entriesPruned}</strong>,
                  swept forward <strong>{taskRebuildReceipt.tasksSweptForward}</strong>.
                </p>
                {#if taskRebuildReceipt.failedFiles.length > 0}
                  <p class="hint hint-warning">
                    Couldn't read {taskRebuildReceipt.failedFiles.length}
                    {taskRebuildReceipt.failedFiles.length === 1 ? 'file' : 'files'}:
                    {taskRebuildReceipt.failedFiles.join(', ')}
                  </p>
                {/if}
              </div>
            {/if}
          </div>
        </div>
      {/if}

      {#if saveError}
        <p class="error">{saveError}</p>
      {/if}

      <!-- Done (primary) on the left, Cancel (ruby/destructive) on the
         right — app-wide convention from Phase 2.7. -->
      <div class="actions">
        <button class="btn btn-emerald" onclick={save} disabled={saving}>
          {saving ? 'Saving…' : 'Done'}
        </button>
        <button class="btn btn-ruby" onclick={cancel} disabled={saving}>
          Cancel
        </button>
      </div>
    </section>
  </main>

  <!-- Slice 10 — Label Details modal. Only rendered when:
         (a) the user has actually selected a row, AND
         (b) the Labels tab is still active. (Switching tabs while the
             modal is open would otherwise leave it floating over an
             unrelated panel.)
       The modal owns its own backdrop + focus trap + Escape handling. -->
  {#if selectedLabel && activeTab === 'labels'}
    <LabelDetailsModal
      label={selectedLabel}
      colorfulLabels={colorfulLabels}
      theme={currentThemeMode()}
      bgSurface={currentBgSurface()}
      onClose={closeLabelModal}
      onLabelMutated={() => void onLabelMutated()}
    />
  {/if}

  <!-- Slice 2 — bulk delete confirm. ConfirmDialog owns backdrop + Escape
       + focus. `activeTab === 'labels'` guard mirrors the details modal
       pattern so tab-switching mid-dialog can't leave a floating confirm. -->
  {#if showBulkDeleteConfirm && activeTab === 'labels'}
    <ConfirmDialog
      title="Delete {selectionCount} label{selectionCount === 1 ? '' : 's'}?"
      confirmLabel={bulkDeleteInFlight ? 'Deleting…' : 'Delete'}
      confirmVariant="ruby"
      cancelVariant="marble"
      onConfirm={() => void confirmBulkDelete()}
      onCancel={() => (showBulkDeleteConfirm = false)}
      body={bulkDeleteBody}
    />
  {/if}

  {#snippet bulkDeleteBody()}
    <p>
      This removes the selected labels from every Note and Weekly Summary
      across your journal. Inline <code>#hashtag</code> text in prose is
      left alone.
    </p>
  {/snippet}

  <!-- Slice 2 — bulk merge picker. Plain Modal (not ConfirmDialog) because
       the primary action must stay disabled until a canonical is picked,
       which ConfirmDialog's slim API doesn't expose. -->
  {#if showBulkMergePicker && activeTab === 'labels'}
    <Modal
      open={true}
      onClose={cancelBulkMerge}
      title="Merge {selectionCount} labels"
      maxWidth="min(480px, calc(100vw - 32px))"
      zLayer="nested"
    >
      <div class="bulk-merge-body">
        <p>
          Pick the canonical label. Every other selected label will be
          renamed to match across your journal (inline <code>#hashtag</code>
          text in prose is left alone).
        </p>
        <ul class="bulk-merge-list" role="radiogroup" aria-label="Canonical label">
          {#each Array.from(selectedLabelNames).sort((a, b) => a.localeCompare(b)) as name (name)}
            <li>
              <label class="bulk-merge-option">
                <input
                  type="radio"
                  name="bulk-merge-canonical"
                  value={name}
                  checked={bulkMergeCanonical === name}
                  onchange={() => (bulkMergeCanonical = name)}
                  disabled={bulkMergeInFlight}
                />
                <span class="bulk-merge-name">{name}</span>
              </label>
            </li>
          {/each}
        </ul>
        {#if bulkOpError}
          <p class="hint hint-warning">{bulkOpError}</p>
        {/if}
        <div class="modal-actions">
          <button
            type="button"
            class="btn btn-emerald"
            onclick={() => void confirmBulkMerge()}
            disabled={bulkMergeInFlight || !bulkMergeCanonical}
          >
            {bulkMergeInFlight ? 'Merging…' : 'Merge'}
          </button>
          <button
            type="button"
            class="btn btn-marble"
            onclick={cancelBulkMerge}
            disabled={bulkMergeInFlight}
          >
            Cancel
          </button>
        </div>
      </div>
    </Modal>
  {/if}

  <!-- Cancel-with-imported-theme guard. Renders only when the user
       clicked Cancel while an imported theme hasn't been Done'd yet. -->
  {#if showCancelImportConfirm}
    <ConfirmDialog
      title="Discard The Imported Theme?"
      confirmLabel="Discard"
      confirmVariant="ruby"
      cancelLabel="Keep Editing"
      cancelVariant="marble"
      onConfirm={() => void performCancel()}
      onCancel={() => (showCancelImportConfirm = false)}
      body={cancelImportConfirmBody}
    />
  {/if}
{/if}

{#snippet cancelImportConfirmBody()}
  <p>
    You just imported a theme but haven't saved yet. Discard the imported
    palette and revert Settings, or keep editing so you can click Done?
  </p>
{/snippet}

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
    max-width: 640px;
  }

  header {
    margin-bottom: var(--space-6);
  }

  .subtitle {
    color: var(--text-secondary);
    margin-top: var(--space-2);
  }

  /* ---- Tab bar ---- */

  .tabs {
    display: flex;
    gap: var(--space-1);
    border-bottom: 1px solid var(--border-structural);
    margin-bottom: var(--space-6);
  }

  .tab {
    appearance: none;
    background: none;
    border: none;
    padding: var(--space-3) var(--space-4);
    font-family: var(--font-display);
    font-size: var(--text-button);
    color: var(--text-secondary);
    cursor: pointer;
    /* Underline-on-active is the affordance; the 2px reservation keeps
       the inactive tabs from shifting down 2px when one activates. */
    border-bottom: 2px solid transparent;
    margin-bottom: -1px;
    transition: color var(--transition-fast), border-color var(--transition-fast);
  }
  .tab:hover {
    color: var(--text-primary);
  }
  .tab.active {
    color: var(--text-primary);
    border-bottom-color: var(--accent-primary);
  }
  .tab:focus-visible {
    outline: none;
    box-shadow: 0 0 0 2px var(--focus-glow);
    border-radius: var(--radius-sm);
  }

  /* ---- Form ---- */

  .form {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
  }

  .section {
    display: flex;
    flex-direction: column;
    /* space-6 (24px) gives breathing room between fields without the
       section feeling sparse. Earlier shape used a nonexistent
       --space-5 token which resolved to 0; that was the "cramped"
       feel on the Reminders tab where there's no section-title rule
       to provide implicit vertical rhythm. */
    gap: var(--space-6);
  }

  /*
    Section title = chapter marker. Displays the tab's top-level
    groupings (User Information, Weekly Reminder, Mail Delivery,
    etc.). Bigger than a field-heading (text-display-sm vs
    text-button) so scanning the tab reads as "chapter → fields"
    rather than "row → row".
  */
  .section-title {
    font-family: var(--font-display);
    font-size: var(--text-display-sm);
    line-height: var(--text-display-sm-lh);
    color: var(--text-primary);
    margin: 0 0 var(--space-3);
    padding-bottom: var(--space-2);
    border-bottom: 1px solid var(--border-decorative);
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

  /* .text-input + :focus-visible live in app.css as a shared utility.
     The label+input+Browse trio for the journal-location row now uses
     $lib/PathPickerField (owns the .path-row + .path-input layout). */

  .time-input {
    /* Native time inputs are wider than they need to be by default — clamp
       so it doesn't dominate the form rhythm. */
    max-width: 160px;
  }

  .hint {
    margin: 0;
    font-size: var(--text-caption);
    line-height: var(--text-caption-lh);
    color: var(--text-secondary);
  }
  /* Use the per-theme contrast-safe pink — raw --accent-pink (#eb018b)
     fails WCAG AA at 13px against bg-base. --accent-pink-text is
     calibrated per theme to clear 4.5:1. */
  .hint-warning {
    color: var(--accent-pink-text);
  }
  /* .hint strong rule removed — no <strong> markup lives inside a .hint
     <p> anymore (display-font emphasis tokens live in the TipBubble
     component now). */

  /* The persistent-hint + link-button styles that used to live here have
     moved to the shared <TipBubble> component (see lib/onboarding/
     TipBubble.svelte). Settings's reminder tip now uses TipBubble too,
     so /capture, /settings, and the onboarding wizard all render the
     same tip language with a single source of styling truth. */

  /* ---- Reminder: day pills ---- */

  .day-pills {
    display: flex;
    gap: var(--space-2);
    flex-wrap: wrap;
  }

  .day-pill {
    appearance: none;
    flex: 1 1 0;
    min-width: 48px;
    padding: var(--space-2) var(--space-3);
    background: var(--bg-surface);
    color: var(--text-secondary);
    border: 1px solid var(--border-structural);
    border-radius: var(--radius-md);
    font-family: var(--font-display);
    font-size: var(--text-caption);
    line-height: 1.2;
    cursor: pointer;
    transition: all var(--transition-fast);
  }
  .day-pill:hover {
    color: var(--text-primary);
    border-color: var(--accent-primary);
  }
  /* Active pill uses the same fill (--accent-primary) as primary buttons,
     so reuse --btn-primary-text — the token that the derivation pipeline
     already calibrates against the live --accent-primary fill (white for
     saturated/dark primaries, derived near-black for pale primaries).
     Previously hardcoded to #1f0a02; that color was hand-tuned against
     the shipping orange (#ff5c08) and turned near-invisible on custom
     dark accents. */
  .day-pill.active {
    background: var(--accent-primary);
    color: var(--btn-primary-text);
    border-color: var(--accent-primary);
  }
  .day-pill:focus-visible {
    outline: none;
    box-shadow: 0 0 0 2px var(--focus-glow);
  }

  /* ---- Theme: radio cards ---- */

  .theme-radio-group {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .theme-radio {
    appearance: none;
    text-align: left;
    display: grid;
    grid-template-columns: 20px 1fr;
    grid-template-rows: auto auto;
    column-gap: var(--space-3);
    row-gap: var(--space-1);
    padding: var(--space-3) var(--space-4);
    background: var(--bg-surface);
    border: 1px solid var(--border-structural);
    border-radius: var(--radius-md);
    cursor: pointer;
    font-family: var(--font-body);
    color: var(--text-primary);
    transition: all var(--transition-fast);
  }
  .theme-radio:hover:not(.is-disabled) {
    border-color: var(--accent-primary);
  }
  .theme-radio:focus-visible {
    outline: none;
    box-shadow: 0 0 0 2px var(--focus-glow);
    border-color: var(--accent-primary);
  }
  .theme-radio.active {
    border-color: var(--accent-primary);
    background: var(--bg-elevated);
  }
  /* (Phase 2.8 Slice 4): the .theme-radio.is-disabled and .badge "Coming
     soon" styles were removed when the Custom card was promoted to fully
     interactive. The detail/dot/label rules below still serve all three
     theme radios. */

  .radio-dot {
    grid-column: 1;
    grid-row: 1 / span 2;
    align-self: start;
    margin-top: 3px;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: 1.5px solid var(--border-structural);
    background: transparent;
    transition: all var(--transition-fast);
  }
  .theme-radio.active .radio-dot {
    border-color: var(--accent-primary);
    box-shadow: inset 0 0 0 3px var(--accent-primary);
  }

  .radio-label {
    grid-column: 2;
    grid-row: 1;
    font-family: var(--font-display);
    font-size: var(--text-button);
    color: var(--text-primary);
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }
  .radio-detail {
    grid-column: 2;
    grid-row: 2;
    font-size: var(--text-caption);
    line-height: var(--text-caption-lh);
    color: var(--text-secondary);
  }

  /* ---- Theme: Custom editor (Phase 2.8 — Slice 4) ----
     Per-row layout is:
       <label>
       [swatch] [hex input]
       <hint>
       <warning?>
     The swatch is a 28×28 button so it's clickable (focuses the input) and
     keyboard-reachable. Hint is the regular .hint style; warning uses the
     contrast-safe pink (--accent-pink-text). */
  .custom-seed-hint {
    /* Slight tint so this announcement reads as a panel intro rather than
       a per-field hint. Mirrors the look of the saving-tip in the wizard. */
    padding: var(--space-3);
    background: var(--bg-elevated);
    border: 1px solid var(--border-decorative);
    border-radius: var(--radius-md);
    color: var(--text-secondary);
    margin-top: calc(-1 * var(--space-4));
  }

  /* Slice 6 — saturated-surface advisory. Same panel-intro container as
     the seed hint, but tinted in the pink/error family because it's a
     "heads up" rather than a confirmation. Non-blocking — the user can
     keep editing. */
  .custom-warning-banner {
    padding: var(--space-3);
    background: var(--bg-error-tint-soft);
    border: 1px solid var(--border-error);
    border-radius: var(--radius-md);
    color: var(--accent-pink-text);
  }

  /* Slice 6 — Force base picker. Only renders for mid-grey surfaces where
     auto-inference is ambiguous. Visually peer to the seed/warning bands. */
  .custom-force-base {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2) var(--space-3);
    padding: var(--space-3);
    background: var(--bg-elevated);
    border: 1px solid var(--border-decorative);
    border-radius: var(--radius-md);
  }
  .custom-force-base-label {
    font-family: var(--font-display);
    font-size: var(--text-button);
    color: var(--text-primary);
  }
  .force-base-pill {
    appearance: none;
    padding: var(--space-1) var(--space-3);
    background: var(--bg-surface);
    color: var(--text-secondary);
    border: 1px solid var(--border-structural);
    border-radius: var(--radius-pill);
    font-family: var(--font-display);
    font-size: var(--text-caption);
    cursor: pointer;
    transition: all var(--transition-fast);
  }
  .force-base-pill:hover {
    color: var(--text-primary);
    border-color: var(--accent-primary);
  }
  .force-base-pill.active {
    background: var(--accent-primary);
    color: var(--btn-primary-text);
    border-color: var(--accent-primary);
  }
  .force-base-pill:focus-visible {
    outline: none;
    box-shadow: 0 0 0 2px var(--focus-glow);
  }
  .custom-force-base-hint {
    flex-basis: 100%;
    margin-top: var(--space-1);
  }

  .token-field {
    /* Tighter than the form's default field rhythm — 12 of these in a row
       would balloon the panel if we used the section-level space-6. */
    gap: var(--space-1);
  }

  .token-row {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }

  .token-swatch {
    appearance: none;
    width: 28px;
    height: 28px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-structural);
    cursor: pointer;
    padding: 0;
    /* Subtle inner ring keeps the swatch visible when the user's chosen
       color matches the page surface exactly. */
    box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.06);
    transition: border-color var(--transition-fast),
      transform var(--transition-fast);
  }
  .token-swatch:hover {
    border-color: var(--accent-primary);
    transform: scale(1.05);
  }
  .token-swatch:focus-visible {
    outline: none;
    box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.06),
      0 0 0 2px var(--focus-glow);
    border-color: var(--accent-primary);
  }

  .token-input {
    /* 140px holds "#rrggbb" with room to spare without stretching the
       column the way the default .text-input would. Monospace so the hex
       reads cleanly. */
    width: 140px;
    font-family: var(--font-mono, ui-monospace, SFMono-Regular, Menlo, monospace);
    font-size: var(--text-caption);
    letter-spacing: 0.02em;
    text-transform: lowercase;
  }
  .token-input.invalid {
    border-color: var(--accent-pink-text);
    /* Soft pink wash so the row reads as "needs attention" without the
       full alarm-state pink of an actual error card. */
    background: var(--bg-error-tint-soft);
  }

  .token-hint {
    /* Visually demote the per-row hint relative to the .hint default so
       12 of them don't crowd the panel. */
    font-size: 12px;
    margin-top: 2px;
  }

  /* ---- Custom-theme toolbar (Phase 2.8 — Slice 5) ----
     Sits below the 4 token sections as the panel's footer band. Buttons
     are .btn .btn-marble .btn-sm — same affordance the path-row Browse
     button uses — so the toolbar reads as a tertiary control row, not as
     a peer to the Done/Cancel pair at the page bottom. */
  .custom-toolbar {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    padding: var(--space-4);
    background: var(--bg-elevated);
    border: 1px solid var(--border-structural);
    border-radius: var(--radius-md);
  }
  .custom-toolbar-row {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
    align-items: center;
  }
  .custom-toolbar-hint {
    margin: 0;
  }
  /* Toolbar uses link-button outside of a TipBubble, so the TipBubble's
     :global(button.link-button) rule doesn't reach it. Provide a local
     baseline that matches the same visual language — accent-colored,
     button-shaped only on focus. */
  .custom-toolbar :global(button.link-button) {
    appearance: none;
    background: none;
    border: none;
    padding: 0;
    margin: 0;
    color: var(--accent-primary);
    font-family: inherit;
    font-size: inherit;
    line-height: inherit;
    text-decoration: underline;
    text-underline-offset: 2px;
    cursor: pointer;
  }
  .custom-toolbar :global(button.link-button:hover) {
    color: var(--text-primary);
  }
  .custom-toolbar :global(button.link-button:focus-visible) {
    outline: none;
    box-shadow: 0 0 0 2px var(--focus-glow);
    border-radius: var(--radius-sm);
  }
  .custom-toolbar-status {
    margin: 0;
  }
  /* Saved confirmation — green to match the .btn-emerald Done button so
     the "you did the save thing" feedback is visually consistent. */
  .custom-toolbar-status-ok {
    color: var(--accent-green);
  }

  /* ---- Mail tab ---- */

  .mode-select {
    /* Native <select> picks up .text-input padding + border. Cap width
       so the dropdown doesn't stretch the full form width. */
    max-width: 280px;
    cursor: pointer;
  }

  /* Tight bulleted body inside a TipBubble. The TipBubble already
     provides the orange left stripe and the bg-elevated tint; this
     just constrains list indent and rhythm so the bullets sit aligned
     under the "Heads up" strong-styled heading rather than indenting
     hard against the bubble's left padding. */
  .tip-list {
    margin: var(--space-2) 0 0;
    padding-left: var(--space-5, var(--space-4));
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .tip-list li {
    line-height: var(--text-caption-lh);
  }
  /* Inline <code> inside tips — match the body monospace at caption size
     so 'gmail' / '**bold**' don't break the reading rhythm. */
  .tip-list :global(code) {
    font-family: var(--font-mono, ui-monospace, SFMono-Regular, Menlo, monospace);
    font-size: 0.95em;
    background: var(--bg-surface);
    border: 1px solid var(--border-decorative);
    border-radius: var(--radius-sm);
    padding: 0 4px;
  }

  /* Radio-card stack — used by Mail body-format and Outlook-flavor pickers.
     Lighter weight than .theme-radio (no grid layout, no accent dot) so
     three rows don't dominate the Mail tab the way they would on Theme. */
  .radio-stack {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  /* Shared vertical stack for the Checkbox card variant (padded
     capsule with heading + detail) — Tasks, Reminders, and Mail
     tabs all use this. Same spacing as .radio-stack so the two
     control families sit uniformly if a section ever mixes them. */
  .checkbox-stack {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .radio-row {
    position: relative;
    display: flex;
    align-items: flex-start;
    gap: var(--space-3);
    padding: var(--space-3);
    background: var(--bg-surface);
    border: 1px solid var(--border-structural);
    border-radius: var(--radius-md);
    cursor: pointer;
    transition: border-color var(--transition-fast),
      background var(--transition-fast);
  }
  .radio-row:hover {
    border-color: var(--accent-primary);
  }

  /* Radio rows (NOT checkbox rows) get a custom circle dot via ::before
   * so the visual matches the Theme tab's .theme-radio cards. The native
   * input stays in the DOM for `bind:group` to keep working but is
   * visually hidden via position:absolute + opacity:0. `:has()` gates
   * the dot to radio-only rows so checkbox rows (the Native Mac HTML
   * toggle) keep their native check. */
  .radio-row:has(input[type='radio']) input[type='radio'] {
    position: absolute;
    opacity: 0;
    pointer-events: none;
  }
  .radio-row:has(input[type='radio'])::before {
    content: '';
    margin-top: 3px;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: 1.5px solid var(--border-structural);
    background: transparent;
    flex-shrink: 0;
    transition: border-color var(--transition-fast),
      box-shadow var(--transition-fast);
  }
  .radio-row:has(input[type='radio']:checked) {
    border-color: var(--accent-primary);
    background: var(--bg-elevated);
  }
  .radio-row:has(input[type='radio']:checked)::before {
    border-color: var(--accent-primary);
    box-shadow: inset 0 0 0 3px var(--accent-primary);
  }
  .radio-row:has(input[type='radio']:focus-visible) {
    border-color: var(--accent-primary);
    box-shadow: 0 0 0 2px var(--focus-glow);
  }

  /* Checkbox rows keep the native check + accent-color tint — `:has()`
   * gates the dot above so this rule only matches when the row hosts
   * an actual checkbox. */
  .radio-row-label {
    display: block;
    font-family: var(--font-display);
    font-size: var(--text-button);
    color: var(--text-primary);
  }
  .radio-row-detail {
    display: block;
    font-size: var(--text-caption);
    line-height: var(--text-caption-lh);
    color: var(--text-secondary);
    margin-top: 2px;
  }

  /* ---- Actions ---- */

  .actions {
    display: flex;
    gap: var(--space-3);
    justify-content: flex-end;
    margin-top: var(--space-8);
  }

  .error {
    padding: var(--space-3);
    border-radius: var(--radius-md);
    background: var(--bg-error-tint);
    color: var(--accent-pink-text);
    border: 1px solid var(--border-error);
    font-size: var(--text-caption);
    margin: var(--space-4) 0 0;
  }

  /* ---- Error-state card tint ----
   * The .card / .card.is-narrow base + .card h2 / .card p rules live in
   * app.css as a shared utility. This route only adds the pink-tint
   * overlay used on the load-error placeholder. */

  .error-card {
    background: var(--bg-error-tint-soft);
    border-color: var(--border-error);
  }

  /* ---- Labels tab (Phase 3a — Slice 8) ----
     Panel is a relative container so the loading overlay can absolute-pos
     inside it without escaping. The list itself is a scrollable column —
     max-height clamps it so the page footer (Done/Cancel) stays reachable
     even with hundreds of labels. */

  .label-panel {
    position: relative;
    min-height: 200px;
  }

  /* Loading overlay markup + styles moved to $lib/LoadingOverlay.svelte
   * so any future blocking op (Save, Import, Export, etc.) can reuse it. */

  .label-error-card {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    align-items: flex-start;
    padding: var(--space-4);
    background: var(--bg-error-tint-soft);
    border: 1px solid var(--border-error);
    border-radius: var(--radius-md);
  }

  .label-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    max-height: 480px;
    overflow-y: auto;
    /* Slight inset so the scrollbar gutter doesn't crowd the rows. */
    padding-right: var(--space-2);
  }

  .label-row {
    /* Columns: [checkbox] [chip] [name] [count] [Details]. Checkbox
       column added in Phase 3a Slice 2 — the auto sizing keeps it
       flush with the chip while name grows to 1fr. */
    display: grid;
    grid-template-columns: auto auto 1fr auto auto;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-2) var(--space-3);
    background: var(--bg-surface);
    border: 1px solid var(--border-structural);
    border-radius: var(--radius-md);
  }
  /* Phase 3a Slice 2 — multi-select toolbar. Renders above the label list
     when any labels are present. Left side owns selection state; right
     side owns batch actions (hidden until N > 0). */
  .bulk-actions {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-2) var(--space-3);
    background: var(--bg-elevated);
    border: 1px solid var(--border-structural);
    border-radius: var(--radius-md);
    margin-bottom: var(--space-3);
  }
  .bulk-actions-spacer {
    flex: 1;
  }
  .bulk-select-all {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    cursor: pointer;
  }
  .bulk-select-all-text {
    font-family: var(--font-display);
    font-size: var(--text-caption);
    color: var(--text-secondary);
  }
  .bulk-clear {
    font-size: var(--text-caption);
  }

  /* Result / error banner after a bulk op. Persists until dismissed or
     until the user modifies selection. .is-error swaps in the error
     tokens so failures read as needs-attention rather than success. */
  .bulk-op-banner {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
    padding: var(--space-2) var(--space-3);
    background: var(--bg-elevated);
    border-radius: var(--radius-md);
    border-left: 3px solid var(--accent-primary);
    color: var(--text-secondary);
    font-size: var(--text-caption);
    line-height: var(--text-caption-lh);
    margin-bottom: var(--space-3);
  }
  .bulk-op-banner.is-error {
    background: var(--bg-error-tint-soft, var(--bg-error-tint));
    border-left-color: var(--border-error, var(--accent-pink));
    color: var(--text-primary);
  }

  /* Merge picker body — radio list of the selected labels. Radio group
     is bounded height + scrolls internally so a 50-label merge (unlikely
     but possible) doesn't push the actions off-screen. */
  .bulk-merge-body {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }
  .bulk-merge-list {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 260px;
    overflow-y: auto;
    border: 1px solid var(--border-structural);
    border-radius: var(--radius-sm, 4px);
  }
  .bulk-merge-list li + li {
    border-top: 1px solid var(--border-structural);
  }
  .bulk-merge-option {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-2) var(--space-3);
    cursor: pointer;
  }
  .bulk-merge-option:hover {
    background: var(--bg-elevated);
  }
  .bulk-merge-option input[type='radio'] {
    width: 16px;
    height: 16px;
    flex-shrink: 0;
    cursor: pointer;
  }
  .bulk-merge-name {
    font-family: var(--font-body);
    color: var(--text-primary);
  }
  /* Actions row inside the merge picker Modal body slot. Modal's slot
     renders children with Settings' scoped classes, so this rule reaches
     the <div class="modal-actions"> inside the bulk-merge-body. Standard
     right-aligned button row, matches the shape ConfirmDialog uses. */
  .modal-actions {
    display: flex;
    gap: var(--space-3);
    justify-content: flex-end;
    margin-top: var(--space-2);
  }

  .label-chip {
    display: inline-flex;
    align-items: center;
    padding: 2px var(--space-2);
    border: 1px solid var(--chip-color, var(--border-structural));
    color: var(--chip-color, var(--text-primary));
    border-radius: var(--radius-pill);
    font-family: var(--font-display);
    font-size: var(--text-caption);
    line-height: 1.2;
    /* Don't let very long names blow the chip out — the .label-name-text
       column carries the full name; the chip is the colored token. */
    max-width: 180px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .label-name-text {
    /* Hidden from sighted users when chip already shows the name; left in
       the grid for screen readers and to give the row a stable layout
       column even when the chip ellipsises. Visually muted so it doesn't
       compete with the chip. */
    font-size: var(--text-caption);
    color: var(--text-secondary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .label-count {
    font-family: var(--font-display);
    font-size: var(--text-caption);
    color: var(--text-secondary);
    background: var(--bg-elevated);
    border: 1px solid var(--border-decorative);
    border-radius: var(--radius-pill);
    padding: 2px var(--space-2);
    min-width: 28px;
    text-align: center;
  }
</style>
