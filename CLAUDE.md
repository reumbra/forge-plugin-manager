# CLAUDE.md

## Project Overview

**forge-plugin-manager** — Tauri 2 desktop app for managing Forge plugins in Claude Cowork. Handles license activation, plugin catalog browsing, install/update/remove, and feedback.

**Status:** v0.6.0 — canonical install path migration (Phase F complete)

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

## Plugin Installation Architecture

### Empirically verified 2026-05-28 — supersedes all prior Cowork integration assumptions

**Single canonical install target** for Claude Desktop (Cowork view + Code view) and Claude Code CLI: `~/.claude/plugins/marketplaces/<marketplace-name>/`. There is no longer any separate "Cowork install" — both views read from this same location.

### Required layout

```
~/.claude/plugins/                                                  ← CLI store root
├── known_marketplaces.json                                         ← marketplace registry
├── installed_plugins.json                                          ← installed list + installPath
└── marketplaces/<marketplace>/                                     ← marketplace dir
    ├── .claude-plugin/marketplace.json                             ← catalog
    │   └── owner MUST be {name, email} object (not string)         ← Cowork schema validation
    └── plugins/<plugin>/                                           ← plugin files (NO version subdir)
        ├── .claude-plugin/plugin.json
        ├── skills/
        ├── agents/
        └── commands/

~/.claude/settings.json                                             ← enabledPlugins["<plugin>@<marketplace>"] = true
```

Windows host path: `%USERPROFILE%\.claude\plugins\...`. macOS/Linux: `~/.claude/plugins/...`.

`cache/` subdir is **only** used by Claude Code for github-source marketplaces (it fetches + caches there). For our directory-source `reumbra`, the marketplace dir IS the install location — no caching needed.

### Hard rules enforced by Claude Desktop

1. **Out-of-bounds installPath rejection** — `[LocalPluginsReader] Skipping plugin with invalid path` appears for any path outside `~/.claude/`. **Never** write install targets to `%APPDATA%/forge-devkit/`, `%LOCALAPPDATA%/`, or anywhere outside the user's `.claude` dir.

2. **`owner` schema** — `marketplace.json::owner` must be `{name, email}` object. String values cause `[CCDMarketplacePluginManagerCLI] Failed to refresh marketplace: ... owner: Invalid input: expected object, received string`.

3. **No locally-faked rpm entries** — `local-agent-mode-sessions/<owner>/<workspace>/rpm/manifest.json` is server-authoritative. Desktop wipes any entry whose `marketplaceId` doesn't resolve against Anthropic's backend on next launch. Code view's "Anthropic" tab is reserved for Anthropic-curated marketplaces.

### Legacy code to retire (Phase F+)

- **`src-tauri/src/cowork.rs`** — fully legacy. Targets `cowork_plugins/` store in `local-agent-mode-sessions/<owner>/<workspace>/`, which the current Desktop only reads as fallback for the Cowork sidebar (never writes). Delete after Phase F migration ships.
- **`integrate_cowork_space()`** in `storage.rs` — same legacy path family. Same fate.
- **Workspace selector for Cowork target** in UI — obsolete; single global install target now covers both views. Phase F should simplify UI to one "Install" button.

### Migration plan (Phase F brief, pending)

For existing customer installs already at `%APPDATA%/forge-devkit/marketplace/`:
1. On first launch of fixed Plugin Manager, detect old marketplace dir
2. Move (or copy) to `~/.claude/plugins/marketplaces/reumbra/`
3. Rewrite `known_marketplaces.json::reumbra::source.path` and `installLocation` to new absolute path
4. Rewrite each `installed_plugins.json::<key>::installPath` to `~/.claude/plugins/marketplaces/reumbra/plugins/<name>/`
5. Optionally delete old `%APPDATA%/forge-devkit/marketplace/` after successful migration

For new installs: write directly to canonical location, no migration.

See `memory/claude-desktop-plugin-architecture.md` in the AI Marketplace repo for the full evidence trail (log lines, security gate names, store comparison) behind these rules.

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
