# Changelog

## 0.5.0 - 2026-04-29

Proxy correctness, chain-preserving GUI errors, and auth chain safety.

### Fixed

- Proxy no longer drops HTTP/1.1 trailers as silent empty data frames; non-data frames are filtered out before the upstream body is forwarded.
- Proxy `io::Error` boundary preserves the source chain instead of stringifying via `to_string()`.
- Tokio runtime initialiser returns `io::Error` on the (impossible) `OnceLock` race instead of `process::abort`.
- Proxy listener now binds IPv4 loopback (`127.0.0.1`) first and falls back to IPv6 loopback (`::1`); previously bound dual-stack `[::]:port` which exposed the proxy to non-loopback peers on hosts where `IPV6_V6ONLY` was off.
- Windows Claude Desktop profile generator emits `inferenceModels` as `REG_MULTI_SZ` (`hex(7):`-encoded UTF-16LE) instead of a comma-joined `REG_SZ`. Claude Desktop reads this value as a multi-string; the prior format was silently ignored.
- `auth::cache::write` and `proxy::secret::load_or_mint` now log a `tracing::warn!` when `chmod 0600` fails on the cached file, instead of swallowing the error.
- Single-flight token cache verified under concurrent near-expiry refresh; collapses to one upstream auth call.

### Changed (breaking)

- `auth::acquire_bearer` and `auth::mint_fresh` return `Result<HelperOutput, ChainError>` (previously `Option<HelperOutput>`).
- `UiEvent::{SyncFinished, LoginFinished, LogoutFinished, SetGatewayFinished}` and `HostUiEvent::{ProfileGenerateFinished, ProfileInstallFinished}` payloads now carry `Arc<GuiError>` instead of `GuiError`. Cloning a `UiEvent` no longer flattens non-`Clone` source chains.
- `GuiError::Msg` and the manual `Clone` impl on `GuiError` removed.
- `http_local::request::parse(&mut TcpStream)` removed — callers use `parse_from_read` (any `Read`) or `parse_buffered` (any `BufRead`).
- `cli run` and `cli whoami` exit with code `10` on a transient failure of the configured preferred provider (mtls), distinct from `5` ("no credential source succeeded"). Previously a network blip on mtls would silently downgrade to PAT.
- `gateway::GatewayClient` request timeout reduced from 30 s to 10 s.

### Added

- `auth::providers::AuthFailedSource::is_terminal()` distinguishing permanent failures (`PubkeyMissing`, `UnsafePath`, decode errors, `Serialize`) from transient network failures.
- `auth::evaluate_chain()` — testable chain evaluator that takes an explicit provider list and preferred-provider hint.
- `auth::ChainError` — `NoneSucceeded` and `PreferredTransient { provider, source }`.

### Internal

- All inline (`//`) and doc (`///`) comments removed from `bin/cowork/src/` per project standard.
- Pedantic clippy: 0 warnings.

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
