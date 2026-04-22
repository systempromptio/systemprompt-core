# Cowork integration

`systemprompt-cowork` (at `bin/cowork/`) is the on-device binary that makes Anthropic's Claude Cowork desktop app route through a systemprompt.io gateway. It plays two roles:

1. **Credential helper.** Matches Anthropic's `inferenceCredentialHelper` contract — called by Cowork at session start, emits `{token, ttl, headers}` to stdout.
2. **Sync agent.** Pulls a signed plugin + MCP allowlist manifest from the gateway and materialises it into Cowork's OS-specific `org-plugins/` directory.

## Server endpoints

| Path | Auth | Purpose |
|------|------|---------|
| `GET /v1/cowork/pubkey` | public | ed25519 public key used to sign manifests |
| `GET /v1/cowork/manifest` | bearer JWT | signed bundle: identity + plugins + skills + agents + managed MCP, scoped to the caller |
| `GET /v1/cowork/whoami` | bearer JWT | unsigned identity probe (`{ user, capabilities }`) for `status` / debug |

## What gets synced

Each `sync` run pulls the signed manifest and materialises every section:

| Section | Source | On-device target |
|---------|--------|------------------|
| `user`     | `UserService::find_by_id` (JWT `sub`)       | `<org-plugins>/.systemprompt-cowork/user.json` |
| `plugins`  | filesystem walk of `services/plugins/`      | `<org-plugins>/<plugin_id>/` (atomic stage→swap) |
| `skills`   | `SkillRepository::list_enabled` (DB)        | `<org-plugins>/.systemprompt-cowork/skills/<skill_id>/{metadata.json, SKILL.md}` |
| `agents`   | `AgentRepository::list_enabled` (DB)        | `<org-plugins>/.systemprompt-cowork/agents/<name>.json` |
| `managed_mcp_servers` | `RegistryManager::get_enabled_servers` | `<org-plugins>/.systemprompt-cowork/managed-mcp.json` |

The manifest carries `revocations: []` for future plugin/skill/agent removal pushes.

Source: `crates/entry/api/src/routes/gateway/cowork.rs`, mounted in `gateway_router` under `ApiPaths::GATEWAY_BASE` (`/v1`). The signing key is derived deterministically from `SecretsBootstrap::jwt_secret` via SHA-256 with a domain-separation prefix — no new secret management, all replicas agree.

## org-plugins mount paths

Per Anthropic's support docs, with the Linux convention we define locally:

| OS | System path | User fallback |
|----|-------------|---------------|
| macOS | `/Library/Application Support/Claude/org-plugins/` | `~/Library/Application Support/Claude/org-plugins/` |
| Windows | `C:\ProgramData\Claude\org-plugins\` | `%LOCALAPPDATA%\Claude\org-plugins\` |
| Linux | `/opt/Claude/org-plugins/` | `${XDG_DATA_HOME:-$HOME/.local/share}/Claude/org-plugins/` |

Linux is *not* documented by Anthropic — we use XDG per-user to avoid requiring sudo.

## Command surface

```
systemprompt-cowork                      # credential helper (default, Anthropic contract)
systemprompt-cowork login <sp-live-...>  # store PAT
systemprompt-cowork logout
systemprompt-cowork status
systemprompt-cowork whoami               # print authenticated identity from gateway
systemprompt-cowork install              # bootstrap dir + print MDM snippet
systemprompt-cowork sync                 # pull manifest, verify sig, apply to org-plugins
systemprompt-cowork sync --watch         # long-running
systemprompt-cowork validate             # self-check
systemprompt-cowork uninstall [--purge]
```

Full flags: `systemprompt-cowork help`.

## MDM profile keys

`install --print-mdm <os>` prints ready-to-paste snippets.

**macOS (`.mobileconfig`, domain `com.anthropic.claudefordesktop`):**

```xml
<dict>
  <key>inferenceProvider</key><string>gateway</string>
  <key>inferenceGatewayBaseUrl</key><string>https://gateway.example.com</string>
  <key>inferenceCredentialHelper</key><string>/opt/systemprompt/bin/systemprompt-cowork</string>
  <key>inferenceCredentialHelperTtlSec</key><integer>3600</integer>
  <key>inferenceGatewayAuthScheme</key><string>bearer</string>
</dict>
```

**Windows (`.reg`, `HKCU\SOFTWARE\Policies\Claude`):**

```
"inferenceProvider"="gateway"
"inferenceGatewayBaseUrl"="https://gateway.example.com"
"inferenceCredentialHelper"="C:\\Program Files\\systemprompt\\systemprompt-cowork.exe"
"inferenceCredentialHelperTtlSec"=dword:00000E10
"inferenceGatewayAuthScheme"="bearer"
```

**Linux** — no Anthropic-documented MDM format; use env vars or a systemd-user drop-in.

## Signed-manifest verification

On first `install` (or first `sync` if install was skipped), `systemprompt-cowork` fetches `/v1/cowork/pubkey` and pins it to `[sync].pinned_pubkey` in `systemprompt-cowork.toml`. Every subsequent `sync` verifies the manifest's ed25519 signature against the pinned key before touching disk. `--allow-unsigned` bypasses this for local dev; it logs a warning and is not intended for production.

## Scheduler templates

`install --emit-schedule-template <os>` writes a template to CWD:

- **macOS** — launchd plist, install to `~/Library/LaunchAgents/`. Runs `sync` on load and every 30 min.
- **Windows** — Task Scheduler XML. Logon trigger + 30-min repetition.
- **Linux** — combined systemd-user `.service` + `.timer` (must be split into two files before `systemctl --user daemon-reload`).

## On-device layout

After `install` + `sync`:

```
<org-plugins>/
  .systemprompt-cowork/
    version.json           # installed binary version, gateway URL, timestamp
    last-sync.json         # last manifest version + section counts
    user.json              # authenticated identity {id, name, email, display_name, roles}
    managed-mcp.json       # current managed MCP allowlist (mirror of manifest field)
    skills/
      index.json           # array of {id, name, description, file_path, tags, sha256}
      <skill_id>/
        metadata.json
        SKILL.md           # instructions body (markdown)
    agents/
      index.json           # array of {id, name, display_name, version, endpoint, ...}
      <agent_name>.json    # full agent definition incl. card_json, mcp_servers, skills
  plugin-id-1/
    claude-plugin/...
  plugin-id-2/
    claude-plugin/...
```

Plugin directories are atomically swapped (staged under `.staging/` then `rename`d) to avoid Cowork reading half-written state.

## Release

Tag `cowork-vX.Y.Z` triggers `.github/workflows/cowork-release.yml`. Matrix builds: `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-pc-windows-msvc`, `x86_64-unknown-linux-gnu`. Artifacts attach to a draft GitHub Release with SHA256SUMS.
