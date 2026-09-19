# systemprompt-bridge

A desktop integration process connecting supported clients to a configured gateway. Credential helper, signed-manifest sync agent, and local inference proxy in a single binary.

Three roles:

1. **Credential helper.** Emits a JSON envelope matching Anthropic's `inferenceCredentialHelper` contract, `{ "token": "...", "ttl": 3600, "headers": {} }`, to stdout.
2. **Sync agent.** Pulls the user's signed plugin, skill, agent, and MCP allowlist manifest from the gateway into the `org-plugins/` mount.
3. **Local inference proxy.** Loopback HTTP/1.1 proxy on `127.0.0.1:48217`. Every client presents a local credential; the bridge swaps it for a fresh gateway JWT before forwarding upstream. The credential a client holds is scoped to the surface it was written to (see *Loopback credentials* below).

Diagnostics on stderr. `tracing` JSON via `SP_BRIDGE_LOG_FORMAT=json`. Exit 0 on success.

---

## Status

Independent semver, separate from the systemprompt-core workspace. The version is declared in `Cargo.toml`. See [`CHANGELOG.md`](CHANGELOG.md) for what each release changed.

Released artifacts: macOS (arm64, x86_64), Windows (x86_64), Linux (x86_64). The release workflow defines artifact checksums and Sigstore signing; inspect a release’s attachments for available verification material.

---

## Architecture

| Module | Purpose |
|---|---|
| [`context.rs`](src/context.rs) | `BridgeContext`: the one composition root — tokio runtime, proxy handle, install id, MCP registry, activity log, gateway HTTP client, per-process caches. Built once in `cli::run_with_args` (serving for `proxy`/`gui`, attaching for every other command) and injected; nothing below it reaches for process state |
| [`auth/`](src/auth/) | Provider chain (session → PAT), single credential contract |
| [`proxy/`](src/proxy/) | Loopback inference proxy, forwarding, single-flight token cache |
| [`gateway/`](src/gateway/) | Gateway client, manifest fetch and signature verification |
| [`sync/`](src/sync/) | Manifest apply, replay protection (monotonic version + skew) |
| [`wire/`](src/wire/) | Every payload the webview is written against (`StatePayload`, hosts, verdict codes, IPC envelope). Builds on every target and is exported to TypeScript under [`bindings/web/js/types/`](bindings/web/js/types/) by `just bridge-bindings` |
| [`gui/`](src/gui/) | Native settings window (winit + wry), Windows + macOS only |
| [`integration/`](src/integration/) | Host integration registry: Claude Desktop, Codex CLI, Hermes, OpenCode, the Claude Desktop facets (Cowork plugins + artifacts), and the sync-only Claude Code agent |
| [`install/`](src/install/) | Install and uninstall, pubkey pinning, MDM snippet emission |
| [`mcp_registry.rs`](src/mcp_registry.rs) | On-disk MCP snapshot, rehydrated at startup |
| [`schedule/`](src/schedule/) | OS scheduler templates for periodic sync |

The modules are layered bottom-up and `just lint-bridge-layers` refuses an upward `crate::` reference; the order lives in [`scripts/lint-bridge-layers.sh`](../../scripts/lint-bridge-layers.sh) (leaves → `config` → `gateway` → `mcp_registry` → `auth` → `validate`/`update` → `proxy_probe` → `proxy` → `context` → `host_sync` → `install` → `integration` → `sync` → `wire` → `gui` → `cli`). `just lint-bridge-globals` refuses a new process static outside its reasoned allowlist — service state belongs on `BridgeContext`. The two composition roots are `cli::run_with_args` (every command) and `gui::run(ctx)`, which receives the context the CLI built.

---

## Commands

| Command | Purpose |
|---|---|
| `run` _(default)_ | Acquire a bearer via the auth chain and emit the JWT envelope to stdout |
| `proxy` | Run the local inference proxy headlessly (Linux/server equivalent of the desktop GUI) |
| `gui` | Launch the native settings window (Windows + macOS) |
| `login [<sp-live-…>] [--stdin] [--code <exchange-code>] [--gateway <url>]` | Store a PAT securely and wire up config; `--stdin` reads the PAT from stdin so it never appears on the command line |
| `logout` | Remove the stored PAT and its config section |
| `clean` | Wipe local bridge state (config + PAT + token cache) |
| `status` | Show config paths and what is currently set up |
| `whoami` | Print authenticated identity from the gateway |
| `install [--apply] [--pubkey <base64>] …` | Bootstrap integration; pin manifest signing pubkey |
| `sync [--watch [--interval <secs>]] [--fresh] [--allow-tofu] [--force-replay] …` | Pull plugins + MCP allowlist into `org-plugins/`; `--fresh` bypasses the gateway's per-user catalogue memo; an unparseable `--interval` exits `64` |
| `oauth-client {status\|rotate}` | Manage the per-tenant OAuth client that mints plugin-scoped hook tokens |
| `validate` | End-to-end self-check (paths, gateway, creds, signatures) |
| `doctor` | Diagnose common failure modes (config, creds, gateway, loopback secret, pinned pubkey), one line per check |
| `credential-helper --host <id>` | Emit per-host bearer credentials on stdout (git/Anthropic credential-helper protocol) |
| `diagnostics` | Collect version, build provenance and diagnostic state |
| `uninstall [--purge]` | Reverse install; `--purge` also clears credentials |

Exit codes: `0` success, `2` emit error, `3` whoami error, `4` signature failure or unsigned refused under a pin, `5` no credential source succeeded, `8` pubkey not pinned, `10` transient credential failure worth retrying, `64` usage error.

A bare invocation with no subcommand is `run`; the GUI opens by default only when the process was started without a console of its own (a double-click on Windows, an app-bundle launch on macOS). A scheduler or a pipe invoking the binary with no arguments gets the credential helper, never a window.

---

## Security posture

- **Manifest trust.** `install --apply --pubkey <base64>` provisions administrator trust in the brand’s policy location. Operator trust uses a gateway-bound `[sync.trust]` record; a key that is not bound to the configured gateway is never adopted. `sync --allow-unsigned` is refused once a pin exists for the gateway.
- **Distinct JWT audience.** Bridge tokens use the audiences selected by the gateway credential exchange. Acceptance is determined by the receiving route’s audience and authorization policies.
- **Replay protection.** Manifests carry a signed `not_before` field; sync rejects `manifest_version` ≤ last applied or `not_before` outside ±5 min skew.
- **RFC 8785 (JCS) canonical JSON** for signature input. Field-order stability is contract, not coincidence.
- **Loopback proxy** validates a constant-time-compared credential on every inbound path except `/healthz` and `/__bridge/whoami`, and rejects non-loopback `Host` headers.
- **Auto-update policy is fail-closed.** The last-sync sentinel carries the organisation's `AutoUpdatePolicy`; when it is unreadable, corrupt, or written by another gateway the bridge withholds automatic staging and records a start-up fault instead of falling back to the staged default.
- **Uninstall removes only what the bridge wrote.** Plugins, policy values and marketplace entries are removed from the sidecar records the bridge keeps; foreign entries are reported and left in place.

### Loopback credentials

The loopback secret (`<config_dir>/<brand>/bridge-loopback.key`, mode 0600) is the root credential and is only ever written to bridge-owned 0600 files: the OpenCode `auth.json`, the Hermes `.env`, the Codex credential helper, and the Linux `env.sh`. Every surface another local account can read carries a token derived from it with a domain-separated HMAC-SHA256 (`src/proxy/scoped_token.rs`), so a leaked file grants only what that surface needs and a secret reset invalidates every derived token at once:

| Surface | Credential | Accepted on |
|---|---|---|
| Bridge-owned 0600 files | loopback secret | inference (`/v1/*`), `/mcp/*`, `/otel*` |
| Claude Desktop managed preferences / policy hive (`inferenceGatewayApiKey`, `managedMcpServers[].bearer`) | `host:claude-desktop` token | inference and `/mcp/*` only |
| `org-plugins/<plugin>/hooks/hooks.json` | `hook:<plugin_id>` token | `/api/public/hooks/*?plugin_id=<plugin_id>` only |

A hook route accepts only the matching plugin's hook token (the raw secret and host tokens are rejected there); `/otel*` accepts only the raw secret, so telemetry forwarded with the user's gateway JWT can be emitted only by a process that can read a bridge-owned file. A request whose credential does not fit the route is answered `401` with `x-systemprompt-bridge-reason: scope-mismatch`. Claude Desktop keeps a credential in its managed preferences because its third-party gateway contract offers no credential helper — `inferenceGatewayApiKey` is the only value it can present.

---

## Build

This crate is **not** part of the main workspace. Build standalone:

```bash
just build-bridge                              # host triple
just build-bridge aarch64-apple-darwin         # cross target
just build-bridge-all                          # mac arm+x86, windows x86_64, linux x86_64
```

The authoritative build commands are the `build-bridge*` recipes in the root `justfile`; CI mirrors them. Detailed build, release, versioning, and per-OS maintainer reference lives in the project's internal documentation.

---

## Developing the GUI

The webview is Windows/macOS only — `mod gui` is `#[cfg(any(target_os = "windows",
target_os = "macos"))]`, wry is not even a Linux dependency, and `gui` on Linux
prints *"gui not supported on this platform"*. Assets never travel over HTTP
either: they reach the webview through a wry custom protocol (`sp://app/…`).

`dev-web` serves the same web tree over plain HTTP so any browser — or
Playwright — can render it:

```bash
just bridge-preview            # http://127.0.0.1:4310
just bridge-preview 4399       # another port
```

- **Assets come off disk, not the embedded manifest.** Edit CSS, JS or
  `index.html` and refresh; there is no rebuild between edits. Only changing
  Rust needs one.
- **The shell is the real one.** The page is `web_assets::render_index()`, so it
  gets the same `__PLATFORM__` / `__LOGO_SVG__` / `__VERSION__` substitution and
  brand-theme injection the shipped app gets, and cannot quietly drift from it.
- **`?fixture=<name>` picks the state.** `web/dev/mock-ipc.js` stands in for the
  native IPC and answers `state.snapshot` from `web/dev/fixtures/*.json`. Write
  commands mutate the fixture in memory, so flows are clickable, not stills:
  pressing Repair really does generate, install, re-probe and settle the row.
  A switcher across the bottom of the page moves between fixtures.
- Add a state by dropping another JSON file in `web/dev/fixtures/`. It is one
  `state.snapshot` reply — the shape is `StatePayload` in
  `src/wire/payloads.rs` with `HostsPayload` (`src/wire/hosts.rs`)
  flattened in.

All of it is behind the `dev-preview` cargo feature, which is not in `default`,
and `build.rs` drops `web/dev/` from the staged tree — so neither the command
nor the mock exists in a shipped binary.

To review every state at once, the branded repo screenshots them and builds a
contact sheet:

```bash
just bridge-shots              # in systemprompt-internal
# → playwright/bridge-shots/index.html
```

That runs `playwright/tests/bridge-agents.spec.ts`, which also asserts the
structural things a screenshot cannot: no page errors, no horizontal overflow,
the drawer's Escape/focus contract, and that an agent with no profile is offered
under *Add agent* rather than listed with a status.

---

## Runtime environment

| Variable | Purpose |
|---|---|
| `SP_BRIDGE_CONFIG` | Path to `systemprompt-bridge.toml` (default: `<config_dir>/systemprompt/systemprompt-bridge.toml`) |
| `SP_BRIDGE_PAT` | Inline PAT (overrides file-based `[pat]`) |
| `SP_BRIDGE_POLICY_TRUST` | Managed manifest trust record as JSON (`{"gateway","key","source"}`); takes precedence over the OS policy store `manifestTrust` key |
| `SP_BRIDGE_ORG_PLUGINS_SYSTEM` | Override the system-scope org-plugins root (nonstandard installs, hermetic tests) |
| `SP_BRIDGE_EGRESS_ALLOWED_HOSTS` | Comma-separated Cowork egress allowlist for `install --apply` when `--egress-allowed-hosts` is absent; `loopback` expands to `127.0.0.1` |
| `SP_BRIDGE_LOG_FORMAT` | `json` for structured logs; default human-readable |
| `RUST_LOG` | `tracing` filter directive; default `info,systemprompt_bridge::proxy=debug`. A malformed value fails start-up |

The `SP_BRIDGE_` prefix is the brand's `env_prefix`; a white-label build reads the same suffixes under its own prefix. Every other environment read is an OS directory or identity probe (`HOME`, `XDG_CONFIG_HOME`/`XDG_CACHE_HOME`/`XDG_DATA_HOME`/`XDG_STATE_HOME`, `USER`, `SUDO_USER`, `HOSTNAME`/`COMPUTERNAME`, `LANG`, `PATH`, and the Windows `LOCALAPPDATA`/`APPDATA`/`USERPROFILE`/`ProgramData`/`ProgramFiles` folders) or a home override the managed host tool itself defines (`CODEX_HOME`, `CODEX_SYSTEM_CONFIG`, `HERMES_HOME`). Bridge behaviour is never switched by an undocumented variable; `just lint-env-vars` holds that line.

Cache lives at the OS cache dir under `systemprompt-bridge/cache.json` (mode 0600 on Unix).

---

## Configuration file

`systemprompt-bridge.toml` (location above, or `SP_BRIDGE_CONFIG`). Every key is optional.

```toml
gateway_url = "https://gateway.example.com"
deployment_organization_uuid = "…"   # Cowork organization this deployment targets

[pat]
file = "…"                            # PAT path override (default: <config_dir>/systemprompt-bridge.pat)

[session]
enabled = true                        # device-link browser sign-in

[sync.trust]
gateway = "https://gateway.example.com"   # the gateway this key is bound to
key = "…"                              # base64 manifest signing key; also settable via --pubkey / MDM
source = "operator"

[claude]
# host-app integration overrides

[cowork]
session_org_dir = "…"                 # absolute path to the Cowork session/organization directory

[opencode]
managed_dir = "…"                     # where the OpenCode managed config is written (default: the OS-managed OpenCode config dir)
```

`[cowork] session_org_dir` pins which Cowork session directory the bridge writes plugin enables and
the artifacts library into. Leave it unset when there is exactly one usable candidate — resolution
falls back to the deployment's personal-session UUID, then to a sole usable candidate, and otherwise
fails loudly listing what it found rather than guessing.

---

## Plugin mount paths

| OS | Path |
|---|---|
| macOS | `/Library/Application Support/Claude/org-plugins/` (system) · `~/Library/Application Support/Claude/org-plugins/` (user fallback) |
| Windows | `C:\ProgramData\Claude\org-plugins\` (system) · `%LOCALAPPDATA%\Claude\org-plugins\` (user fallback) |
| Linux | `${XDG_DATA_HOME:-$HOME/.local/share}/Claude/org-plugins/` |

---

## Release

Tag `bridge-vX.Y.Z` triggers `.github/workflows/bridge-release.yml`. Workspace CI is unaffected.

---

Part of [systemprompt.io](https://systemprompt.io), self-hosted AI governance infrastructure.


## Installation receipts and native sessions

An administrator provisions a device-scoped credential through
`POST /api/v1/consumer-devices/{enrolled_certificate_id}/credential`. Transfer that
secret to the intended device in a protected file, then run
`systemprompt-bridge device-enroll --token-file <file>`. Enrollment verifies the
credential against `/api/v1/consumer-devices/enrollment` before atomically storing
it with the authenticated consumer, enrolled device, and gateway identity.
The existing per-user bridge token and a supplied certificate fingerprint cannot
substitute for this credential. Local secret storage uses mode 0600 on Unix and a
verified private ACL on Windows. Same-device credential rotation preserves the
installation identity; another device or account uses a separate outbox and cannot
replay its predecessor's evidence.

Every implemented skill host participates after its native emitter succeeds:
Claude Code's active plugin cache, Codex's source and active cache, OpenCode and
Hermes skill directories, and Claude Desktop's enabled org-provisioned plugin
roots. A device-authorized download supplies a deterministic host installation
plan derived from the retained publication and full dependency bundle. Readback
checks native `SKILL.md`, every active supporting/dependency file, and retained
source material. A `.systemprompt-source` copy alone cannot pass: changing an
active script or entrypoint independently prevents acknowledgment. Unix file
permissions are checked against the applied 0644/0755 modes. Windows executable
mode verification is explicitly unavailable, so those receipts cannot support
verified session attribution.

Pending plan downloads are retained before network access, including their signed
publication, host and actual target directories. Sync retries materialization and
readback under a cross-process installation lock. A newer generation supersedes an
unresolved old plan; recovery cannot install the old plan over the newer version.
Receipt and session retries run during sync and the existing owned heartbeat task;
the heartbeat does not mutate native installations. The bounded outbox retains
unacknowledged evidence across process restarts, acknowledges identical retries,
and exposes conflicts and rejected credentials explicitly. Limits are 512 receipts,
512 pending plans and 16 MiB per identity-scoped outbox. Pending evidence is never
evicted to create room. `systemprompt-bridge feedback-status` reports acknowledged,
unacknowledged, fully verified and superseded states separately.

Session observation uses native client metadata on authenticated loopback requests:
Claude session UUIDs in `metadata.user_id`, Codex thread metadata (or compatible
native session headers), OpenCode's per-session header, and Hermes's native
`session_id` when present. It never binds the bridge-generated `x-session-id`.
Client versions/transports that omit native metadata retain unknown session
attribution; a user-agent alone cannot establish a session. Session bindings freeze
the observed generation instead of silently following later upgrades. These wire
parsers and filesystem contracts do not establish Windows/macOS native acceptance,
paid inference acceptance, or automated evaluator capability.

The upstream wire references inspected for these parsers are
[Codex response metadata](https://github.com/openai/codex/blob/main/codex-rs/core/src/responses_metadata.rs)
and [Hermes auxiliary request routing](https://github.com/NousResearch/hermes-agent/blob/main/agent/auxiliary_client.py).
