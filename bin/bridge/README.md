# systemprompt-bridge

Credential helper, signed-manifest sync agent, and local inference proxy for Anthropic's Claude on the systemprompt.io gateway.

One binary, three roles:

1. **Credential helper.** Emits a JSON envelope matching Anthropic's `inferenceCredentialHelper` contract — `{ "token": "...", "ttl": 3600, "headers": {} }` to stdout.
2. **Sync agent.** Pulls the user's signed plugin + skill + agent + MCP allowlist manifest from the gateway into Cowork's `org-plugins/` mount.
3. **Local inference proxy.** Loopback HTTP/1.1 proxy on `127.0.0.1:48217`. Claude Desktop's profile pins it as `inferenceGatewayBaseUrl` with a long-lived loopback secret; cowork swaps the bearer to a fresh JWT before forwarding upstream. JWT rotation never leaves the host.

Diagnostics on stderr. `tracing` JSON via `SP_BRIDGE_LOG_FORMAT=json`. Exit 0 on success.

---

## Status

Independent semver, separate from the systemprompt-core workspace. Latest release **0.5.0** — proxy correctness, chain-preserving GUI errors, and auth chain safety. See [`CHANGELOG.md`](CHANGELOG.md).

Released artifacts: macOS (arm64, x86_64), Windows (x86_64), Linux (x86_64). Sigstore-signed; SBOM attached to every release.

---

## Architecture

| Concern | File |
|---|---|
| Provider chain (mTLS → session → PAT) | [`src/auth/mod.rs`](src/auth/mod.rs) · [`src/auth/providers/`](src/auth/providers/) |
| Loopback inference proxy | [`src/proxy/server.rs`](src/proxy/server.rs) · [`src/proxy/forward.rs`](src/proxy/forward.rs) |
| Single-flight token cache | [`src/proxy/token_cache.rs`](src/proxy/token_cache.rs) |
| Manifest signature verification | [`src/gateway/manifest.rs`](src/gateway/manifest.rs) |
| Replay protection (monotonic version + skew) | [`src/sync/replay.rs`](src/sync/replay.rs) |
| Native GUI (winit + wry) | [`src/gui/`](src/gui/) |
| Host integration registry (Claude Desktop, future hosts) | [`src/integration/`](src/integration/) |

---

## Commands

| Command | Purpose |
|---|---|
| _(no args)_ | Emit credential-helper JSON envelope |
| `gui` | Launch the native settings window (Windows + macOS) |
| `login <sp-live-…> [--gateway <url>]` | Store a PAT securely |
| `logout` | Remove the stored PAT |
| `clean` | Wipe local cowork state (config + PAT + token cache) |
| `status` | Show config paths, cache state, last sync |
| `whoami` | Print authenticated identity from the gateway |
| `install [--apply] [--pubkey <base64>]` | Bootstrap integration; pin manifest signing pubkey |
| `sync [--watch] [--allow-tofu] [--force-replay]` | Pull plugins + MCP allowlist into `org-plugins/` |
| `validate` | End-to-end self-check (paths, gateway, creds, signatures) |
| `uninstall [--purge]` | Reverse install; `--purge` also clears credentials |

Exit codes: `0` success, `2` emit error, `3` whoami error, `5` no credential source succeeded, `8` pubkey not pinned, `10` transient failure on preferred provider.

---

## Security posture

- **Out-of-band manifest pubkey pinning.** `cowork install --apply --pubkey <base64>` writes the pin to `HKCU\SOFTWARE\Policies\Claude` (Windows) or the `com.anthropic.claudefordesktop` Managed Preferences plist (macOS) for MDM rollout. `cowork sync` is fail-closed without a pin unless `--allow-tofu`.
- **Distinct JWT audience.** Cowork tokens are minted with `audience: Cowork`. A stolen cowork JWT cannot call generic API endpoints.
- **Replay protection.** Manifests carry a signed `not_before` field; sync rejects `manifest_version` ≤ last applied or `not_before` outside ±5 min skew.
- **RFC 8785 (JCS) canonical JSON** for signature input. Field-order stability is contract, not coincidence.
- **Loopback proxy** validates a constant-time-compared shared secret on every inbound request and rejects non-loopback `Host` headers.
- **mTLS-preferred chain.** When mtls is configured, a transient gateway failure no longer silently downgrades to PAT — exits `10` distinct from the "no credential source" `5`.

---

## Build

This crate is **not** part of the main workspace. Build standalone:

```bash
just build-bridge                              # host triple
just build-bridge aarch64-apple-darwin         # cross target
just build-bridge-all                          # mac arm+x86, windows x86_64, linux x86_64
```

For the full build, release, versioning, and per-OS reference, see [`documentation/cowork/`](../../documentation/cowork/README.md).

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

## Plugin mount paths

| OS | Path |
|---|---|
| macOS | `/Library/Application Support/Claude/org-plugins/` (system) · `~/Library/Application Support/Claude/org-plugins/` (user fallback) |
| Windows | `C:\ProgramData\Claude\org-plugins\` (system) · `%LOCALAPPDATA%\Claude\org-plugins\` (user fallback) |
| Linux | `${XDG_DATA_HOME:-$HOME/.local/share}/Claude/org-plugins/` |

---

## Release

Tag `cowork-vX.Y.Z` triggers `.github/workflows/cowork-release.yml`. Workspace CI is unaffected.
