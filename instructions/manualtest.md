# Manual test: Windows Cowork ↔ WSL gateway

End-to-end checklist for verifying `systemprompt-cowork.exe` against a local
`systemprompt-template` gateway running in WSL.

Work top to bottom. Each step has a pass criterion. If a step fails, fix it
before moving on — later steps assume the earlier ones are green.

## Current artefacts (already produced)

| Thing | Location | Detail |
|---|---|---|
| Windows binary | `C:\Users\ejb50\Downloads\systemprompt-cowork.exe` | 2.3 MB, PE32+ x86-64 |
| SHA-256 | `f1f627e1770b378bb487bb8dd3e7a452cb9d94eff9da9754be38a30acdc9ca4c` | confirm with `certutil -hashfile <path> SHA256` |
| Gateway project | `/var/www/html/systemprompt-template` (in WSL) | runs on port 8080 |
| Backup of original `.wslconfig` | `C:\Users\ejb50\.wslconfig.bak` | restore if mirrored mode breaks anything |

---

## Step 1 — Fix WSL networking so Windows can reach WSL

The gateway boots on `127.0.0.1:8080`. From WSL's perspective that's
loopback; Windows sees a different network namespace and can't reach it.
The cleanest fix is WSL2 **mirrored networking**: Windows and WSL share
the same network stack, so `localhost:8080` works from both sides and the
WSL IP never drifts on reboot.

**Open `C:\Users\ejb50\.wslconfig` in Notepad** and replace its contents with:

```ini
[wsl2]
memory=24GB
swap=8GB
processors=32
networkingMode=mirrored

[experimental]
autoMemoryReclaim=gradual
sparseVhd=true
hostAddressLoopback=true
```

Save. Then from any **Windows PowerShell**:

```powershell
wsl --shutdown
```

Reopen WSL.

> **Tradeoff**: mirrored mode occasionally conflicts with corporate VPNs
> that do their own routing (Cisco AnyConnect, GlobalProtect). If things
> break after the restart, restore the backup (`cp .wslconfig.bak .wslconfig`
> in WSL, or copy it back via Explorer) and use Step 1-B below instead.

### Step 1-B (fallback only, skip if Step 1 worked)

If mirrored mode is a no-go, bind the gateway to `0.0.0.0` and punch a
firewall hole instead:

```bash
# In WSL, edit your profile YAML:
cd /var/www/html/systemprompt-template
grep -rn "host:" .systemprompt/profiles/ | head
# Change host: 127.0.0.1 → host: 0.0.0.0, save, restart
```

```powershell
# From an elevated PowerShell:
New-NetFirewallRule -DisplayName "WSL gateway 8080" -Direction Inbound `
    -LocalPort 8080 -Protocol TCP -Action Allow
```

Then substitute `http://<wsl-ip>:8080` everywhere `http://127.0.0.1:8080`
appears below. Find the WSL IP with `hostname -I` inside WSL.

---

## Step 2 — Start the gateway

In WSL:

```bash
cd /var/www/html/systemprompt-template
# however you normally start it:
cargo run --release
```

Wait for "All services started successfully".

**Pass criterion** — from a **Windows PowerShell**:

```powershell
curl http://127.0.0.1:8080/api/v1/health
curl http://127.0.0.1:8080/v1/cowork/pubkey
```

Both must return `200`. The second should return JSON containing a
`pubkey` field (base64 ed25519). If that works, the new routes we just
shipped are live.

> **New in this build (commit `9c685473`)**: the manifest now bundles
> `user`, `skills`, and `agents` alongside `plugins`/`managed_mcp_servers`,
> and there is a new `GET /v1/cowork/whoami` probe. Steps 8a and 8b below
> verify those.

---

## Step 3 — Smoke-test the Windows binary

**Windows PowerShell**:

```powershell
cd C:\Users\ejb50\Downloads
.\systemprompt-cowork.exe help
.\systemprompt-cowork.exe validate
```

Windows Defender SmartScreen may warn on first run (unsigned GNU build).
Click "More info" → "Run anyway".

**Pass criterion**: `help` prints the full command list including
`install`, `sync`, `validate`, `uninstall`. `validate` prints a report
with some `[warn]` lines — that's expected pre-login.

---

## Step 4 — Point the helper at the gateway and log in

```powershell
setx SP_COWORK_GATEWAY_URL "http://127.0.0.1:8080"
```

**Open a fresh PowerShell** — `setx` only applies to new shells.

```powershell
cd C:\Users\ejb50\Downloads
.\systemprompt-cowork.exe login sp-live-xxxxxxxx
```

Use a PAT minted by your gateway. The binary stores it under
`%APPDATA%\systemprompt\systemprompt-cowork.toml` and a 0600-equivalent
PAT file alongside.

**Pass criterion**: prints "Stored PAT…" and shows config + secret paths.

---

## Step 5 — Confirm the credential-helper contract

This is exactly what Cowork will run on every session start. If it
prints anything other than a single JSON line on stdout, Cowork rejects it.

```powershell
.\systemprompt-cowork.exe
```

**Pass criterion**: stdout is one line matching:

```json
{"token":"eyJhbGciOi...","ttl":3600,"headers":{"x-user-id":"...","x-session-id":"...","x-trace-id":"...","x-client-id":"...","x-tenant-id":"...","x-policy-version":"...","x-call-source":"..."}}
```

Diagnostics (`[systemprompt-cowork] ...`) go to stderr and are fine.

---

## Step 6 — Install: create org-plugins, pin pubkey, print MDM snippet

```powershell
.\systemprompt-cowork.exe install `
    --gateway http://127.0.0.1:8080 `
    --print-mdm windows
```

What this does:

1. Creates `C:\ProgramData\Claude\org-plugins\` (system scope) or
   `%LOCALAPPDATA%\Claude\org-plugins\` (user fallback if not elevated).
2. Writes `.systemprompt-cowork\version.json` inside it.
3. Fetches `/v1/cowork/pubkey` and pins it to the config under `[sync]`.
4. Persists `gateway_url` into the config.
5. Prints a `.reg` snippet.

**Pass criterion**: prints "Installed systemprompt-cowork integration",
"Pinned manifest signing pubkey from http://127.0.0.1:8080", and the
`.reg` snippet.

---

## Step 7 — Apply MDM registry keys

Copy the printed snippet into `cowork.reg` (or paste into `regedit`
under `HKEY_CURRENT_USER\SOFTWARE\Policies\Claude`). The values will be:

```
inferenceProvider               = "gateway"
inferenceGatewayBaseUrl         = "http://127.0.0.1:8080"
inferenceCredentialHelper       = "C:\Users\ejb50\Downloads\systemprompt-cowork.exe"
inferenceCredentialHelperTtlSec = 3600
inferenceGatewayAuthScheme      = "bearer"
```

Save as `cowork.reg` and double-click, or in PowerShell:

```powershell
reg import cowork.reg
```

Fully quit Cowork (right-click tray icon → Quit, then confirm no
`Claude.exe` in Task Manager) and relaunch.

**Pass criterion**: Cowork opens and does **not** show an Anthropic
sign-in screen. That means the helper returned a valid JWT on launch
and Cowork accepted it.

---

## Step 8 — First sync

```powershell
.\systemprompt-cowork.exe sync
```

What this does:

1. Fetches `/v1/cowork/manifest` with the cached JWT.
2. Verifies the ed25519 signature against the pinned pubkey.
3. For each plugin in the manifest: downloads each file from
   `/plugins/{id}/*`, verifies per-file SHA-256, stages under
   `org-plugins\.staging\`, then atomically `rename`s into place.
4. Writes `org-plugins\.systemprompt-cowork\managed-mcp.json` with the
   gateway's managed MCP allowlist.
5. Writes `org-plugins\.systemprompt-cowork\last-sync.json` with the
   manifest version and per-plugin digests.

**Pass criterion** (new format — identity + every section):

```
sync ok (you@example.com): N plugins (X new, Y updated, Z removed), S skills, A agents, M MCP — manifest 2026-...
```

If `sync` still prints the old `sync ok: N installed, ...` line you are
running a pre-`9c685473` binary — rebuild and redeploy.

Inspect:

```powershell
dir C:\ProgramData\Claude\org-plugins\
dir C:\ProgramData\Claude\org-plugins\.systemprompt-cowork\
type C:\ProgramData\Claude\org-plugins\.systemprompt-cowork\last-sync.json
type C:\ProgramData\Claude\org-plugins\.systemprompt-cowork\managed-mcp.json
```

`last-sync.json` now also contains `skill_count`, `agent_count`, and `user`.

Each subdirectory under `org-plugins\` (except `.systemprompt-cowork\`
and `.staging\` if present) should correspond to a plugin on the WSL
side under `<system-root>/services/plugins/`.

Re-run `validate`:

```powershell
.\systemprompt-cowork.exe validate
```

All lines should now be `[ok]` (possibly a single `[warn]` on cached
token if more than an hour has passed).

---

## Step 8a — Verify `whoami` and `status` (new in `9c685473`)

```powershell
.\systemprompt-cowork.exe whoami
```

**Pass criterion**: prints a JSON blob with at least:

```json
{
  "user": {
    "id": "...",
    "name": "...",
    "email": "you@example.com",
    "display_name": "...",
    "roles": ["..."]
  },
  "capabilities": ["plugins", "skills", "agents", "mcp", "user"]
}
```

If you get `401 invalid bearer token`, your cached JWT expired — run
`.\systemprompt-cowork.exe` once to refresh, then retry.

```powershell
.\systemprompt-cowork.exe status
```

**Pass criterion**: in addition to config/secret/org-plugins paths,
`status` now prints:

```
  identity: you@example.com
  skills: <S>
  agents: <A>
```

The numbers must match the `S` and `A` from the `sync ok` line.

---

## Step 8b — Verify the new on-device layout

```powershell
dir C:\ProgramData\Claude\org-plugins\.systemprompt-cowork\
type C:\ProgramData\Claude\org-plugins\.systemprompt-cowork\user.json
dir C:\ProgramData\Claude\org-plugins\.systemprompt-cowork\skills\
type C:\ProgramData\Claude\org-plugins\.systemprompt-cowork\skills\index.json
dir C:\ProgramData\Claude\org-plugins\.systemprompt-cowork\agents\
type C:\ProgramData\Claude\org-plugins\.systemprompt-cowork\agents\index.json
```

**Pass criteria**:

| File / dir | What to look for |
|---|---|
| `user.json` | object with `id`, `name`, `email` matching your gateway user |
| `skills\index.json` | JSON array, length matches the `S` from `sync ok` |
| `skills\<skill_id>\metadata.json` | one per enabled skill in the gateway DB |
| `skills\<skill_id>\SKILL.md` | the skill's instructions body (markdown) |
| `agents\index.json` | JSON array, length matches the `A` from `sync ok` |
| `agents\<agent_name>.json` | full agent config — `mcp_servers`, `skills`, `card`, etc. |

**Tamper test** — confirm signed-manifest verification still covers
the new sections:

```powershell
# Use Fiddler or a quick proxy to flip one byte in any skill or agent
# field of the response, then re-run sync. Expected:
#   manifest signature verification failed: ...
```

If a tampered response is *accepted*, the canonical payload is missing
a field — file a bug; do not ship.

---

## Step 8c — Cross-check against the gateway

From WSL, hit the manifest endpoint directly with your JWT (grab one
via the helper):

```bash
JWT=$(./systemprompt-cowork | python3 -c "import sys,json; print(json.load(sys.stdin)['token'])")
curl -s -H "Authorization: Bearer $JWT" http://127.0.0.1:8080/v1/cowork/manifest \
  | jq '{user: .user, plugins: (.plugins|length), skills: (.skills|length), agents: (.agents|length), mcp: (.managed_mcp_servers|length)}'
```

**Pass criterion**: counts match what `sync` printed and what's on disk.
If `skills`/`agents` are `0` but the gateway DB has rows, check that
the rows have `enabled = true` — the helpers call `list_enabled()`, not
`list_all()`.

---

## Step 9 — Exercise Cowork end-to-end

1. Open Cowork → "+ New chat".
2. Send `Hello`.
3. In the WSL terminal running the gateway, watch for:
   - `POST /v1/messages` — the actual inference call
   - Request carries `authorization: Bearer <jwt>`
   - Request carries the 7 canonical `x-*` headers
4. In Cowork, try invoking a plugin that was synced (slash command or
   sidebar entry). It should execute files that live under
   `C:\ProgramData\Claude\org-plugins\<plugin>\`.
5. If you have an MCP server enabled on the gateway, invoke a tool it
   provides from Cowork — the call should proxy through the gateway and
   show up in the audit table linked by `trace_id`.

---

## Step 10 — Schedule automatic sync (optional)

```powershell
cd C:\Users\ejb50\Downloads
.\systemprompt-cowork.exe install --emit-schedule-template windows
# writes: systemprompt-cowork-sync.xml in the current directory

schtasks /Create /TN "SystempromptCoworkSync" /XML systemprompt-cowork-sync.xml
```

Verify:

```powershell
schtasks /Query /TN "SystempromptCoworkSync" /V /FO LIST
```

The task triggers on logon and re-runs every 30 minutes.

---

## What "fully passed" looks like

| Check | Pass state |
|---|---|
| `wsl --shutdown` + restart | WSL boots cleanly after mirrored config |
| `curl http://127.0.0.1:8080/api/v1/health` (Windows) | `200` |
| `curl http://127.0.0.1:8080/v1/cowork/pubkey` (Windows) | returns base64 ed25519 key |
| `systemprompt-cowork.exe validate` | all `[ok]` after login+install+sync |
| `systemprompt-cowork.exe` no args | one JSON line on stdout (the helper contract) |
| `systemprompt-cowork.exe whoami` | prints `{user: {...}, capabilities: [...]}` from gateway |
| `systemprompt-cowork.exe status` | shows `identity: <email>`, `skills: N`, `agents: M` |
| `.systemprompt-cowork\last-sync.json` | exists, includes `skill_count`, `agent_count`, `user` |
| `.systemprompt-cowork\user.json` | matches authenticated user (id, name, email) |
| `.systemprompt-cowork\skills\index.json` | array length == gateway's enabled skills |
| `.systemprompt-cowork\skills\<id>\SKILL.md` | one per enabled skill, contains instructions |
| `.systemprompt-cowork\agents\index.json` | array length == gateway's enabled agents |
| `.systemprompt-cowork\agents\<name>.json` | one per enabled agent, full config |
| `.systemprompt-cowork\managed-mcp.json` | exists, contains gateway MCP servers |
| Cowork chat | no Anthropic login, WSL logs show `/v1/messages` hits |
| Plugin invocation in Cowork | files under `org-plugins\<plugin>\` execute |
| MCP tool from Cowork | routed through gateway, audit row written |

---

## Common failures

| Symptom | Cause + fix |
|---|---|
| Windows `curl localhost:8080` connection refused | Mirrored mode not active → `wsl --shutdown`, confirm `.wslconfig` has `networkingMode=mirrored`, restart WSL, restart gateway |
| Mirrored mode broke corporate VPN | Restore `.wslconfig.bak`, use Step 1-B (bind `0.0.0.0`) instead |
| SmartScreen blocks `systemprompt-cowork.exe` | Unsigned GNU build — "More info" → "Run anyway". For signed MSVC build, push a `cowork-v*` tag and pull from GH Actions release |
| `validate` shows `org-plugins path: ... (scope: user)` | `C:\ProgramData\Claude\org-plugins\` not writable — run `install` once from an elevated PowerShell, or accept user fallback for dev |
| Cowork still shows Anthropic login screen | Registry values not applied. Fully quit Cowork (check Task Manager), `reg query "HKCU\SOFTWARE\Policies\Claude"` to verify keys exist, relaunch |
| Cowork shows "credential helper failed" | Run `.\systemprompt-cowork.exe` manually. Any stderr output = fix it. Any extra stdout output = breaks the Anthropic contract |
| `sync` fails "manifest signature verification failed" | Gateway's `jwt_secret` changed since the pubkey was pinned. Open `%APPDATA%\systemprompt\systemprompt-cowork.toml`, delete the `[sync]` block, re-run `install --gateway http://127.0.0.1:8080` |
| `manifest fetch failed: 401` | PAT expired or wrong gateway. `.\systemprompt-cowork.exe logout`, then `login` again |
| `whoami` returns `401 invalid bearer token` | Cached JWT expired. Run `.\systemprompt-cowork.exe` (no args) to refresh, then re-run `whoami` |
| `whoami` returns `500 user ... not found` | JWT `sub` claim does not match any row in `users`. Confirm the PAT was minted for a real user, not a dev shim |
| `sync ok` reports `0 skills, 0 agents` | Gateway DB has no rows with `enabled = true`. Check `select count(*) from skills where enabled` / `from agents where enabled` |
| Chat works but no plugins visible in Cowork | `sync` hasn't run, or ran to the user-scope path while Cowork reads system path. Check where `validate` says `org-plugins path` resolved |
| `sync` accepted a tampered response (Step 8b) | Bug — canonical_payload missed a field. Filed against `bin/cowork/src/manifest.rs::canonical_payload` |

---

## Teardown

```powershell
cd C:\Users\ejb50\Downloads

# remove stored PAT + metadata
.\systemprompt-cowork.exe uninstall --purge

# revert Cowork's MDM config
reg delete "HKCU\SOFTWARE\Policies\Claude" /f

# remove the scheduled task (if created)
schtasks /Delete /TN "SystempromptCoworkSync" /F

# clear the env var
[Environment]::SetEnvironmentVariable("SP_COWORK_GATEWAY_URL", $null, "User")

# optionally revert mirrored networking
# copy C:\Users\ejb50\.wslconfig.bak over C:\Users\ejb50\.wslconfig
# then wsl --shutdown
```
