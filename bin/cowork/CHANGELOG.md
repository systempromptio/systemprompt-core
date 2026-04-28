# Changelog

## Unreleased

Local inference proxy + invariant-driven dashboard state. The configuration profile installed in Claude Desktop is now an install-once artifact: cowork runs an HTTP/1.1 proxy on `127.0.0.1:48217`, the profile pins it as the inference URL with a long-lived loopback shared secret, and JWT rotation moves entirely inside cowork (background tick refreshes the cached JWT 5 minutes before expiry). No more profile re-install dance when the JWT lapses.

- New `proxy` module (`src/proxy/{mod,server,forward,secret}.rs`) — hand-rolled listener bound to loopback, validates `Authorization: Bearer <loopback-secret>` constant-time, rejects non-loopback Host headers, swaps the bearer to the cached JWT before forwarding to the gateway. SSE responses stream chunked back to the client without buffering.
- New `auth` module (`src/auth.rs`) — single source of truth for credential acquisition. `obtain_live_token` returns the cached JWT or walks the provider chain (mTLS → session → PAT). `read_or_refresh(threshold)` mints a fresh JWT proactively when TTL drops below the threshold. `has_credential_source` is the configuration-only check used to suppress ghost-JWT state.
- New `proxy::secret::load_or_mint` — generates a 32-byte loopback secret on first run, persists at `~/Library/Application Support/systemprompt/cowork-loopback.key` mode 0600. Verified constant-time on every inbound request.
- `generate_claude_profile` now writes `inferenceGatewayBaseUrl = http://127.0.0.1:48217` and `inferenceGatewayApiKey = <loopback-secret>`. The JWT no longer leaves cowork. The expiry-warning banner in the dashboard is retired (the embedded credential has no TTL).
- `claude_desktop::probe` reads the user-scoped managed plist (`/Library/Managed Preferences/$USER/<domain>.plist`) in addition to the system-scope path, and queries values via `defaults read <domain> <key>` so `CFPreferences` resolves the proper search order. Fixes "profile missing" reports on user-scoped (`PayloadScope=User`) installs.
- Probe storms eliminated. Both `GatewayProbeRequested` and `ClaudeProbeRequested` now have in-flight guards (`AppState::mark_probing` / `mark_claude_probing`) so a slow probe can't be re-enqueued from `about_to_wait`. Previously a saturated system caused fork-bomb-grade subprocess spawn loops.
- Consistent dashboard state across sign-in / sign-out / cache expiry. `setup::logout` now also nukes the JWT cache file. `AppState::reload_into` and `probe_gateway_async` both check `auth::has_credential_source(&cfg)`; if no provider is configured, they purge orphan cache files and force `verified_identity = None`. Identity green-light is no longer possible without a real underlying credential.
- GUI sync auto-enables TOFU on first run only. `SyncRequested` passes `allow_tofu = config::pinned_pubkey().is_none()`; subsequent syncs reject pubkey rotation. The pre-warning is suppressed when a pubkey is already pinned, and the TOFU log line moves from `warn` to `info` with a clear "pinned <prefix>…" confirmation.
- `cache::clear`, `cache::read_with_threshold`, and `cache::ttl_remaining_secs` exposed on the cache API for the proxy refresh tick and consistency-cleanup paths.
- `gui::run` boots the proxy before the winit event loop and installs SIGINT/SIGTERM/SIGHUP handlers that immediately `_exit(128 + signum)` so the per-thread proxy listener never strands the parent process.

### Carry-over from prior unreleased work

Phase 0 of the cowork sync + auth pipeline hardening — shared prep, no behavioural change.

- `serde_jcs`, `thiserror`, `tracing`, and `tracing-subscriber` added as direct dependencies. JCS canonicalisation flips on in Phase 1 Track A; `thiserror` + `tracing` adoption follows in Track E.
- `SignedManifest` gains a `not_before: Option<String>` field with `#[serde(default)]`. Not yet wired into `canonical_payload` (Phase 1 Track D promotes it to a required, signed field with monotonic-version + skew enforcement).

## 0.4.0 - 2026-04-27

Native GUI (Windows + macOS). Double-clicking the binary now opens a branded settings window — gateway URL (editable), PAT input + cached-JWT state, marketplace counters (skills / agents / MCP), plugins-directory path, last-sync timestamp, Sync/Validate/Open-folder actions, and a live activity log. Tray stays native; the window is rendered via `wry`'s embedded WebView2/WKWebView using systemprompt.io's canonical brand (orange `#fb9b34` palette, real wordmark + favicon shipped from `storage/files/images`).

- `gui` subcommand explicitly launches the UI on Windows and macOS; Linux returns exit 64 with `gui not supported on this platform`.
- Default routing falls through to `gui` when the binary is launched without an attached terminal (Explorer / Finder double-click — detected via `GetConsoleProcessList==1` on Windows). Terminal invocations (`systemprompt-cowork`, `systemprompt-cowork run`) keep emitting the JWT envelope to stdout, so the credential-helper contract is unchanged.
- Tray icon left/right click and dedicated menu items (Sync now, Validate, Open settings…, Open config folder, Quit) feed the same event pipeline used by the window.
- `sync::run_once` now returns a structured `SyncSummary` / `SyncError`; `validate::run` returns a structured `ValidationReport`. CLI wrappers preserve the previous stdout text byte-for-byte.

## 0.3.3 - 2026-04-23

Release-only bump — v0.3.2 tag was consumed by GitHub's immutable-releases feature before a successful publish (macos-13 runner queue, then HTTP 422 after release delete). No code changes vs 0.3.2. `release-sign.yml` now drops the Intel-mac matrix entry and creates releases atomically.

## 0.3.2 - 2026-04-23

`install --apply` on macOS supports both MDM and non-MDM workflows. `profiles install` was deprecated by Apple (macOS 11+) for CLI-initiated installs, so the default `--apply` now does a direct-write to `/Library/Managed Preferences/` — works standalone with just a sudo prompt, no profile approval UI. `--apply-mobileconfig` is the new opt-in for the MDM/System-Settings path.

- `--apply` (default): writes raw prefs plist to `/Library/Managed Preferences/com.anthropic.claudefordesktop.plist` (+ per-user path), restarts `cfprefsd`. Single sudo call.
- `--apply-mobileconfig`: builds `.mobileconfig` and `open`s System Settings → Profiles for user approval. Use this for fleet deploys via Jamf/Intune/Mosyle (distribute the file; don't try to `profiles install` it locally).
- `uninstall` mirrors: tries `profiles remove`, then sudo-removes both managed-prefs plists and kicks `cfprefsd`.
- Rejects `http://` for non-loopback gateways up front (Cowork rejects it too).

## 0.3.1 - 2026-04-23

Superseded by 0.3.2 — did not ship; `profiles install` is deprecated on modern macOS.

## 0.3.0 - 2026-04-22

Breaking: signed-manifest wire format extended with `user`, `skills`, `agents`. AgentEntry replaces `card: object` with `system_prompt: string?`. 0.2.x clients cannot deserialise 0.3.x manifests.

- `whoami` subcommand prints authenticated identity from gateway.
- `sync` materialises `user.json`, `skills/<id>/{metadata.json, SKILL.md}`, `agents/<name>.json` under `.systemprompt-cowork/`.
- `status` surfaces identity + skill/agent counts from on-disk fragments.
- Manifest signing primitive moved to `systemprompt-security::manifest_signing` (no behaviour change; same SHA-256 derivation from JWT secret, same pubkey).
- Per-user manifest assembly relocated from `systemprompt-core` gateway into the template admin extension (boundary fix — per-user tables live in the extension).

## 0.2.0 - 2026-04-22

- Renamed crate to `systemprompt-cowork` (binary `systemprompt-cowork`, lib `systemprompt_cowork`).
- Expanded scope: credential helper + plugin/MCP sync agent for Cowork's `org-plugins/` mount.
- Added `ed25519-dalek` for signed-manifest verification.
- Manual release via `cargo-zigbuild` + `gh release create` on tag `cowork-v*`; Linux x86_64 and Windows x86_64 (mingw) binaries attached. macOS binaries require a Mac host.

## 0.1.0 (unreleased)

- Initial scaffold: JSON wire contract, cache, blocking HTTP client, platform keystore trait (macOS/Windows/Linux stubs), SSO assertion fetch, stdout JSON emission.
