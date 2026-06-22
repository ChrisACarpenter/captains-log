# Captain's Log — App

The application source. Tauri 2.0 backend (Rust) + SvelteKit (Svelte 5, TypeScript) frontend.

For project overview, vision, and roadmap, see the [top-level README](../README.md) and [docs/](../docs/).

## Stack

- **Tauri 2.0** — desktop framework (Rust backend, system WebView frontend)
- **SvelteKit 2 + Svelte 5** — frontend, with `@sveltejs/adapter-static` for SPA output
- **TypeScript** — across the frontend
- **Vite** — dev server and bundler

## Layout

```
app/
├── src/                  # Svelte frontend
│   ├── app.html
│   └── routes/           # SvelteKit file-based routing
├── src-tauri/            # Rust backend
│   ├── src/
│   │   ├── main.rs       # Entry point
│   │   └── lib.rs        # Library + Tauri builder
│   ├── Cargo.toml
│   └── tauri.conf.json   # Window config, bundle settings
├── static/               # Static assets (favicons, fonts)
├── package.json
└── vite.config.js
```

## Prerequisites

- **Node.js 20+** (developed against Node 25.x)
- **Rust** stable toolchain (install via [rustup](https://rustup.rs/))
- **macOS:** Xcode Command Line Tools
- **Windows:** Microsoft Visual C++ Build Tools
- **Linux:** see [Tauri prerequisites](https://tauri.app/start/prerequisites/)

## Setup

```bash
npm install
```

Note: this directory has a project-local `.npmrc` that points npm at the public registry (`https://registry.npmjs.org/`). It overrides any user-level config (e.g., a private corporate registry). Don't remove it — the scaffold and all subsequent installs need the public registry.

## Development

```bash
npm run tauri dev
```

Hot-reload is enabled for the frontend (Vite). The Rust backend recompiles automatically on save.

## Type-check

```bash
npm run check
```

Runs `svelte-kit sync` then `svelte-check` against `tsconfig.json`. Rust is checked when you run `cargo check` or `npm run tauri dev`.

## Build

```bash
npm run tauri build
```

Produces a `.app` on macOS, `.exe` on Windows, etc., under `src-tauri/target/release/bundle/`.

## Troubleshooting

- **npm auth errors** — confirm `app/.npmrc` exists and points to `https://registry.npmjs.org/`. If your `~/.npmrc` has expired tokens for a private registry, this file shields the project.
- **Cargo not found** — run `. "$HOME/.cargo/env"` or restart your shell after rustup install.
- **Vite port conflict** — `tauri.conf.json`'s `devUrl` is `http://localhost:1420`. Vite must serve on this port; the scaffold's `vite.config.js` enforces it.
