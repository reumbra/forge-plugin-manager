# Changelog

## 0.6.0 - 2026-05-29

- Breaking change: relocated the Claude Code marketplace target to the canonical `~/.claude/plugins/marketplaces/reumbra/` path so Claude Desktop accepts installed plugin paths in Code view.
- Added copy-only migration from legacy `forge-devkit/marketplace/` installs when the canonical marketplace does not already exist.
- Added registry-path rewrite step that updates existing `known_marketplaces.json` and `installed_plugins.json` entries from legacy to canonical paths on first v0.6.0 install. Customers upgrading from v0.5.3 no longer need to reinstall their plugins for them to be picked up by Code view.
- Updated Claude Code registry metadata to write canonical marketplace and install paths.
- Deprecated legacy `cowork_plugins/` integration helpers ahead of removal in v0.7.0.

## 0.5.3 - 2026-05-28

- Fixed Claude Code plugin installation metadata so `installed_plugins.json` points to the extracted marketplace plugin directory.
- Normalized the local marketplace owner field to the object schema expected by Claude Desktop.
- Removed Claude Code `installed_plugins.json` entries during uninstall.
