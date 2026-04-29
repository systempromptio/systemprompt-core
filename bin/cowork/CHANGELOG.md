# Changelog

## Unreleased

### Round 2 — pedantic-clean + tracing parity

Closes the entire `clippy::pedantic` lint set on the `bin/cowork` crate (174 → 0) and brings tracing instrumentation parity across the proxy, auth, GUI handler, and sync/apply paths. No behavioural changes.

- `bin/cowork/src/lib.rs` — crate-level `#![allow(...)]` for `missing_errors_doc`, `missing_panics_doc`, `module_name_repetitions`, `similar_names`. The first two contradict the project standard ("delete rustdoc"); the latter two are noise on the deliberate per-module `*Error` convention.
- ~63 `#[must_use]` attributes added across `auth/`, `config/`, `gateway/`, `http_local/`, `install/`, `integration/`, `proxy/`, `schedule/`, `sync/` for pure pub fns and builder methods.
- `Option::map(...).unwrap_or(...)` → `map_or(...)` / `map_or_else(...)`; `is_some_and(...)` adopted; `match Some/None` blocks rewritten as `let-else`; needless `continue` on for-loop tail arms collapsed into `{}` or merged variant patterns.
- `String::push_str(&format!(...))` → `let _ = writeln!(out, ...)` via `std::fmt::Write` (concentrated in `install/summary.rs`, ~14 sites).
- Tracing spans added on `proxy::forward::forward`, `auth::setup::{login, logout, set_gateway_url, clean}`, `gui::handlers::{auth, sync, validate, settings, gateway_probe}::on_*_requested`, and `sync::apply::{skill::write_skills, agent::write_agents, mcp::write_managed_mcp_fragment}` — bringing parity with `apply::plugin` instrumented in Wave 2C. Span sites emit `info!`/`warn!`/`debug!` at decision points (auth missing, upstream non-2xx, etc.).
- Misc cleanups: `match_same_arms` collapsed (`SyncError::exit_code`); `r#"…"#` → `r"…"` where unnecessary (`install/mdm`); dropped redundant `200 => "OK"` arm in `http_local/response::reason_phrase`; `Option<MessageVisitor<'a>>` lifetime elided.

### Host integration refactor — `gui/claude/` → `gui/hosts/`, host registry, proxy probe

Generalises the Claude-Desktop-specific GUI integration surface into a host-agnostic registry so future host apps (Cursor, Zed, Continue, etc.) can plug in without forking the dispatch tree.

- `gui/claude/` renamed to `gui/hosts/` — `dispatch.rs`, `events.rs`, `handlers.rs`, `mod.rs`, `serde.rs`, `state.rs`, `tick.rs` all moved. Public surface follows the new naming.
- New `integration/registry.rs` — `HostApp` trait + registry that enumerates installed host apps and routes setup/teardown calls.
- New `integration/host_app.rs` — common `HostApp` shape (id, display name, install/uninstall/probe).
- New `integration/proxy_probe.rs` — host-agnostic proxy reachability probe extracted from the per-host modules.
- New `integration/stub_host/` (gated behind `dev-stub-host` feature) — in-memory host implementation for tests / dev runs.
- New `integration/claude_desktop/win_reg_parser.rs` — isolated Windows registry parser for Claude Desktop's managed prefs.
- `Cargo.toml` adds `[features]` block with `dev-stub-host` opt-in feature.
- Web UI (`web/{index.html,app.js,style.css}`) updated to render the host-registry-driven view.
- `crates/tests/unit/cowork/integration/` adds coverage for the new registry and stub host.

### Settings window + marketplace listing + typed-ID rollout

Native settings window is now actually rendered (wry WebView2/WKWebView), the dashboard gains a marketplace listing pane backed by an on-disk scan of installed plugins/skills/hooks/MCP/agents, and the entire `bin/cowork` tree adopts the typed-identifier discipline used by the rest of the systemprompt-core codebase.

- New `gui/window/native.rs` — `SettingsWindow` wrapping `winit::Window` + `wry::WebView`. Decorationless 1100x760 default, 800x600 min. Loads from the local settings server. Adds `wry = "0.55"` (macOS + Windows targets only).
- New `gui/server_marketplace.rs` — `/api/marketplace` endpoint exposes a typed `MarketplaceListing { plugins, skills, hooks, mcp, agents }` scanned from `paths::org_plugins_effective()`. Each entry carries id, name, source, path, summary, README excerpt (capped at 32 KiB), and a free-form `extra` payload. Wired into the dashboard via `gui/server.rs`.
- New `gui/handlers/settings.rs` flow updates — settings actions now go through the same dispatch pipeline as auth/sync/validate, with structured outcomes back to the front-end.
- Web UI overhaul (`web/{index.html,style.css,app.js}`) — ~1k lines of dashboard shell, marketplace pane, settings forms, and brand styling.
- `bin/cowork/src/ids.rs` adds a vendored `cowork_define_id!` / `cowork_define_token!` macro pair (Display, FromStr, AsRef<str>, From<&str>/<String>, serde(transparent), redacted Debug for tokens, Zeroize on Drop). Local newtypes: `PatToken`, `BearerToken`, `LoopbackSecret`, `ProxySecret`, `ManifestSignature`, `PinnedPubKey`, `Sha256Digest` (validated 64-char lowercase hex), `PluginId`, `SkillId`, `SkillName`, `ManagedMcpServerName`, `ToolName`, `ToolPolicy`, `PrefsDomain`, `PrefsKey`, `PrefsValue`, `ModelId`, `KeystoreRef`, `CertFingerprint`, `QueryKey`, `QueryValue`.
- `systemprompt-identifiers = "0.4.2"` added as a dependency. Existing `String` fields for `SessionId`, `UserId`, `TenantId`, `AgentId`, `AgentName`, `ApiKeySecret`, `ValidatedUrl`, `ValidatedFilePath` adopt the upstream typed equivalents across `auth/`, `gateway/manifest.rs`, `config/`, `proxy/`, `install/`, `integration/claude_desktop/`, `sync/`, `gui/state.rs`, and `cli/`.
- JSON / TOML wire formats unchanged — every typed ID is `serde(transparent)`. `ManagedMcpServer.headers` deliberately remains `BTreeMap<String, String>` (case-preserving) so manifest signature canonical bytes are untouched.
- Token Debug now redacts: `BearerToken("xxxxxxxx…yyyy")` for tokens >16 chars, `***` otherwise. Drop zeroizes the inner buffer.
- `gui/window/mod.rs` — `open_url` renamed to `open_external_url` to disambiguate from `wry::WebView::load_url`.
- New `.github/workflows/scoop-cowork.yml` — Scoop bucket update workflow for Windows installs.
- `documentation/cowork/build-and-release.md` updated to reflect the typed-ID + webview status.

### Post-refactor lint sweep — zero warnings on Linux + Windows

Follow-up to phases E–H. Closes the remaining clippy warnings on `x86_64-unknown-linux-gnu` and `x86_64-pc-windows-gnu` and applies the punch-list items from the post-refactor review.

- `proxy::runtime` — `RUNTIME.get().expect("runtime just set")` annotated with `#[allow(clippy::expect_used)]`; the value is set on the line immediately above and cannot be `None`.
- `proxy::server::record_stats`, `gui::handlers::gateway_probe`, `integration::claude_desktop::gateway_probe`, `winproc::is_elevated`, `auth::keystore::linux::base64_decode` — every `as u64` / `as u32` / `as u8` cast that triggered `cast_possible_truncation` switched to `try_from(...).unwrap_or(MAX)`.
- `install::install` takes `&InstallOptions` (was owned `InstallOptions`); caller in `cli::install::cmd_install` updated to pass a reference. Closes `needless_pass_by_value`.
- `gui::server::handle_connection` takes `&Arc<AppState>`, `&Sender<UiEvent>`, `&str`, `&ActivityLog` (were owned). Closes four `needless_pass_by_value` warnings.
- `gui::handlers::auth::on_login_requested` takes `&Secret`; `on_set_gateway_requested` takes `&str`. `gui::dispatch` updated.
- `cli/` module gets `#![allow(clippy::print_stdout, clippy::print_stderr)]` — CLI is the user-output layer; routing intentional terminal output through `tracing` would defeat the purpose.
- `bin/cowork/Cargo.toml` flips `mod_module_files` from `warn` to `allow` — repo convention is `mod.rs` style throughout the cowork tree.

### Phase B — internals refactor (no behavioural change)

Five-stream decomposition of the `bin/cowork/src/` tree. Every source file is now ≤300 LOC, every fn ≤75 LOC, HTTP/1.1 parsing and response writing live in exactly one place, and GUI thread spawning is bounded and traceable.

- New `http_local/` module — single HTTP/1.1 implementation shared by the loopback proxy and the settings-UI server. `proxy/server.rs` drops 289 → 145 LOC, `proxy/forward.rs` 197 → 113 LOC, `gui/server.rs` 519 → 267 LOC. `ResponseBuilder` (fluent fixed-size response) and `write_chunked` (streaming with auto-framing) replace four ad-hoc copies of header/body code. Chunked-body decoding is now uniformly available on both HTTP entry points.
- New `cli/` module — `lib.rs` shrinks 375 → 77 LOC; every `dispatch_*` handler moves into a per-subcommand file (`run.rs`, `login.rs`, `logout.rs`, `status.rs`, `whoami.rs`, `install.rs`, `sync.rs`, `uninstall.rs`, `gui.rs`). `status` and `whoami` split into smaller helpers; the 18 `println!`s in the status path now route through `status_line` / `status_indent` so future tabular output is a one-line change.
- `gui/mod.rs` decomposed 471 → 203 LOC. The 206-LOC `dispatch()` match moves to `gui/dispatch.rs`; each arm is one line that delegates to a per-event-family handler module under `gui/handlers/` (`sync`, `auth`, `validate`, `settings`, `quit`, `gateway_probe`, `state`, `claude`). The 11 ad-hoc `std::thread::spawn` call sites are replaced by `gui::worker::WorkerPool`, which records `JoinHandle`s and joins on `Drop`. JSON serialization for the `/api/state` endpoint moves to `gui/server_json.rs`.
- `integration/claude_desktop.rs` (450 LOC) decomposed into `claude_desktop/{mod,managed_prefs,gateway_probe,process,profile}.rs`. The local `xml_escape` is gone — `crate::install::xml::escape` is now the single implementation (`mod xml` was promoted to `pub(crate)`).
- `config::load()` is idempotent. The previously-implicit policy-pubkey override is now a separate `Config::with_policy_overrides(self) -> Self`; the free `load()` composes them so all 13 call sites stay unchanged. The "policy-provided manifest pubkey overrides operator-set value" `tracing::warn!` is now `Once`-guarded — fires at most once per process instead of on every `config::load()` invocation.

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
