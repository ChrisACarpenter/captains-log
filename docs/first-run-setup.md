# First-Run Setup

What happens the first time a user opens Captain's Log.

## Goal

Get the user from "just installed" to "ready to capture" in under 90 seconds. Most fields are optional and skippable.

## Flow

The wizard ships as 5 steps. Implementation lives in [`app/src/lib/onboarding/`](../app/src/lib/onboarding/) — `Wizard.svelte` owns the orchestration; each step has its own component (`StepIntro`, `StepAboutYou`, `StepAboutManager`, `StepSettings`, `StepComplete`). Shared chrome: [StepHeader](../app/src/lib/onboarding/StepHeader.svelte), [TipBubble](../app/src/lib/onboarding/TipBubble.svelte), [PathPickerField](../app/src/lib/PathPickerField.svelte), [InputField](../app/src/lib/InputField.svelte), [PointerFinger](../app/src/lib/PointerFinger.svelte), [WizardFrame](../app/src/lib/onboarding/WizardFrame.svelte).

### Step 1 — Welcome (Intro)

```
Welcome to Captain's Log.

A weekly work journal with tools to help you write self reviews.

  [ Let's get started ]
```

Brand-voice intro. Single CTA. No fields.

### Step 2 — Tell me about you

```
Tell me about you.

Helps personalize the app. Anything you skip you can always set later
in Settings.

  What should we call you?            [ ____________________ ]
  Your email                          [ ____________________ ]
  Your job title (Bamboo)             [ ____________________ ]
  Jira project keys (comma-separated) [ ____________________ ]

  Tip: These show up later in features like the weekly Send-to-Manager
  email (your title in the signature) and link enrichment for Jira
  tickets.

  [ Back ]                          [ Continue ]
```

All four fields optional. The Bamboo link in the tip opens BambooHR in the system browser. Jira keys normalize to all-caps on save (whitespace-trimmed, empties dropped).

### Step 3 — Tell me about your manager

```
Tell me about your manager.

Personalizes the weekly email greeting and pre-fills the To: field. Leave blank to send to whoever you like at the time.

  Their name                          [ ____________________ ]
  Their email                         [ ____________________ ]

  Tip: Used to personalize the Send weekly summary button on the
  /summary screen — name in the greeting, email in the To: field.

  [ Back ]                          [ Continue ]
```

### Step 4 — Settings (journal location + reminders)

```
A few last settings.

Where your journal lives, and whether Captain's Log should nudge you to write a weekly summary.

  Folder        [ ~/Documents/CaptainsLog/ ] [ Browse… ]
                Plain markdown on your machine. You can move it later.

  ☑ Send me a weekly reminder to fill in the Weekly Summary
    Day        [ Friday ]
    Time       [ 4:00 PM ]

  [ Back ]                          [ Finish setup ]
```

- Folder picker uses the shared [PathPickerField](../app/src/lib/PathPickerField.svelte) component (label + path input + Browse button + Tauri dialog). If the chosen folder doesn't exist, it's created. If it exists and already contains journal data, the existing data is used as-is.
- Reminders default to off. The wizard collects a single day via a dropdown (persisted as a one-element `daysOfWeek` array); the multi-day pill picker lives on the Settings > Reminder tab, not the wizard. Time defaults to 4:00 PM (end-of-week reflection time).
- "Finish setup" calls `complete_first_run` on the Rust side — writes settings, hot-swaps the storage layer, requests notification permission if reminders are enabled, and starts the scheduler.

### Step 5 — All set

```
You're all set, Chris.

Captain's Log is ready when you are.

A few places to start:
  - Tap the menu-bar icon (top-right) to capture a quick Note.
  - Open the /summary screen at the end of the week.
  - Browse past weeks from the sidebar in the journal window.

  [ Start journaling… ]
```

Heading personalizes when a name was provided ("You're all set, {name}.") and falls back to "You're all set." otherwise.

## After setup

### Immediate initialization

Triggered by `complete_first_run`:

- `~/Library/Application Support/com.prodigygame.captainslog/app-settings.json` written (theme, journal root pointer, `firstRunComplete: true`)
- `<root>/.metadata/settings.json` written (per-journal — user details, manager, reminder, mail mode)
- Storage layer hot-swaps to the chosen journal root (no app restart needed)
- Notification permission requested via macOS UN if reminders enabled
- Journal-reminder scheduler started; the first reminder fires at the next matching day/time
- Task-reminder scheduler started so due-date reminders fire even if the wizard didn't collect task-reminder settings
- Activation policy flips to `.Regular` so the Dock icon appears
- Menu-bar tray icon already present from launch

### Deferred initialization

Everything else is created lazily on first use of the feature that owns it. No file is pre-written by the wizard.

- `<root>/.metadata/labels.json` — on the first label added to a note (`LabelIndex::load` returns an empty index if the file is missing, then subsequent writes materialize it)
- `<root>/.metadata/sent-log.json` — on the first successful Send-to-Manager
- `<root>/.metadata/capture-draft.json` — on the first keystroke into the quick-capture window (debounced auto-save)
- `<root>/.metadata/task-completions.json` — on the first task checkbox toggle
- `<root>/.metadata/task-due-dates.json` — on the first due date attached to a task
- `<root>/.metadata/rollover-log.json` — on the first rollover from one weekly file to the next
- `<root>/.metadata/auto-import-log.json` — on the first auto-import from a legacy note
- `<root>/.metadata/pre-slice6-backups/YYYY-Www.md` — on the first migration of a pre-Slice-6 weekly file (one backup per migrated week)

## Re-running the wizard

The wizard re-shows when `app-settings.json` is missing or `firstRunComplete` is false. For QA / iteration, deleting `~/Library/Application Support/com.prodigygame.captainslog/app-settings.json` retriggers it on next launch.
