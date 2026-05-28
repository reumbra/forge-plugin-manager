# Changelog

## 0.6.0 - 2026-05-28

- Breaking change: relocated the Claude Code marketplace target to the canonical `~/.claude/plugins/marketplaces/reumbra/` path so Claude Desktop accepts installed plugin paths in Code view.
- Added copy-only migration from legacy `forge-devkit/marketplace/` installs when the canonical marketplace does not already exist.
- Updated Claude Code registry metadata to write canonical marketplace and install paths.
- Deprecated legacy `cowork_plugins/` integration helpers ahead of removal in v0.7.0.

## 0.5.3 - 2026-05-28

- Fixed Claude Code plugin installation metadata so `installed_plugins.json` points to the extracted marketplace plugin directory.
- Normalized the local marketplace owner field to the object schema expected by Claude Desktop.
- Removed Claude Code `installed_plugins.json` entries during uninstall.
