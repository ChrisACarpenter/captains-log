# First-Run Setup

What happens the first time a user opens Captain's Log.

## Goal

Get the user from "just installed" to "captured my first Note" in under 60 seconds.

## Flow

### Step 1 — Welcome

Single screen, brand voice, sets the stage.

> **Welcome to Captain's Log.**
>
> A weekly work journal that makes self-reviews painless.
> Capture what you do as you do it — Captain's Log handles the rest.

Single button: **Get started**

### Step 2 — Your name

```
What should we call you?

[ Chris ____________________ ]

This is just for the app — your journal stays on your machine.

[ Back ]    [ Continue ]
```

Used for:

- UI greetings ("Welcome back, Chris")
- Future multi-user scenarios, if we ever go there
- The auto-generated README at the journal root

### Step 3 — Where should we store your journal?

```
Where should we store your journal files?

📁 ~/Documents/CaptainsLog/  (recommended)
   └─ [ Use this location ]

📁 Choose another location...
   └─ [ Browse ]

Your journal is yours. Everything is plain markdown on your machine.
You can move it later in Settings.

[ Back ]    [ Continue ]
```

- If the chosen location doesn't exist, create it.
- If it exists and already contains journal data, offer to "use existing data" or "pick a different folder."

### Step 4 — Reminders (optional)

```
Want a weekly nudge to fill in your Weekly Summary?

[ Yes, remind me ]    [ No thanks ]

(If yes:)
What day and time?
[ Friday  ▾ ] at [ 4:00 PM ▾ ]

[ Back ]    [ Continue ]
```

Default: Friday at 4:00 PM (end-of-week reflection time). User can change or disable later in Settings.

### Step 5 — You're set

```
🚀 You're set.

Click the 🧭 icon in your menu bar (top right) to capture a Note.
The Captain's Log window is in your Dock if you want to browse.

[ Open Captain's Log ]
```

## After setup

- `settings.json` is written
- `journals/2026/2026-Www.md` (current week) is created with an empty Weekly Summary scaffold
- `.metadata/labels.json` initialized as an empty array
- Menu bar and Dock icons appear
- (If reminders enabled) Notification scheduler kicks off
