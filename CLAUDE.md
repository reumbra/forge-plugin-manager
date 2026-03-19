# CLAUDE.md

## Project Overview

**forge-plugin-manager** — Tauri 2 desktop app for managing Forge plugins in Claude Cowork. Handles license activation, plugin catalog browsing, install/update/remove, and feedback.

**Status:** v0.4.0 — Dynamic catalog, dual-target install (Claude Code + Cowork), macOS builds.

## Ecosystem Contract

**Single source of truth** for cross-repo contracts (API, file paths, machine ID, install flow): `forge-devkit-api/docs/forge-ecosystem-contract.md`

When changing paths, API contracts, or install mechanics — update the contract document first, then implement in each repo.

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
│   ├── storage.rs      # Plugin storage, target detection, install/uninstall logic
│   ├── commands.rs     # Tauri commands (bridge frontend ↔ backend)
│   ├── error.rs        # Error types
│   ├── machine.rs      # Machine ID generation (SHA256)
│   └── cowork.rs       # (legacy, superseded by storage.rs)
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

### Personal accounts (implemented)

Plugin installation into personal Cowork accounts via `cowork_plugins/`:
- Marketplace dir: `cowork_plugins/marketplaces/reumbra/{plugin-name}/`
- Cache dir: `cowork_plugins/cache/reumbra/{plugin-name}/{version}/`
- Registry: `cowork_plugins/installed_plugins.json` (v2 format)
- Known marketplaces: `cowork_plugins/known_marketplaces.json`
- Key format: `{plugin-name}@reumbra`
- Detection: scan `{config_dir}/Claude/local-agent-mode-sessions/{session}/{user}/cowork_plugins/`

### Organization accounts (VERIFIED)

Org accounts have two plugin systems running in parallel:
- **`remote_cowork_plugins/`** — cloud-synced org plugins (read-only, managed by Anthropic)
- **`cowork_plugins/`** — personal plugins (writable, same format as personal account)

Org sessions do NOT create `cowork_plugins/` by default. Our approach: create it alongside `remote_cowork_plugins/` with the same 4-location format as personal accounts. Cowork internally uses Claude CLI with `--cowork` flag, which reads `cowork_plugins/` independently.

**Detection:**
- Personal account: has `cowork_plugins/` dir
- Org account: has `remote_cowork_plugins/` with non-empty `manifest.json`, may or may not have `cowork_plugins/`

**Path migration:** Claude Desktop v1.1.4498+ uses `claude-code-sessions/` instead of `local-agent-mode-sessions/` — code checks both.

See ecosystem contract (section 9) for full path documentation and env vars.

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
