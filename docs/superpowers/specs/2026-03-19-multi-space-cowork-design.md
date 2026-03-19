# Multi-Space Cowork Support

## Problem

Plugin manager only detects one `cowork_plugins/` directory (personal account). Users with organizational accounts need to install personal plugins there too. Plugins installed in personal session are invisible in org session.

## Verified Mechanism

Creating `cowork_plugins/` + `cowork_settings.json` in org account directory works. Cowork reads both `cowork_plugins/` (personal) and `remote_cowork_plugins/` (org-synced) independently. Tested 2026-03-19, survives restart.

## Design

### Backend Changes (storage.rs)

**New struct:**
```rust
#[derive(Debug, Serialize, Clone)]
pub struct CoworkSpace {
    pub id: String,              // stable hash of path
    pub label: String,           // "Personal" | org label from manifest
    pub path: String,            // full path to account dir (parent of cowork_plugins/)
    pub is_org: bool,            // has remote_cowork_plugins with non-empty manifest
    pub has_cowork_plugins: bool, // cowork_plugins/ already exists
}
```

**TargetInfo changes:**
```rust
pub struct TargetInfo {
    pub claude_code: bool,
    pub claude_code_path: Option<String>,
    pub cowork_spaces: Vec<CoworkSpace>, // replaces claude_cowork + cowork_path
}
```

**Scanning logic:**
- Check both `claude-code-sessions/` and `local-agent-mode-sessions/`
- For each `{session}/{account}/` dir:
  - Has `cowork_plugins/` → include (personal or org with our plugins)
  - Has `remote_cowork_plugins/manifest.json` with non-empty plugins array → is_org=true
- Deduplicate by account UUID (same account appears in multiple session dirs)
- Org label: extract from manifest's first plugin marketplaceName, or "Organization"

**InstallTarget changes:**
- `ClaudeCowork` becomes `ClaudeCowork { space_id: String }`
- Install resolves space_id → path, creates `cowork_plugins/` + `cowork_settings.json` if missing
- Uninstall uses same space_id resolution

### Frontend Changes

**api.ts types:**
```typescript
interface CoworkSpace {
    id: string;
    label: string;
    path: string;
    is_org: boolean;
    has_cowork_plugins: boolean;
}

interface TargetInfo {
    claude_code: boolean;
    claude_code_path: string | null;
    cowork_spaces: CoworkSpace[];
}
```

**Catalog page:**
- Toggle "Cowork | Code" unchanged
- When Cowork selected AND spaces.length > 1: show dropdown below toggle
- Preselect: first is_org space, fallback to first space
- When spaces.length === 1: dropdown hidden, auto-select
- When spaces.length === 0: Cowork toggle disabled

**Installed page badges:**
- Current `Cowork` badge → `Cowork · {label}` (e.g., "Cowork · Personal", "Cowork · GoodieMate")
- One plugin can have multiple Cowork badges
- × button on each badge removes from that specific space

**InstallTarget in API calls:**
- `'claude-code'` stays as-is
- Cowork becomes `{ cowork: space_id }` object

### Migration

- `cowork_path: Option<String>` removed from TargetInfo
- `claude_cowork: bool` removed — derived from `cowork_spaces.len() > 0`
- Frontend `getCoworkPath()` command removed (legacy, already no-op)
