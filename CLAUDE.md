# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

SnapX is a lightweight screenshot and annotation tool built with Tauri v2 + Rust backend + Svelte 5 frontend. Package manager is **pnpm** (enforced).

## Commands

```bash
# Full dev mode (Vite hot reload + Rust watch)
pnpm tauri dev

# Type-check frontend
pnpm check

# Rust-only build/check
cd src-tauri && cargo check
cd src-tauri && cargo clippy

# Production build
pnpm tauri build
```

No test commands are configured yet. No frontend linting setup exists.

## Architecture

### Dual-Window Design

The app runs two Tauri windows that share the same Svelte frontend bundle:

- **`main`** — Hidden at startup (visible=false). Receives the `trigger-capture` event from tray/shortcut, calls Rust capture commands, then emits `show-overlay` to the overlay window.
- **`overlay`** — Fullscreen, transparent, always-on-top, no decorations. Renders the region-selection UI. Appears only during capture.

`App.svelte` checks `getCurrent().label` to branch logic between the two windows.

### Capture Flow

1. Tray menu or `Ctrl+Shift+X` global shortcut → Rust emits `trigger-capture` with mode (`"region"`, `"window"`, `"fullscreen"`)
2. Main window receives event → calls `invoke('capture_screens')` → Rust (xcap) captures full monitor, returns base64 PNG
3. Main window emits `show-overlay` with image data → overlay window renders fullscreen screenshot
4. User drags to select region → `CaptureOverlay.svelte` tracks mouse coords
5. User clicks Copy → `invoke('capture_region', {x, y, width, height})` or canvas crop fallback → `invoke('save_to_clipboard')`

### Rust–Frontend Bridge

All cross-boundary calls use Tauri's `invoke()` (RPC) and `emit()`/`listen()` (events). Screenshots are transmitted as **base64-encoded PNG strings** — not binary blobs.

Key Rust commands (`src-tauri/src/commands.rs`):
- `capture_screens(monitor_index?)` → `CaptureResult { image_data, width, height, monitor_id }`
- `capture_region(monitor_index, x, y, width, height)` → `CaptureResult`
- `save_to_clipboard(image_data)`
- `list_monitors()` → `Vec<MonitorInfo>`

### Frontend Rendering

`CaptureOverlay.svelte` uses an **SVG mask** to create the selection hole effect: a white rect covers the whole screen (darkened), and a black rect cut-out at the selection coordinates shows the original screenshot underneath. Corner handles and the size indicator are rendered on top.

### Key Files

| File | Role |
|------|------|
| `src-tauri/src/lib.rs` | Tauri setup: tray menu, global shortcut registration, window config |
| `src-tauri/src/commands.rs` | All `#[tauri::command]` handlers |
| `src-tauri/src/capture.rs` | xcap wrapper + `MonitorInfo`/`CaptureResult` structs |
| `src/App.svelte` | Entry component; dual-window routing via window label |
| `src/lib/components/CaptureOverlay.svelte` | Region selection UI (SVG mask, drag logic) |
| `src-tauri/capabilities/default.json` | Tauri permissions for both windows |
| `src-tauri/tauri.conf.json` | Window definitions, tray, bundle targets |

## Important Details

- **Coordinates**: `CaptureOverlay` uses client/viewport coordinates. DPI scaling (`scale_factor`) is captured but not yet fully applied in rendering.
- **Multi-monitor**: `xcap` enumerates all monitors; the app currently defaults to monitor index 0.
- **UI language**: All UI text is Simplified Chinese.
- **Status**: MVP (~25% complete). Annotation tools (module B) and image pinning (module C) are not yet implemented — see `ROADMAP.md`.
