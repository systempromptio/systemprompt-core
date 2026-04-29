# Changelog

## [0.5.0] - 2026-04-29

### Added

- `auth::ChainError` enum (`NoneSucceeded`, `PreferredTransient { provider, source }`).
- `auth::providers::AuthFailedSource::is_terminal()` distinguishing permanent failures (`PubkeyMissing`, `UnsafePath`, decode errors, `Serialize`) from transient network failures.
- `auth::evaluate_chain()` — chain evaluator accepting an explicit provider list and preferred-provider hint.
- Exit code `10` on `cli run` and `cli whoami` for a transient failure on the configured preferred provider (distinct from `5` for "no credential source succeeded").

### Changed

- **Breaking**: `auth::acquire_bearer` and `auth::mint_fresh` return `Result<HelperOutput, ChainError>` (previously `Option<HelperOutput>`).
- **Breaking**: `UiEvent::{SyncFinished, LoginFinished, LogoutFinished, SetGatewayFinished}` and `HostUiEvent::{ProfileGenerateFinished, ProfileInstallFinished}` payloads now carry `Arc<GuiError>` instead of `GuiError`.
- **Breaking**: `gateway::GatewayClient` request timeout reduced from 30 s to 10 s.
- Preferred mtls provider with a transient gateway failure no longer silently falls through to PAT.

### Removed

- **Breaking**: `GuiError::Msg` variant and the manual `Clone` impl on `GuiError`.
- **Breaking**: `http_local::request::parse(&mut TcpStream)`. Use `parse_from_read` (any `Read`) or `parse_buffered` (any `BufRead`).
- All inline (`//`) and doc (`///`) comments under `bin/cowork/src/`.
- Unused `CODE_DOMAIN` constant in `integration::claude_desktop::shared`.

### Fixed

- Proxy dropped HTTP/1.1 trailers as silent empty data frames; non-data frames are now filtered out before the upstream body is forwarded.
- Proxy `io::Error` boundary preserves the source chain instead of stringifying via `to_string()`.
- Tokio runtime initialiser returns `io::Error` on the `OnceLock` race instead of `process::abort`.
- Proxy listener binds IPv4 loopback (`127.0.0.1`) first and falls back to IPv6 loopback (`::1`); previously bound dual-stack `[::]:port`, exposing the proxy to non-loopback peers on hosts where `IPV6_V6ONLY` was off.
- Windows Claude Desktop profile generator emits `inferenceModels` as `REG_MULTI_SZ` (`hex(7):`-encoded UTF-16LE) instead of a comma-joined `REG_SZ`.
- `auth::cache::write` and `proxy::secret::load_or_mint` log a `tracing::warn!` when `chmod 0600` fails on the cached file, instead of swallowing the error.

## [0.4.0] - 2026-04-27

### Added

- Native GUI on Windows and macOS; `gui` subcommand launches a branded settings window (gateway URL, PAT input, cached-JWT state, marketplace counters, plugins-directory path, last-sync timestamp, activity log).
- Default routing falls through to `gui` when launched without an attached terminal; terminal invocations continue to emit the JWT envelope to stdout.
- Tray menu items: Sync now, Validate, Open settings, Open config folder, Quit.
- `sync::run_once` returns a structured `SyncSummary` / `SyncError`; `validate::run` returns a structured `ValidationReport`.

### Changed

- Linux `gui` exits `64` with `gui not supported on this platform`.

## [0.3.3] - 2026-04-23

### Changed

- Release-only bump; no code changes vs 0.3.2.

## [0.3.2] - 2026-04-23

### Added

- `install --apply` on macOS direct-writes `/Library/Managed Preferences/com.anthropic.claudefordesktop.plist` and restarts `cfprefsd` (single sudo prompt, no MDM required).
- `install --apply-mobileconfig` builds a `.mobileconfig` and opens System Settings → Profiles for approval (MDM workflow).
- `uninstall` removes both managed-prefs plists and kicks `cfprefsd`.

### Removed

- `profiles install` / `profiles remove` invocations (deprecated by Apple on macOS 11+).

### Fixed

- Reject `http://` for non-loopback gateways at install time.

## [0.3.1] - 2026-04-23

### Notes

- Superseded by 0.3.2; did not ship.

## [0.3.0] - 2026-04-22

### Added

- `whoami` subcommand prints authenticated identity from the gateway.
- `sync` materialises `user.json`, `skills/<id>/{metadata.json, SKILL.md}`, `agents/<name>.json` under `.systemprompt-cowork/`.
- `status` surfaces identity and skill/agent counts from on-disk fragments.

### Changed

- **Breaking**: signed-manifest wire format extended with `user`, `skills`, `agents`. `AgentEntry.card: object` replaced with `system_prompt: string?`. 0.2.x clients cannot deserialise 0.3.x manifests.
- Manifest signing primitive moved to `systemprompt-security::manifest_signing` (signature semantics unchanged).
- Per-user manifest assembly relocated from the gateway into the template admin extension.

## [0.2.0] - 2026-04-22

### Added

- `ed25519-dalek` dependency for signed-manifest verification.
- Plugin / MCP sync against Cowork's `org-plugins/` mount.

### Changed

- **Breaking**: crate renamed to `systemprompt-cowork` (binary `systemprompt-cowork`, lib `systemprompt_cowork`).
- Manual release via `cargo-zigbuild` + `gh release create` on tag `cowork-v*` (Linux x86_64 + Windows x86_64 binaries).

## [0.1.0] - unreleased

### Added

- Initial scaffold: JSON wire contract, cache, blocking HTTP client, platform keystore trait (macOS/Windows/Linux stubs), SSO assertion fetch, stdout JSON emission.
