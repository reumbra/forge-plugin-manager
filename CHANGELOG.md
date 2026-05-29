# Changelog

## 0.6.1 - 2026-05-29

- Fixed: do not canonicalize the plugin install path before writing it to `installed_plugins.json`. On Windows `std::fs::canonicalize` returns a `\\?\C:\...` extended-length verbatim path, which Claude Desktop's `LocalPluginsReader` rejects with an out-of-bounds check. The result was that v0.6.0 installs registered correctly enough for slash commands to work, but were hidden from the Customize UI in both Cowork and Code views. Path is now written as the plain absolute path returned by `marketplace_dir()`.

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
