# Changelog

## [Unreleased]

### Changed

- **Bridge frontend rewritten off Lit.js — pure vanilla Web Components.** All 22 `sp-*` components migrated from `LitElement` to a 110-line `SpElement` base (`web/js/components/sp-element.js`) with reactive setters, microtask-batched re-render, and `data-action` / `data-input` event delegation. `vendor/lit-all.min.js` deleted. `js/atoms.js` deleted (unused by components — bridge state subscription is the single source of truth).
- **State path unified.** Components subscribe to `bridge.subscribe('state.changed', ...)`, mutate reactive setters, and re-render. `hydrateAtoms` removed from `index.js`. The four parallel communication patterns (bridge sub, atoms, custom events, Lit reactive props) collapse to one.
- **Centralized event registry at `web/js/events/bridge-events.js`** owns all `document.addEventListener` calls (keydown, mkt:count, crumb:set, setup-open). `theme.js` module-scope listeners wrapped in `initTheme()`. Components subscribe via `onBridgeEvent(name, fn)` instead of registering their own document listeners.
- **Oversized components split.** `sp-setup-gateway` 211→117 lines (form rendering extracted to `utils/gateway.js::renderGatewayForm`), `sp-marketplace` 161→138 (listing fetch logic moved to `services/marketplace-service.js`), `sp-cloud-status` 161→127, `sp-rail` 160→119 (tab definitions extracted to `utils/rail-tabs.js`). Every JS file ≤150 lines, every CSS file ≤200.
- **Toast styles tokenised.** Hardcoded hex (`#2a1a1a`, `#d97757`, …) and px (`20px`, `12px`, …) in `main.css` replaced with new `--sp-toast-bg`, `--sp-toast-bg-error`, `--sp-toast-border`, `--sp-toast-fg`, `--sp-toast-shadow`, `--sp-radius-md`, `--sp-z-toast` tokens. Toast block extracted to `web/css/toast.css`.
- **Empty `.catch(() => {})` handlers replaced** with `.catch((e) => console.warn("snapshot failed", e))` across 19 component snapshot calls — visible failure logging instead of silent swallowing.
- **`assets.rs` registry updated** to drop `LIT_VENDOR`, `atoms`, `components/base` and register `components/sp-element`, `events/bridge-events`, `services/marketplace-service`, `utils/rail-tabs`, `utils/gateway`, `css/toast`. The `/assets/js/vendor/lit-all.js` route removed.
- **`i18n.js` leading comment block deleted** (4 lines). `log-virtual.js` switched from `frag.appendChild(li)` to `frag.append(li)`.

### Fixed

- **Clippy cleanup — zero warnings on `x86_64-pc-windows-gnu` and host targets under `-D warnings`.**
  - Removed dead `GuiApp.cancel: CancellationToken` field; cancellation is owned by `AppState.cancels` and per-handler tokens.
  - Collapsed 32 nested `if let` blocks into stable `let_chains` (autofix).
  - Switched four `needless_pass_by_value` sites to borrow: `ipc_runtime::handle_inbound(&str)`, `ipc_runtime::emit_sync_progress(Option<&str>)`, `SettingsWindow::create(&EventLoopProxy, Option<&str>)`.
  - Removed unjustified `#[allow]` attributes:
    - `clippy::unused_self` on `InstallError::exit_code` — replaced with `InstallError::EXIT_CODE` associated constant.
    - `clippy::vec_init_then_push` + `unused_mut` in `integration::registry` — refactored to cfg-gated const slices chained into the registry vec.
  - Audited remaining `#[allow]`s — kept only well-justified FFI (`unsafe_code`), logger-bootstrap fallback (`print_stderr` in `obs.rs`), CLI entry-point output, project-wide stylistic opts in `lib.rs`, `#[cfg(test)]` scopes, and cross-platform signature parity (`unnecessary_wraps` on Linux `org_plugins_system`).

### Added

- **Phase 3 frontend rewrite — full migration from HTTP polling + delegated dispatcher to Lit components + IPC channels.** Every legacy panel under `web/js/` is now an `sp-*` custom element extending `BridgeElement`, hydrated from `state.snapshot` and refreshed by the appropriate channel (`state.changed`, `host.changed`, `proxy.changed`, `proxy.stats`, `sync.progress`, `error`, `log`).
  - **23 new Lit components** in `bin/bridge/web/js/components/`:
    - **Stateless info panels (Phase 3a)**: `sp-proxy-status`, `sp-agent-presence`, `sp-agents-summary`, `sp-overall-badge`, `sp-sync-pill`, `sp-rail-profile`, `sp-footer`, `sp-crumb`.
    - **Interactive panels (Phase 3b)**: `sp-rail` (replaces `tabs.js` + `rail-indicator.js`, owns ⌘1–⌘4 and ⌘F shortcuts, persists `cowork.tab` to `localStorage`, broadcasts `crumb:set`), `sp-toast`, `sp-activity-log`, `sp-host-card`, `sp-hosts-list`, `sp-settings`.
    - **Marketplace + setup wizards (Phase 3c)**: `sp-marketplace`, `sp-marketplace-list`, `sp-marketplace-detail`, `sp-setup`, `sp-setup-gateway`, `sp-setup-agents`.
    - All components use light DOM (`createRenderRoot() { return this; }`) so existing CSS class selectors apply unchanged.
  - **Incremental host updates** — `sp-hosts-list` keeps a `Map<id, host>` and merges per-host deltas from the `host.changed` channel without re-fetching the full snapshot. `sp-agent-presence`, `sp-agents-summary`, and `sp-setup-agents` likewise merge per-host payloads in place.
  - **`bridge.js` shims** added: `openLogFolder`, `diagnosticsExportBundle`, `diagnosticsInfo`. `setup-open` cross-component event lets `sp-settings` reopen the setup wizard.
  - **`crumb:set` `CustomEvent`** decouples breadcrumb updates from `tabs.js`. `mkt:count` `CustomEvent` lets `sp-marketplace` push the marketplace total into `sp-rail` without a shared atom.

### Changed

- **HTTP control-plane server cut to single-instance focus only.** `gui::server::Server` reduced from a full HTTP router (state polling, log polling, marketplace listing, action dispatch, asset serving) to ~85 lines that handle exclusively `POST /api/focus_window` with constant-time CSRF check. The webview already loads via the `sp://app/` custom protocol (`window/native.rs::serve_custom_asset`), so no asset serving needs the HTTP path. Second-launch instances still ping the focus endpoint via `single_instance::ping_focus_running_instance`.
- **`assets::lookup_path` no longer takes a CSRF-token argument** — the `sp://` protocol bypasses it. `__TOKEN__` placeholders in CSS/JS modules are substituted with empty strings.
- **`last_action_message` removed** from `AppStateSnapshot`, `AppStateSnapshotBuilder`, and `StatePayload`. `AppState::set_message` deleted along with all 8 call sites in `handlers/sync.rs` and `handlers/auth.rs`. Toast surfacing now flows exclusively through the `error` IPC channel (`ipc_runtime::emit_error`), which is structured (`{scope, code, message}`) rather than a free-form snapshot field. `sp-toast` simplified to listen to `error` only; `sp-setup-gateway` stops parsing `last_action_message` for failure detection.
- **Marketplace install/uninstall buttons removed** from `sp-marketplace-detail`. Cloud sync (`sync::run_once`) is the install mechanism — signed manifests pulled from the gateway materialize plugins/skills/hooks/agents into `org_plugins_effective()`. Per-item buttons were redundant with sync. Dropped: `marketplace.install` / `marketplace.uninstall` IPC commands, the `MarketplaceItemArgs` struct, and the `bridge.marketplaceInstall` / `marketplaceUninstall` shims.
- **`tabs.js` decoupled from `crumb.js`**: `activateTab` now dispatches `document.dispatchEvent(new CustomEvent("crumb:set", { detail: { name } }))` instead of importing `setCrumb`. (Then both files were deleted entirely as `sp-rail` and `sp-crumb` took over.)
- **`web/index.html` reduced from ~485 to ~120 lines.** Wholesale markup blocks for the rail nav, marketplace tab (categories list + items + detail + actions footer), agents tab, host-card `<template>`, settings panel, activity drawer, setup wizard, and footer all replaced with single `<sp-*>` tags that own their own rendering.
- **`web/js/index.js` reduced from 64 to 43 lines.** No more `applySnapshot`, `subscribePolling`, `subscribeLog`, or `initEvents`/`initKeyboard`/`initTabs`/`initSetup`/`initMarketplace`/`initToast`. Final form: theme + i18n init, side-effect imports for every component, atom hydration from `state.changed`.
- **`gui/command.rs`**: added `openLogFolder` as an alias for `diagnostics.openLogDirectory`.

### Removed

- **Phase 4 — orphaned `http_local` module deleted.** `bin/bridge/src/http_local/` (mod, request, response, hop_by_hop) had zero remaining callers after Phase 3 cut the HTTP control plane. `pub mod http_local;` removed from `lib.rs`.
- **Phase 4 dead-code cleanup**: `Server::csrf_token` field inlined into the listener thread (the cloned token is sufficient); `#[allow(dead_code)]` markers removed from `gui/server.rs::Server` and `gui/menu.rs::MenuBarHandles`. `ErrorScope::Setup` variant dropped — no remaining call sites.
- **18 legacy frontend modules deleted**: `agents.js`, `api.js`, `crumb.js`, `dom.js`, `drawer.js`, `events/keyboard.js`, `events/registry.js`, `footer.js`, `hosts.js`, `hosts/card.js`, `marketplace.js`, `marketplace/detail.js`, `marketplace/glyph.js`, `marketplace/list.js`, `marketplace/state.js`, `overall-badge.js`, `profile.js`, `proxy.js`, `rail-indicator.js`, `setup.js`, `setup/agents.js`, `setup/gateway.js`, `setup/mode.js`, `state.js`, `sync-pill.js`, `tabs.js`. Subdirectories `events/`, `hosts/`, `marketplace/`, `setup/` removed.
- **2 backend Rust modules deleted**: `gui/connection.rs` (HTTP request parsing + CSRF validation + GET routing), `gui/action_dispatch.rs` (POST `/api/<action>` → `UiEvent`).
- **`gui/server_util.rs` trimmed** — `parse_query` and `now_unix` removed; only `mint_csrf_token` and `constant_time_eq` remain.
- **`server_json::snapshot_to_json`** removed (was used only by the deleted HTTP server).
- **`http_local`-based connection handling, `last_action_message` field**, `set_message` setter, builder method `with_last_action_message`, the `last_action_message` payload field, and the `csrf_token` query-parameter validation on asset URLs.

### Notes

- Single-instance focus continues to use a 127.0.0.1 TCP listener (loopback + CSRF). A FIXME in `gui/server.rs` tracks the future migration to Unix domain sockets / Windows named pipes.
- The `sp://app/` custom-protocol asset path remains the only way the webview loads HTML/CSS/JS; the `lit-all.min.js` vendor bundle is served as-is and special-cased to skip `__TOKEN__` substitution.
- `marketplace.list` IPC command and listing payload retained — it surfaces what's already been synced to disk by `sync::run_once`. There is no separate "catalog vs installed" model.
- Single-instance focus across platforms continues to work via the trimmed HTTP server.

### Earlier in this Unreleased window

- **Phase 3 follow-ups (3F.A / 3F.B / 3F.C)**:
  - **Cross-platform menu bar** — `gui::menu::attach_to_window(&MenuBarHandles, &Window)` on Windows extracts the HWND via `raw-window-handle` and calls muda `init_for_hwnd`, attached after settings-window creation. macOS continues to use app-wide `init_for_nsapp`. New direct dep on `raw-window-handle = "0.6"` for the Windows target. Native menu items now go through `i18n::t`.
  - **Cancellation plumbing + UI** — `AppState::install_cancel`/`clear_cancel`/`cancel_scope`/`cancel_all` keyed by a new `CancelScope` enum (`Sync`, `Login`, `GatewayProbe`). `sync`, `login`, `set-gateway`, `logout`, and `gateway_probe` handlers now wrap their `spawn_blocking` futures in `tokio::select!` against a child token; on cancel the result is dropped and a sensible failure outcome is emitted. `on_sync_finished` distinguishes `cancelled` from `failed` and emits a `cancelled` `sync.progress` phase. New `UiEvent::CancelInFlight { scope, reply_to }` + `gui/handlers/cancel.rs`. New IPC command `cancel` (scope `sync` | `login` | `gateway` | `all`) + `bridge.cancel(scope)` JS helper. New Cancel button (`#sync-cancel`) in the sync pill, hidden by default, shown when `sync_in_flight`, wired to `bridge.cancel("sync")`.
  - **Full i18n hydration** — `web/i18n/en-US/bridge.ftl` expanded from ~30 to ~140 keys grouped by surface (setup-, sync-, login-, gateway-, validate-, marketplace-, agents-, status-, settings-, activity-, footer-, nav-, menu-, host-, proxy-). `data-l10n-id` added to every visible static string in `web/index.html`; `web/js/i18n.js` extended to also hydrate `data-l10n-placeholder` and `data-l10n-aria` attributes. JS modules now route every `textContent =` literal through `t()` / `t_args`: `marketplace.js`, `marketplace/detail.js`, `marketplace/glyph.js`, `hosts.js`, `hosts/card.js`, `agents.js`, `proxy.js`, `setup/agents.js`, `setup/gateway.js`, `setup/mode.js`, `sync-pill.js`. Rust handler messages (`auth.rs`, `sync.rs`, `validate.rs`) now use `i18n::t` / `i18n::t_args` for log lines and bridge errors. Translators can drop a `web/i18n/<locale>/bridge.ftl` file and the entire UI switches over.

- In-progress concurrent work staged alongside Phase 2 observability: i18n module + web translation assets, native menu, system process helpers, ipc runtime split, lit-based web components (`atoms`, `bridge`, `theme`, `components/`), tokio-runtime handler refactor (`app.runtime` replacing `app.pool.spawn_task`), proxy/gateway/hosts/integration tweaks. Note: cross-target Windows/macOS build is currently broken in this snapshot pending the GuiApp `runtime` field landing.
- **Phase 2 observability**: support-grade diagnostics surface.
  - Daily log rotation via `tracing-appender` (max 7 files, non-blocking writer).
  - `bridge diagnostics` and `bridge --version` subcommands print version, git SHA, build timestamp, profile, log/config paths.
  - `vergen` build script embeds `VERGEN_GIT_SHA`, `VERGEN_GIT_COMMIT_DATE`, `VERGEN_BUILD_TIMESTAMP`, `VERGEN_GIT_BRANCH`.
  - Footer renders `vX.Y.Z (sha, date)` alongside the version pill.
  - Panic hook writes `bridge-crash-{utc-ts}.log` with payload, location, and backtrace; emits a `tracing::error!` event before abort.
  - Persistent activity log: JSONL writer subscribed to the activity emit hook, atomic byte counter, single rollover at 10 MB to `activity.jsonl.1`.
  - GUI Help & Support drawer panel: "Open log folder" and "Export diagnostic bundle" actions. Bundle zips bridge logs, activity JSONL (+ rolled), crash dumps, redacted config TOML, and `diagnostics.txt`; lands on Desktop and reveals in the OS file manager.
  - HTTP routes `/api/diagnostics/open_log_dir`, `/api/diagnostics/export_bundle`, `/api/focus_window`. IPC commands `diagnostics.openLogDirectory`, `diagnostics.exportBundle`, `diagnostics.info`.
  - INFO-level `gui_dispatch` span with `event_kind` and per-dispatch `request_id` (UUID v4); user-initiated handler entry points promoted from DEBUG → INFO.
  - Single-instance: `bridge.lock.json` sidecar persists `{pid, port, token}`; second launch pings `/api/focus_window` on the running instance (250 ms timeout) instead of silent-exiting.
  - `config::redaction::redacted_config()` walks the loaded TOML and replaces values under sensitive keys (`secret`, `credential`, `auth`, `pat`, `token`, `password`, `key`, `pubkey`, `session`) with `***REDACTED***`.
- New deps: `tracing-appender`, `backtrace`, `opener` (with `reveal`), `zip`, `uuid`, `serde_yaml`. Build dep: `vergen`.

### Changed

- `ActivityLog::set_emit_hook` → `add_emit_hook` (now multi-subscriber `Vec<EmitHook>`); existing IPC subscriber and the new persistent JSONL writer coexist.
- `obs::tracing_init` no longer threads file writes through a static `Mutex<File>`; uses a `NonBlocking` rolling appender behind a `OnceLock<WorkerGuard>`.

- Setup welcome page: drop redundant brand-mark icon from topbar (wordmark only); replace setup-card icon chip with the full systemprompt.io wordmark; hide topbar and footer entirely while in setup mode.
- Primary button (`.sp-btn-primary`) restyled with branded asymmetric corners (`--sp-corners-sm`) and a stable label — removed `transform: scale()` and `translateY` so text size and position no longer shift on hover. Added an icon slot: gray default icon swaps to a rotating spinner via `[aria-busy="true"]`.
- `Connect`, `Finish`, and `Open systemprompt bridge` buttons restructured with `<span class="sp-btn__icon">` + `<span class="sp-btn__label">`. `js/setup/gateway.js` now toggles only the label text on busy, preserving the icon nodes.
- Inputs aligned to `--sp-corners-sm` so form fields share the branded corner profile with buttons and cards.

## [0.7.0] - 2026-04-30

### Added

- `integration::codex_cli` — Codex CLI host integration (probe, config, install).
- `cli::credential_helper` — credential helper command surface.
- `gui::handlers::agents` — GUI handler module for agents.
- `web/css/agents.css` — agent presence cluster, setup-step machine, agents-list-empty, host-card kind chip.
- `web/js/agents.js` — `renderAgentPresence`, `renderAgentsSummary`, `renderAgentsRailCount`.
- `web/js/events/registry.js` — single document-level click registry dispatching `[data-action]`.
- `web/js/events/keyboard.js` — single keydown listener for ⌘1/2/3.
- `web/js/state.js`, `index.js`, `rail-indicator.js`, `crumb.js`, `sync-pill.js`, `profile.js`, `cloud.js`, `proxy.js`, `hosts.js`, `overall-badge.js`, `footer.js`, `marketplace/{detail,glyph,list,state}.js`, `drawer.js`.

### Changed

- **Breaking**: crate renamed from `bin/cowork` to `bin/bridge` (binary name `systemprompt-bridge`). Workspace `exclude` and tests updated.
- `gui::connection`, `gui::dispatch`, `gui::events`, `gui::hosts`, `gui::server_json`, `gui::state`, `gui::mod` — refactored alongside new agents handler and Codex CLI integration.
- GUI assets now serve as 22 modular CSS files and 24 JS ES modules from `/assets/css/*` and `/assets/js/*` instead of inlined into `index.html` via `__STYLE__`/`__SCRIPT__`. Each file is `include_str!`-bundled, served with `?t=<csrf>` token guard, and substituted with the per-request token.
- `web/style.css` (1572 lines, monolithic) split into 22 component files under `web/css/` (`tokens`, `fonts`, `reset`, `kbd`, `dot`, `badge`, `button`, `topbar`, `rail`, `shell`, `drawer`, `marketplace-{base,list,detail}`, `status`, `settings`, `setup`, `agents`, `log`, `footer`, `responsive`, `main`). All custom-property references use the `--sp-*` prefix.
- `web/js/snapshot.js` and `web/js/marketplace.js` (monolithic) replaced by 24 ES modules with named exports only. Single event registry, `data-action` delegation, `<template>` cloning, no `innerHTML` of multi-element strings, no early returns.

### Removed

- `web/style.css` — split into per-component files.
- `web/js/snapshot.js`, `web/js/main.js`, `web/js/activity.js` — carved into the new modules.
- `STYLE` constant, `style_concat()`, `__STYLE__` substitution, and `__SCRIPT__` substitution in `gui::connection`.

## [0.6.0] - 2026-04-30

### Added

- `activity::ActivityLog` ring buffer (1000 entries) capturing live proxy/sync events for the GUI activity feed.
- `proxy::usage` response-stream tap: `is_messages_path`, `wrap_response_stream`. Counts `/v1/messages` calls and sums input/output tokens from JSON and SSE bodies.
- `ProxyStats::messages_total`, `tokens_in_total`, `tokens_out_total` counters.
- `sync::apply::synthetic_plugin` writer: managed skills, agents, and `.mcp.json` are now materialised as a single synthetic Claude plugin (`systemprompt-managed`) under the org plugins root, instead of separate fragments under `.systemprompt-bridge/`.
- `paths::SYNTHETIC_PLUGIN_NAME` constant (`systemprompt-managed`).
- `ApplyError::ReservedPluginId` — manifests containing a plugin with the reserved synthetic-plugin id are rejected.
- GUI: split monolithic `web/app.js` into ES modules under `web/js/` (`main`, `api`, `dom`, `tabs`, `setup`, `marketplace`, `activity`, `snapshot`).
- GUI: `assets/fonts/` bundled fonts and an activity tab driven by the activity log.

### Changed

- **Breaking**: managed assets layout. Skills, agents, and managed MCP servers no longer live under `.systemprompt-bridge/{skills,agents,managed-mcp.json}`; they are written into the synthetic plugin directory `<org-plugins>/systemprompt-managed/{skills,agents,.mcp.json}`. `install` summary, `status`, and GUI counters now read from the new location.
- `install --uninstall` removes the synthetic plugin directory in addition to the metadata directory.
- Plugin sync no longer prunes the synthetic plugin as a stale entry.
- Malformed-plugin counter accepts both `.claude-plugin/plugin.json` and `claude-plugin/plugin.json`, and excludes the synthetic plugin.
- Proxy `forward` now takes `Arc<ProxyStats>` and wraps successful `/v1/messages` responses with the usage tap; counters update on the fly.
- Proxy request handler appends every forwarded request (and client-disconnect / forward errors) to the activity log.

### Removed

- **Breaking**: `paths::MANAGED_MCP_FRAGMENT`, `paths::SKILLS_DIR`, `paths::AGENTS_DIR` constants.
- **Breaking**: `sync::apply::{agent, mcp, skill}` modules. Replaced by `synthetic_plugin`.
- `gui::state::counters::read_index_count` (the old skills/agents `index.json` reader).
- Legacy `bin/cowork/web/app.js`; replaced by ES modules under `web/js/`.

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
- `sync` materialises `user.json`, `skills/<id>/{metadata.json, SKILL.md}`, `agents/<name>.json` under `.systemprompt-bridge/`.
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

- **Breaking**: crate renamed to `systemprompt-bridge` (binary `systemprompt-bridge`, lib `systemprompt_bridge`).
- Manual release via `cargo-zigbuild` + `gh release create` on tag `cowork-v*` (Linux x86_64 + Windows x86_64 binaries).

## [0.1.0] - unreleased

### Added

- Initial scaffold: JSON wire contract, cache, blocking HTTP client, platform keystore trait (macOS/Windows/Linux stubs), SSO assertion fetch, stdout JSON emission.
