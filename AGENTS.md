# AGENTS.md — jPaste v2

Tauri 2 desktop clipboard manager (Windows-only). Frontend in Preact + TypeScript, backend in Rust.

## Commands

```bash
pnpm install              # install frontend deps (pnpm@10, frozen lockfile)
pnpm tauri dev            # dev mode — Vite :3420 (strict), opens Tauri window
pnpm tauri build          # release build → src-tauri/target/release/bundle/
pnpm test                 # vitest run (jsdom, src/**/*.test.{ts,tsx})
pnpm test:watch           # vitest in watch mode
cargo test --manifest-path src-tauri/Cargo.toml   # Rust unit tests
```

There is no lint or typecheck script beyond `tsc` (invoked by `pnpm build`). `tsconfig.json` sets `noUnusedLocals` and `noUnusedParameters` — the build fails on unused bindings.

## Architecture

**Two-language Action system.** Each Action has `detect()` + `handler()`. Rust side (~30 lines per action) powers toast enhancement; TypeScript side powers main-window list rendering. They are independently implemented — changing one does not auto-update the other.

**Frontend** (`src/`): Preact SPA routed by `wouter` via hash paths (`useHashPathLocation`). Entry point `src/main.tsx` resolves the initial hash from `__TAURI__` window label, then renders `<App />`. Routes in `src/app.tsx` map to pages under `src/routes/` and viewer features under `src/features/`.

**Action registration** is side-effect based: `src/actions/index.ts` imports every `features/<name>/action`, each of which calls `register()` on import. The registry (`src/actions/registry.ts`) returns up to 3 matches sorted by priority.

**Backend** (`src-tauri/src/`): `lib.rs` wires plugins, services, and the clipboard worker thread. `command/` holds all Tauri commands grouped by domain (clipboard, history, quicklaunch, share_server, viewer, etc.). Shared state is `AppState` (history, settings, filostack, clipboard manager, keyboard hook, launch-hotkey map).

**Windows**: main (`/`), toast (`/toast`, borderless, auto-hides after 3s), and per-content viewer windows (`/viewer/<type>`). Viewer window labels follow `<type>-viewer-<entryId>`.

**ShareServer**: long-lived HTTP service (axum) bound to the SharePanel viewer window lifecycle. Random port per session, listens on `0.0.0.0`, enumerates physical NIC IPv4s only.

## Platform

Windows-only. The `windows` crate dependency and `windows_subsystem` attribute in `main.rs` mean `cargo build`/`tauri dev` must run on Windows. CI builds on `windows-latest`.

## Conventions

- React-compatible libs (wouter, jsoneditor) work via `vite.config.ts` alias `react → preact/compat`. Use Preact hooks/syntax; do not import from `react` directly.
- Toast window identity is detected via `sessionStorage['__TOAST_WINDOW__']` in `main.tsx` — do not remove this marker.
- Global shortcuts (main hotkey + QuickLauncher targets) are mutually exclusive in a single registry. QuickLauncher hotkeys validate against both other targets and OS-registered shortcuts at save time.
- `withGlobalTauri: true` is set — frontend uses `@tauri-apps/api` directly, not `__TAURI__` IPC.

## Release

Tag `v*` triggers `.github/workflows/release.yml`: builds MSI (preferred) + NSIS exe + portable exe, uploads to GitHub Release. Requires `TAURI_SIGNING_PRIVATE_KEY` secret.
