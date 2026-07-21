# systemprompt-bridge

The one process that keeps a desktop Claude install governed by your gateway without your credentials ever leaving the host. Credential helper, signed-manifest sync agent, and local inference proxy in a single binary.

Three roles:

1. **Credential helper.** Emits a JSON envelope matching Anthropic's `inferenceCredentialHelper` contract, `{ "token": "...", "ttl": 3600, "headers": {} }`, to stdout.
2. **Sync agent.** Pulls the user's signed plugin, skill, agent, and MCP allowlist manifest from the gateway into the `org-plugins/` mount.
3. **Local inference proxy.** Loopback HTTP/1.1 proxy on `127.0.0.1:48217`. The Claude Desktop profile pins it as `inferenceGatewayBaseUrl` with a long-lived loopback secret; the bridge swaps the bearer for a fresh JWT before forwarding upstream. JWT rotation never leaves the host.

Diagnostics on stderr. `tracing` JSON via `SP_BRIDGE_LOG_FORMAT=json`. Exit 0 on success.

---

## Status

Independent semver, separate from the systemprompt-core workspace. Latest release **0.16.0**: a Cowork artifacts emitter that materialises the manifest's `artifacts` section through two sinks, a staging directory consumed by the first-run `create_artifact` seed skill and an on-disk library store, with content-hashed idempotency and remove-on-empty cleanup. The GUI marketplace gains an Artifacts category listing entries in the local library store. See [`CHANGELOG.md`](CHANGELOG.md).

Released artifacts: macOS (arm64, x86_64), Windows (x86_64), Linux (x86_64). Sigstore-signed; SBOM attached to every release.

---

## Architecture

| Module | Purpose |
|---|---|
| [`auth/`](src/auth/) | Provider chain (mTLS → session → PAT), single credential contract |
| [`proxy/`](src/proxy/) | Loopback inference proxy, forwarding, single-flight token cache |
| [`gateway/`](src/gateway/) | Gateway client, manifest fetch and signature verification |
| [`sync/`](src/sync/) | Manifest apply, replay protection (monotonic version + skew) |
| [`gui/`](src/gui/) | Native settings window (winit + wry), Windows + macOS only |
| [`integration/`](src/integration/) | Host integration registry (Claude Desktop and future hosts) |
| [`install/`](src/install/) | Install and uninstall, pubkey pinning, MDM snippet emission |
| [`mcp_registry.rs`](src/mcp_registry.rs) | On-disk MCP snapshot, rehydrated at startup |
| [`schedule/`](src/schedule/) | OS scheduler templates for periodic sync |

---

## Commands

| Command | Purpose |
|---|---|
| `run` _(default)_ | Acquire a bearer via the auth chain and emit the JWT envelope to stdout |
| `proxy` | Run the local inference proxy headlessly (Linux/server equivalent of the desktop GUI) |
| `gui` | Launch the native settings window (Windows + macOS) |
| `login <sp-live-…> [--gateway <url>]` | Store a PAT securely and wire up config |
| `logout` | Remove the stored PAT and its config section |
| `clean` | Wipe local bridge state (config + PAT + token cache) |
| `status` | Show config paths and what is currently set up |
| `whoami` | Print authenticated identity from the gateway |
| `install [--apply] [--pubkey <base64>] …` | Bootstrap integration; pin manifest signing pubkey |
| `sync [--watch] [--allow-tofu] [--force-replay] …` | Pull plugins + MCP allowlist into `org-plugins/` |
| `oauth-client {status\|rotate}` | Manage the per-tenant OAuth client that mints plugin-scoped hook tokens |
| `validate` | End-to-end self-check (paths, gateway, creds, signatures) |
| `doctor` | Diagnose common failure modes (config, creds, gateway, loopback secret, pinned pubkey), one line per check |
| `credential-helper --host <id>` | Emit per-host bearer credentials on stdout (git/Anthropic credential-helper protocol) |
| `diagnostics` | Print the version and build-provenance banner |
| `uninstall [--purge]` | Reverse install; `--purge` also clears credentials |

Exit codes: `0` success, `2` emit error, `3` whoami error, `5` no credential source succeeded, `8` pubkey not pinned, `10` transient failure on preferred provider.

---

## Security posture

- **Out-of-band manifest pubkey pinning.** `bridge install --apply --pubkey <base64>` writes the pin to `HKCU\SOFTWARE\Policies\Claude` (Windows) or the `com.anthropic.claudefordesktop` Managed Preferences plist (macOS) for MDM rollout. `bridge sync` is fail-closed without a pin unless `--allow-tofu`.
- **Distinct JWT audience.** Bridge tokens are minted with `audience: Bridge`. A stolen bridge JWT cannot call generic API endpoints.
- **Replay protection.** Manifests carry a signed `not_before` field; sync rejects `manifest_version` ≤ last applied or `not_before` outside ±5 min skew.
- **RFC 8785 (JCS) canonical JSON** for signature input. Field-order stability is contract, not coincidence.
- **Loopback proxy** validates a constant-time-compared shared secret on every inbound request and rejects non-loopback `Host` headers.
- **mTLS-preferred chain.** When mTLS is configured, a transient gateway failure no longer silently downgrades to PAT; it exits `10`, distinct from the "no credential source" `5`.

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

## Runtime environment

| Variable | Purpose |
|---|---|
| `SP_BRIDGE_CONFIG` | Path to `systemprompt-bridge.toml` (default: `<config_dir>/systemprompt/systemprompt-bridge.toml`) |
| `SP_BRIDGE_GATEWAY_URL` | Gateway base URL (default `https://gateway.systemprompt.io`) |
| `SP_BRIDGE_PAT` | Inline PAT (overrides file-based `[pat]`) |
| `SP_BRIDGE_POLICY_PUBKEY` | Pinned manifest signing pubkey (overrides operator value) |
| `SP_BRIDGE_LOG_FORMAT` | `json` for structured logs; default human-readable |
| `SP_BRIDGE_DEVICE_CERT_SHA256` | Pin a specific device cert by SHA-256 fingerprint |

Cache lives at the OS cache dir under `systemprompt-bridge/cache.json` (mode 0600 on Unix).

---

## Configuration file

`systemprompt-bridge.toml` (location above, or `SP_BRIDGE_CONFIG`). Every key is optional.

```toml
gateway_url = "https://gateway.systemprompt.io"
deployment_organization_uuid = "…"   # Cowork organization this deployment targets

[pat]
file = "…"                            # PAT path override (default: <config_dir>/systemprompt-bridge.pat)

[session]
enabled = true                        # device-link browser sign-in

[mtls]
cert_keystore_ref = "…"               # OS keystore reference for the device cert

[sync]
pinned_pubkey = "…"                   # base64 manifest signing key; also settable via --pubkey / MDM

[claude]
# host-app integration overrides

[cowork]
session_org_dir = "…"                 # absolute path to the Cowork session/organization directory
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
