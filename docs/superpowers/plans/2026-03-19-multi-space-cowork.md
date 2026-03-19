# Multi-Space Cowork Support — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let users install/uninstall plugins to multiple Cowork spaces (personal + org accounts) with a space selector dropdown and per-space badges.

**Architecture:** Backend scans all session dirs for cowork spaces, returns `Vec<CoworkSpace>`. Frontend shows dropdown when >1 space, preselects org. Installed page shows `Cowork · {label}` badges. Web mode has demo spaces for UI testing.

**Tech Stack:** Rust (storage.rs), React 19 + TypeScript + Tailwind CSS

---

### File Map

| File | Action | Responsibility |
|------|--------|---------------|
| `src/lib/api.ts` | Modify | Types + demo data for CoworkSpace |
| `src/pages/Catalog.tsx` | Modify | Space dropdown, preselect logic |
| `src/pages/Installed.tsx` | Modify | Per-space badges with labels |
| `src-tauri/src/storage.rs` | Modify | CoworkSpace detection + install by space_id |
| `src-tauri/src/commands.rs` | Modify | Updated command types |

---

### Task 1: Frontend types + demo data (api.ts)

**Files:**
- Modify: `src/lib/api.ts`

- [ ] **Step 1: Update TargetInfo and add CoworkSpace type**

Replace the existing types at lines 24 and 43-48:

```typescript
export interface CoworkSpace {
  id: string;
  label: string;
  path: string;
  is_org: boolean;
  has_cowork_plugins: boolean;
}

export interface TargetInfo {
  claude_code: boolean;
  claude_code_path: string | null;
  cowork_spaces: CoworkSpace[];
}

// InstallTarget: 'claude-code' for Code, or space_id string for Cowork
export type InstallTarget = string;
```

- [ ] **Step 2: Update demo data in getAppInfo web fallback**

```typescript
// In getAppInfo() web fallback, return demo spaces:
return {
  version: '0.4.0 (web)',
  machine_id: webMachineId,
  targets: {
    claude_code: true,
    claude_code_path: null,
    cowork_spaces: [
      { id: 'personal-abc', label: 'Personal', path: '/mock/personal', is_org: false, has_cowork_plugins: true },
      { id: 'org-xyz', label: 'GoodieMate', path: '/mock/org', is_org: true, has_cowork_plugins: false },
    ],
  },
  config_dir: null,
  os: navigator.platform,
};
```

- [ ] **Step 3: Update installPlugin and uninstallPlugin signatures**

`installPlugin` already passes `target` as string — no change needed since `InstallTarget` is now `string`.

Update `uninstallPlugin` the same way — it already takes `InstallTarget`.

- [ ] **Step 4: Remove deprecated getCoworkPath/setCoworkPath functions**

Delete `getCoworkPath()` and `setCoworkPath()` (legacy, no-op).

- [ ] **Step 5: Update InstalledPlugin.targets to include space labels**

The backend will return targets as `["claude-code", "cowork:personal-abc:Personal", "cowork:org-xyz:GoodieMate"]` — a string encoding `cowork:{id}:{label}`.

No type change needed (already `string[]`), but update the web demo `installPlugin` return:

```typescript
targets: [target === 'claude-code' ? 'claude-code' : `cowork:${target}:Demo`],
```

- [ ] **Step 6: Verify web mode starts**

Run: `pnpm dev`
Expected: Dev server starts on port 1420 without TS errors.

- [ ] **Step 7: Commit**

```
feat: update api.ts types for multi-space cowork support
```

---

### Task 2: Catalog page — space dropdown (Catalog.tsx)

**Files:**
- Modify: `src/pages/Catalog.tsx`

- [ ] **Step 1: Replace target state with dual state**

Replace:
```typescript
const [target, setTarget] = useState<InstallTarget>('claude-cowork');
```

With:
```typescript
const [targetType, setTargetType] = useState<'cowork' | 'code'>('cowork');
const [selectedSpaceId, setSelectedSpaceId] = useState<string>('');
```

- [ ] **Step 2: Update auto-select logic in loadData**

After `setAppInfo(info)`:
```typescript
if (info) {
  const spaces = info.targets.cowork_spaces;
  if (spaces.length > 0) {
    setTargetType('cowork');
    // Preselect: first org space, fallback to first space
    const orgSpace = spaces.find(s => s.is_org);
    setSelectedSpaceId(orgSpace?.id ?? spaces[0].id);
  } else if (info.targets.claude_code) {
    setTargetType('code');
  }
}
```

- [ ] **Step 3: Derive effective target for install calls**

```typescript
const effectiveTarget = targetType === 'code' ? 'claude-code' : selectedSpaceId;
```

Use `effectiveTarget` in `handleInstall` and `isInstalledForTarget`.

- [ ] **Step 4: Update hasTargets / hasBothTargets**

```typescript
const hasCowork = (appInfo?.targets.cowork_spaces.length ?? 0) > 0;
const hasCode = appInfo?.targets.claude_code ?? false;
const hasTargets = hasCowork || hasCode;
const hasBothTargets = hasCowork && hasCode;
const spaces = appInfo?.targets.cowork_spaces ?? [];
```

- [ ] **Step 5: Update isInstalledForTarget**

```typescript
const isInstalledForTarget = (name: string) => {
  const plugin = installed.find((p) => p.name === name);
  if (!plugin?.targets) return false;
  if (targetType === 'code') return plugin.targets.includes('claude-code');
  return plugin.targets.some(t => t.startsWith(`cowork:${selectedSpaceId}:`));
};
```

- [ ] **Step 6: Add space dropdown below toggle**

After the existing toggle div, add:
```tsx
{targetType === 'cowork' && spaces.length > 1 && (
  <select
    value={selectedSpaceId}
    onChange={(e) => setSelectedSpaceId(e.target.value)}
    className="ml-2 px-2 py-1.5 text-xs bg-gray-900 border border-gray-700 rounded-lg text-gray-300 focus:border-forge-500 focus:outline-none"
  >
    {spaces.map((s) => (
      <option key={s.id} value={s.id}>
        {s.label}{s.is_org ? ' (org)' : ''}
      </option>
    ))}
  </select>
)}
```

- [ ] **Step 7: Update success message**

```typescript
const targetLabel = targetType === 'code'
  ? 'Claude Code'
  : `Cowork · ${spaces.find(s => s.id === selectedSpaceId)?.label ?? 'Cowork'}`;
```

- [ ] **Step 8: Visual verification via Playwright**

Open browser, navigate to catalog, verify:
1. Toggle shows "Claude Cowork | Claude Code"
2. Dropdown appears with "GoodieMate (org)" preselected and "Personal" option
3. Switching toggle hides dropdown

- [ ] **Step 9: Commit**

```
feat: add cowork space dropdown to catalog page
```

---

### Task 3: Installed page — per-space badges (Installed.tsx)

**Files:**
- Modify: `src/pages/Installed.tsx`

- [ ] **Step 1: Parse cowork targets into structured data**

Add helper:
```typescript
function parseCoworkTargets(targets: string[]): { id: string; label: string }[] {
  return targets
    .filter(t => t.startsWith('cowork:'))
    .map(t => {
      const [, id, ...labelParts] = t.split(':');
      return { id, label: labelParts.join(':') || 'Cowork' };
    });
}
```

- [ ] **Step 2: Replace single Cowork badge with per-space badges**

Replace the `{inCowork && (...)}` block with:
```tsx
{coworkTargets.map((ct) => (
  <div key={ct.id} className="inline-flex items-center gap-1.5 px-2 py-1 bg-purple-500/10 border border-purple-500/20 rounded text-[10px]">
    <span className="w-1.5 h-1.5 bg-purple-400 rounded-full" />
    <span className="text-purple-300">Cowork · {ct.label}</span>
    <button
      onClick={() => handleUninstall(plugin.name, ct.id)}
      disabled={actionPlugin === plugin.name}
      className="text-purple-600 hover:text-red-400 ml-1"
      title={`Remove from Cowork · ${ct.label}`}
    >
      &times;
    </button>
  </div>
))}
```

- [ ] **Step 3: Update "Integrated with" section**

Replace single "Claude Cowork" indicator with space count:
```tsx
{spaces.length > 0 && (
  <span className="inline-flex items-center gap-1.5 text-xs text-green-400">
    <span className="w-1.5 h-1.5 bg-green-500 rounded-full" />
    Cowork ({spaces.length} space{spaces.length > 1 ? 's' : ''})
  </span>
)}
```

- [ ] **Step 4: Update inCowork check**

```typescript
const coworkTargets = parseCoworkTargets(targets);
const inCode = targets.includes('claude-code');
const inCowork = coworkTargets.length > 0;
```

- [ ] **Step 5: Visual verification via Playwright**

Navigate to Installed page, verify badges show "Cowork · Personal" and "Cowork · GoodieMate" separately with × buttons.

- [ ] **Step 6: Commit**

```
feat: per-space cowork badges on installed page
```

---

### Task 4: Backend — CoworkSpace detection (storage.rs)

**Files:**
- Modify: `src-tauri/src/storage.rs`

- [ ] **Step 1: Add CoworkSpace struct**

```rust
#[derive(Debug, Serialize, Clone)]
pub struct CoworkSpace {
    pub id: String,
    pub label: String,
    pub path: String,
    pub is_org: bool,
    pub has_cowork_plugins: bool,
}
```

- [ ] **Step 2: Update TargetInfo**

```rust
#[derive(Debug, Serialize, Clone)]
pub struct TargetInfo {
    pub claude_code: bool,
    pub claude_code_path: Option<String>,
    pub cowork_spaces: Vec<CoworkSpace>,
}
```

- [ ] **Step 3: Implement detect_cowork_spaces()**

Scans both `claude-code-sessions/` and `local-agent-mode-sessions/`. For each `{session}/{account}/`:
- Check `remote_cowork_plugins/manifest.json` → if has non-empty plugins, `is_org=true`, extract label from first plugin's `marketplaceName`
- Check `cowork_plugins/` exists → `has_cowork_plugins=true`
- Include if either condition is true
- ID = first 8 chars of SHA256(path)
- Deduplicate by account UUID

- [ ] **Step 4: Update detect_targets() to use detect_cowork_spaces()**

- [ ] **Step 5: Update InstallTarget enum**

```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum InstallTarget {
    Code(String),      // "claude-code"
    Cowork(String),    // space_id
}
```

Or simpler: keep as `String` and parse in install logic.

- [ ] **Step 6: Update integrate_cowork to accept space path**

Instead of `find_cowork_plugins_dir()`, resolve space_id → path from detected spaces. Create `cowork_plugins/` + `cowork_settings.json` if missing.

- [ ] **Step 7: Update list_installed to include space info in targets**

Return targets as `["claude-code", "cowork:{id}:{label}"]`.

- [ ] **Step 8: Commit**

```
feat: multi-space cowork detection and installation
```

---

### Task 5: Backend commands (commands.rs)

**Files:**
- Modify: `src-tauri/src/commands.rs`

- [ ] **Step 1: Update InstallRequest**

```rust
pub struct InstallRequest {
    pub plugin_name: String,
    pub version: Option<String>,
    pub target: String,  // "claude-code" or space_id
}
```

- [ ] **Step 2: Update install_plugin command**

Route `"claude-code"` → existing Code flow. Everything else → Cowork flow with space_id.

- [ ] **Step 3: Update uninstall_plugin command**

Same routing logic.

- [ ] **Step 4: Remove legacy get_cowork_path/set_cowork_path commands**

- [ ] **Step 5: Commit**

```
feat: update commands for multi-space cowork
```

---

### Task 6: Integration test — full flow via `pnpm tauri dev`

- [ ] **Step 1:** Start app, verify spaces detected in Settings
- [ ] **Step 2:** Go to Catalog, verify dropdown shows spaces
- [ ] **Step 3:** Install a plugin to org space
- [ ] **Step 4:** Verify badge on Installed page shows correct space label
- [ ] **Step 5:** Uninstall from specific space
- [ ] **Step 6:** Commit final polish

```
test: verify multi-space cowork end-to-end
```
