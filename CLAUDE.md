# CLAUDE.md

## Project Overview

**forge-plugin-manager** — Tauri 2 desktop app for managing Forge plugins in Claude Cowork. Handles license activation, plugin catalog browsing, install/update/remove, and feedback.

**Status:** v0.1.0 — Initial scaffold, all screens and backend commands implemented.

## Tech Stack

- **Framework:** Tauri 2 (Rust backend + WebView frontend)
- **Frontend:** React 19 + TypeScript + Tailwind CSS + Vite
- **Backend:** Rust (reqwest for HTTP, zip for extraction, serde for JSON)
- **Package manager:** pnpm
- **CI/CD:** GitHub Actions (CI on push, release builds on tag)

## Architecture

```
src/                    # React frontend
├── App.tsx             # Router + license state
├── lib/api.ts          # Tauri invoke wrappers (typed)
├── components/         # Sidebar
└── pages/              # Activation, Catalog, Installed, Settings, Feedback

src-tauri/              # Rust backend
├── src/
│   ├── lib.rs          # Tauri builder + command registration
│   ├── main.rs         # Entry point
│   ├── api.rs          # HTTP client for forge-devkit-api
│   ├── cowork.rs       # Cowork directory detection + plugin management
│   ├── commands.rs     # Tauri commands (bridge frontend ↔ backend)
│   ├── error.rs        # Error types
│   └── machine.rs      # Machine ID generation (SHA256)
├── tauri.conf.json     # Tauri config (window, CSP, updater)
└── Cargo.toml          # Rust dependencies
```

## API Integration

All API calls go to `https://api.reumbra.com/velvet` (forge-devkit-api):
- `POST /auth/activate` — license activation
- `POST /auth/deactivate` — deactivation
- `GET /auth/status` — license status
- `GET /plugins/list` — plugin catalog
- `POST /plugins/download` — presigned S3 URL
- `GET /plugins/versions/:name` — version history
- `POST /feedback` — user feedback

## Cowork Integration

Plugin installation mimics Cowork's native format:
- Files go to `cowork_plugins/cache/reumbra-plugins/{name}/{version}/`
- Registry updated in `cowork_plugins/installed_plugins.json`
- Key format: `{plugin-name}@reumbra-plugins`
- Auto-detect paths: Win `%APPDATA%/Claude`, Mac `~/Library/Application Support/Claude`

## Development

```bash
pnpm install            # Install frontend deps
pnpm dev                # Dev server (frontend only, port 1420)
pnpm build              # Build frontend
pnpm tauri dev          # Full Tauri dev mode (requires system deps)
pnpm tauri build        # Production build
```

### System Dependencies (Linux)
```bash
sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf
```

## CI/CD

- **CI:** `ci.yml` — TypeScript check + frontend build + cargo check + clippy (on push/PR to main)
- **Release:** `release.yml` — Cross-platform Tauri builds on tag `v*`
  - Windows: `.exe` / `.msi`
  - macOS: `.dmg` (ARM64 + x86_64)
  - Linux: `.AppImage` / `.deb`
  - Artifacts attached to GitHub Release automatically

## Conventions

- Rust edition 2021, release profile: strip + LTO + single codegen unit
- React: functional components, hooks only
- Tailwind: dark theme (gray-950 base), custom `forge-*` color palette
- All Tauri commands return `Result<T, AppError>` (serialized to frontend)
