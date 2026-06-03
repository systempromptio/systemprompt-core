# Changelog

## [0.10.7] - 2026-06-02

### Added

- The Status tab shows each host's compatible models and warns when no usable model is available — for example when the host's matching provider has no API key configured — instead of reporting the host as healthy. The compatible-model set and provider health come from the gateway's `/v1/bridge/profile` response.

### Changed

- A managed host is offered only the models whose wire protocol it speaks: Claude Desktop receives Anthropic models, Codex CLI receives OpenAI models. Previously every host received the same flat model list, which could hand a host models its client cannot use.

### Fixed

- Installing the Claude Desktop managed-policy profile now elevates on demand instead of failing on a standard account. The policy lives under `SOFTWARE\Policies\Claude`, an ACL-protected subtree that a non-elevated token cannot create in either hive, so the in-process write introduced in 0.10.6 returned `ERROR_ACCESS_DENIED` (status 5) for every unprivileged install — including the `HKEY_CURRENT_USER` fallback. When the bridge is not already elevated it now relaunches its own executable under a Windows UAC consent prompt to write the policy machine-wide (`HKEY_LOCAL_MACHINE`); the activity log explains the prompt before it appears, a declined prompt surfaces an "administrator approval was declined" message instead of a raw status code, and an access-denied write now reports which hive and subkey require administrator rights.

## [0.10.6] - 2026-06-02

### Changed

- Installing the Claude Desktop managed-policy profile no longer shells out to `reg import`. The install path parses the staged `.reg` profile and writes each policy value directly through the Windows registry API (`RegCreateKeyExW`/`RegSetValueExW`), choosing `HKEY_LOCAL_MACHINE` when elevated and `HKEY_CURRENT_USER` otherwise, which removes the dependency on an external binary and surfaces a structured error on failure. The `.reg` render and parse halves move to a platform-independent module so the round-trip is unit-tested on every target.

## [0.10.5] - 2026-06-02

### Fixed

- The Status tab's **MCP servers** section badge no longer stays "UNKNOWN" when a server is authenticated. `sp-mcp-auth-status.js` seeded the section rollup with `"unknown"`, which `rollUp` ranks above `ok`, so an authenticated server could never lift the badge to green; the section state is now derived from the servers' worst state.
- The MCP auth probe now captures the backend's `Mcp-Session-Id` from `initialize` and surfaces it on the server card (`McpServerAuth.session_id`), confirming a session was established and aiding session-lifecycle debugging.

## [0.10.4] - 2026-06-02

### Fixed

- The setup wizard's **Finish** button is no longer gated on `anyInstalled`. Host install-state is probe-driven and can lag or misreport (the agent card shows "Installed ✓" while the parent's `anyInstalled` flag is still `false`), which trapped the user on step 2 with a permanently disabled Finish and no way into the app. Finish is now always enabled — installing agents is optional.

## [0.10.3] - 2026-06-02

### Fixed

- The proxy's upstream client (`proxy::server::build_upstream_client`) now installs the same `Ipv4FirstResolver` the gateway client already uses. The client forwards Cowork's MCP and `/v1/messages` inference to the gateway, so when a user configured the gateway as `http://localhost:…`, every proxied call resolved IPv6 `::1` first and stalled the full connect timeout (~15-21s) before falling back to IPv4 — the WSL2 localhost forwarder black-holes IPv6 SYNs. Sync/probe/profile-fetch were already IPv4-first via the gateway client; the proxy path was the remaining gap, so a user-entered `localhost` URL no longer freezes proxied traffic. `gateway::Ipv4FirstResolver` is now `pub(crate)`.

## [0.10.2] - 2026-06-02

### Added

- **MCP authentication status in the GUI.** The Status tab gains an "MCP servers" group that runs a live `initialize` → `tools/list` round-trip per registered server through the loopback proxy (`proxy::mcp_probe`) and classifies the result — Authenticated, `bad loopback secret` (403), gateway unauthorized (401), proxy unreachable, etc. — so failures that previously required reading Cowork's `main.log` are visible in-app. Authenticated servers list the tools they expose as chips. The panel re-probes automatically after each sync and via a manual "Recheck" button. The MCP server's tools are also listed in the Marketplace detail view.

### Changed

- The synthetic org plugin now ships `installationPreference: "required"` (was `"auto_install"`). `auto_install` installs once at sign-in but treats a later removal as a sticky user-uninstall, so a cleared install record never returned and the plugin sat behind a manual "Add" with its skills disabled. `"required"` is the org-plugin equivalent of the `managedMcpServers` policy — it force-installs at every sign-in and reinstalls if removed — so skills/agents/hooks land automatically, matching the managed MCP connector. Takes effect on each user's next Cowork sign-in.

### Fixed

- `gateway::Ipv4FirstResolver` no longer uses a trivial `as` cast to box its address iterator (a `Box<…> as Box<dyn …>` unsizing the newer toolchain's `trivial_casts` lint flags); the coercion is now expressed via a typed binding.

## [0.10.1] - 2026-06-02

### Fixed

- Windows: `managedMcpServers` (which embeds the rotating loopback secret) is now written only to the per-user `HKCU\…\Policies\Claude` hive — the same hive the GUI already owns for `inferenceGatewayApiKey` — instead of the machine-wide `HKLM` MDM policy. Pinning the secret in HKLM was a latent split-brain bug: once the secret rotated under a non-elevated bridge run (which cannot rewrite or delete HKLM), the stale HKLM value outranked HKCU, so Cowork connected with the old secret and failed every MCP call with `forbidden: bad loopback secret`. `managedMcpServers` is dropped from `windows_policy_values` (the HKLM policy now carries only stable, secret-free keys), and the writer best-effort purges any stale HKLM copy left by older builds.
- Windows: `bridge --uninstall` now actually clears the managed registry policy — `remove_managed_profile` was a no-op on Windows, so a stale, secret-bearing `managedMcpServers` survived "clean" reinstalls. It now removes the bridge-owned `HKCU\…\Policies\Claude` key and best-effort deletes `HKLM\…\managedMcpServers`, so a reinstall starts from a clean registry.

## [0.10.0] - 2026-06-01

### Changed

- The synthetic organization plugin's `plugin.json` and the malformed-plugin / well-formedness probes use the shared `PluginManifest` model and manifest-path constants from `systemprompt-models::bridge::plugin_bundle` instead of bridge-local copies. The emitted wire shape is unchanged.

### Added

- The bridge detects whether each host's desktop app is installed, launches or focuses it on request, and offers an official download link when the app is absent. `HostAppSnapshot` gains `app_installed`, the `HostApp` trait gains `download_url()`, and the setup UI surfaces install, launch, and download actions.

## [0.9.6] - 2026-05-30

### Changed

- Plugin hook calls route through the bridge loopback proxy instead of the gateway directly. The per-plugin `hooks.json` now points Cowork at the proxy's loopback URL with the static loopback secret as `Authorization`; the proxy verifies and strips that header, mints the plugin's `aud:hook` gateway token (resolved from the `plugin_id` query parameter), and injects it before forwarding to the public hook endpoints. This replaces the per-plugin `.env.plugin` file and the `$SYSTEMPROMPT_PLUGIN_TOKEN` env-var substitution, which Cowork's agent VM did not reliably propagate into the hook subprocess; `allowedEnvVars` is now empty. A hook-route `401` rotates the per-plugin hook token rather than invalidating the shared bridge token cache.
- Hook-scoped credentials issued by `admin keys issue-plugin-token` no longer carry the minting admin's roles. A hook token (`aud:hook`) authorizes on scope and `plugin_id` only, so the roles were inert.
- The GUI marketplace lists managed MCP servers from the in-memory MCP registry — the same source that feeds the `managedMcpServers` policy — rather than the removed synthetic-plugin `.mcp.json`.

## [0.9.5] - 2026-05-29

### Changed

- Managed MCP servers are registered with Cowork through the bridge's loopback proxy rather than the upstream gateway. Each entry points Cowork at the proxy's loopback URL with a static loopback-secret `Authorization` header instead of carrying `oauth: true`; the proxy strips that header and injects the rotating, auto-refreshed gateway JWT before forwarding to the registered upstream. This sidesteps Cowork's OAuth flow entirely — it hard-rejects the gateway's non-HTTPS authorize URL on Connect — while every request still carries a live token. Applies to both the MDM managed-prefs writer (`install::mdm`) and the synthetic-plugin writer (`sync::apply::synthetic_plugin`); when the loopback secret is unavailable the managed server list is emitted empty rather than half-configured.

## [0.9.4] - 2026-05-28

### Breaking

- `bridge::manifest::AgentEntry.mcp_servers` and `AgentEntry.skills` are now `PluginComponentRef { source, include, exclude }` instead of `Vec<String>`. The manifest envelope tracks the unified `PluginComponentRef` shape now applied across every entity-id reference list in `systemprompt-models`. Bridge / Cowork consumers that read these fields must traverse `.include` instead of treating the value as a flat list; serialised manifests authored against 0.9.3 are no longer accepted.

### Changed

- Bridge no longer emits the `deploymentOrganizationUuid` policy key into the Claude Desktop managed-prefs plist (macOS) or `HKCU\…\Policies\Claude` registry hive (Windows). Cowork's 3P custom-gateway contract is inference-only (`POST /v1/messages` + optional `GET /v1/models`, per docs/cowork/3p/gateway and gateway-sso); a custom gateway has no spec surface to assert the `integrations:manage` permission that this key flips Cowork into checking. Emitting it locked the Install button under the "Contact an organization owner to install connectors" tooltip without recourse. Cowork now resolves through `manageFromPersonal = true` and the Install button is live — MCP installation and use over the bridge proxy are unchanged.
- `pick_target` no longer takes a `policy_uuid` argument and `resolve_target` no longer reads the now-absent `deploymentOrganizationUuid` policy key; Cowork plugin sync resolves the personal-session org dir directly, falling back to newest-mtime when the personal dir is missing.
- Bridge keeps its own working state (`.staging/`, sync sentinel, version sentinel, user fragment) under a platform-specific user-writable directory (`%LOCALAPPDATA%\systemprompt-bridge` on Windows, `~/Library/Application Support/systemprompt-bridge` on macOS, `$XDG_STATE_HOME/systemprompt-bridge` on Linux) instead of nesting them under the published `org_plugins` tree. On Windows that tree lives under `Program Files` and is admin-write-only, so writing scratch state inside it raised `Sync failed: io error in create staging: Access is denied` for non-elevated bridge runs. `paths::metadata_dir(_)` / `paths::staging_dir(_)` and the `METADATA_DIR` / `STAGING_DIR` constants are gone; callers use the new `paths::bridge_working_dir()` / `bridge_staging_dir()` / `bridge_metadata_dir()`.

### Added

- `bridge doctor` adds a `hook token mint` check that exchanges the cached OAuth client credentials for a hook token against the gateway's token endpoint with `plugin_id=__doctor__`. Failures surface the gateway's `error_description` verbatim on a single line instead of waiting for the next `sync` PARTIAL output.
- `bridge doctor` adds a `personal-session sentinel` check that scans Cowork's sessions root for an org dir matching `PERSONAL_SESSION_UUID` (`00000000-0000-4000-8000-000000000001`). If Cowork sessions exist but none matches, the constant has drifted from Cowork's source of truth and `pick_target` will silently fall through to its mtime fallback — the check fails loud so the operator updates the bridge before sync misroutes plugins into the wrong session.

## [0.9.3] - 2026-05-28

### Changed

- `marketplace.json`, `known_marketplaces.json`, and `installed_plugins.json` are written in the shape the current Cowork (Claude 1.5354) reader expects: `marketplace.json` gains `$schema`, `description`, `metadata { description, version, pluginRoot }`, and per-plugin `author`/`category`, with `plugins[].source` flattened to a plain string path; `known_marketplaces.json` is a top-level object keyed by marketplace name with `source`, `installLocation`, and `lastUpdated` per entry; `installed_plugins.json` is `{ "version": 2, "plugins": { "<plugin>@<marketplace>": [{ "scope", "installPath", "version", "installedAt", "lastUpdated" }] } }`. Foreign sibling entries continue to be preserved verbatim.
- Cache and marketplace path joins sanitise version strings before writing to the filesystem; RFC3339-shaped versions containing `:` no longer trip Windows ERROR_INVALID_NAME during `bridge sync`.
- `sync` propagates per-host emit failures into `SyncSummary::host_failures` and the one-line summary now reads `sync PARTIAL (…) — N host(s) failed: …`, so a silently half-published marketplace surfaces in the GUI Activity panel instead of being reported as `sync ok`.
- 403 "bad loopback secret" rejections log the resolved secret path, and `tracing` lines on empty / missing / freshly minted secret files include the file path, giving operators a single line to follow when Claude Desktop has cached a stale loopback secret.
- `GatewayError::HookTokenRejected { status, body }` replaces the bare `HttpStatus` mapping for `mint_plugin_hook_token` non-2xx responses; the gateway's error body is preserved so `bridge sync` PARTIAL lines carry the underlying RFC 6749 §5.2 reason instead of an opaque status code.

### Added

- `bridge doctor` command groups the bridge-side self-checks (config, credential source, mint JWT, gateway reachable, authenticated whoami, loopback secret, pinned pubkey, cowork marketplace registration) into a single one-line-per-check diagnostic surface; exits 11 on any failure.
- `SyncError::GatewayUnauthorized { endpoint, status }` represents gateway 401/403 from `/manifest` and `/pubkey` as a distinct error with exit code 10 and an actionable "run `systemprompt-bridge login <sp-live-...>`" message; the GUI surfaces it via the new `sync-gateway-unauthorized` Fluent string, and the `sync-no-credentials` string handles the no-PAT-configured case.
- Typed wire-shape structs for the Cowork host adapter: `KnownMarketplacesFile`, `KnownMarketplaceValue`, `InstalledPluginsFile`, `InstalledPluginInstall`, and `MarketplaceMetadata`, replacing the ad-hoc `serde_json::Value` traversals.
- Unit test coverage for the Cowork host adapter (`crates/tests/unit/bridge/cowork-plugins`): canonical marketplace shape, known-marketplaces / installed-plugins / settings upsert behaviour, and path sanitisation.

## [0.9.2] - 2026-05-27

### Changed

- Track `systemprompt-identifiers` and `systemprompt-models` 0.12.0 dependency pins.

## [0.9.1] - 2026-05-25

### Changed

- **Internal lint and visibility cleanup.** Bridge sources adopt the workspace's tightened clippy baseline (`unreachable_pub`, `allow_attributes_without_reason`, `redundant_pub_crate`, `let_underscore_must_use`) — visibility narrowed from `pub` to `pub(crate)` where appropriate, MDM helpers cfg-gated to the OSes that consume them, best-effort `Result` discards justified with `tracing::warn!`. No user-visible behaviour change.

## [0.9.0] - 2026-05-22

### Fixed

- **Session binding: bridge persists and binds its stable `x-session-id`.** The bridge now stores its `x-session-id` and replays the same value across requests, so `/v1/messages` and `/bridge/heartbeat` no longer return `401 "Session missing or revoked"` or `"X-Session-ID does not match"` after the first call. A regenerated session id per request previously orphaned the gateway-side session record.

### Added

- **`HostSync` trait + central dispatcher (`sync/host_sync.rs`).** Every bridge integration that materialises manifest data on disk (Cowork synthetic plugin, Codex managed resources, Windows MDM, …) implements one `HostSync` trait with `apply` / `clear` methods. The dispatcher in `sync::apply` walks the static `registry()`, decides per-host whether to call `apply` or `clear` based on the manifest's `enabled_hosts` field, and uniformly logs each outcome — emitter authors no longer reinvent the toggle-and-cleanup gate. Replaces the imperative pile of "call `cowork::publish` then `mdm::reconcile` then …" in `sync::apply::mod`.
- **Codex CLI host emitter (`integration/codex_cli/managed_resources.rs`).** Implements `HostSync` for Codex by writing a single plugin bundle that matches Codex's documented discovery contract (verified against the published JSON schema and `developers.openai.com/codex/plugins/build`). Skills and MCP servers land as one Codex plugin at `~/.codex/plugins/cache/systemprompt/systemprompt-managed/current/`, containing `.codex-plugin/plugin.json` (carrying the manifest version), `skills/<id>/SKILL.md`, and `.mcp.json`. A `[plugins."systemprompt-managed@systemprompt"] enabled = true|false` block in `~/.codex/config.toml` is the user-facing toggle; every other key in `config.toml` (user MCP servers, sibling plugins, model providers) is preserved across `apply` and `clear`. Earlier iterations wrote to `~/.codex/skills/` and to top-level `[mcp_servers.sp_*]` blocks — neither path is read by Codex, so the marketplace bundle was invisible inside the CLI.
- **Codex provider-profile install (`integration/codex_cli/install.rs`) targets the documented system path and merges instead of overwriting.** Linux/macOS now write to `/etc/codex/config.toml` (the prior `/etc/codex/managed_config.toml` was undocumented and not in Codex's config chain). The install reads the existing target, strips bridge-owned keys (`model_provider`, `model_providers.systemprompt`, `otel`, `analytics`), deep-merges the freshly generated TOML on top, and atomic-writes — so prior keys survive reinstall. New `CODEX_SYSTEM_CONFIG` env var overrides the system path for hermetic tests.
- **GUI: per-host enable toggle posts to gateway (`gui/handlers/agents.rs::on_set_enabled_host_requested`).** New IPC entrypoint sends `POST /v1/bridge/enabled-hosts` with the host id and desired state, then emits `UiEvent::SetEnabledHostFinished`. The GUI no longer mutates local `agents.json` directly — host enable state is a profile fact owned by the gateway and arrives back through the next signed manifest. Matches the broader rule that host enable state lives in the user profile, not local toggles.

### Changed

- **`integration/codex_cli/install.rs` (326 lines, hand-rolled base64) split into `install/{mod,merge,render}.rs`.** `mod.rs` keeps `write_profile` / `install_profile` / `writable`; `merge.rs` owns `merge::install` plus the `OWNED_*` constants for bridge-owned keys; `render.rs` owns TOML + mobileconfig rendering. The 93-line `render_managed_toml` is now 16 lines, dispatching to `write_provider_block` / `write_otel_block` / `write_models_block`. Hand-rolled `base64_encode` replaced with `base64::engine::general_purpose::STANDARD`. WHAT-doc-comments on `OWNED_*` consts collapsed into a single module-level `//!` block.
- **Silent error sites in `sync/mod.rs::persist_last_sync` and `integration/codex_cli/probe.rs::parse_into_keys` now log via `tracing::warn!`.** Three `let _ = …` / `.unwrap_or_default()` discards in `persist_last_sync` and one `.ok()?` on TOML parse in `probe::parse_into_keys` previously dropped errors silently; each now logs context (path, dir, source) before the best-effort fallback.
- **Bridge codex tests no longer use `unsafe { env::set_var }`.** `crates/tests/unit/bridge/{sync,integration}/src/codex_*` rewritten on top of the `temp-env` crate (added as workspace dev-dep) — each test scopes `CODEX_HOME` / `CODEX_SYSTEM_CONFIG` via `temp_env::with_var(s)` instead of mutating process env, removing the manual `Mutex<()>` lock and the `unsafe` block.
- **`agents_state` simplified.** `migrate_from_existing_profiles` (which probed every registered host on startup) and `store_exists` are gone. Replaced by `save_from_manifest(enabled_hosts: &[String])`, called from `sync::apply` whenever a new signed manifest is applied. `save` is now `pub(crate)`. The first-run "auto-enable everything that looks installed" migration is no longer needed because the manifest is authoritative.

### Added

- **Cowork plugin sync (`integration/cowork_plugins/`).** Per-plugin marketplace publish into the active `<session>/<org>/cowork_plugins/` tree: marketplace upsert, installed-plugin upsert, enabled-settings upsert (foreign-entry preservation throughout), plus a per-plugin `claude-plugin/plugin.json` patch that wires `hooks/hooks.json`. Reverse `unpublish` path included.
- **OAuth hook-token client (`auth/plugin_oauth.rs`).** Per-tenant OAuth client + plugin-scoped hook-token cache. `client_secret` is stored in the OS keystore (Keychain on macOS, Credential Manager on Windows, Secret Service on Linux) via the `keyring` crate; only `client_id`, `token_endpoint`, and `scopes` remain on disk. Legacy 0600 JSON files containing `client_secret` are transparently migrated into the keystore on first read.
- **Typed `hooks.json` schema (`sync/apply/hooks_schema.rs`).** `HooksFile`/`HookEntry`/`HookKind` replace the prior `serde_json::json!` literal in `sync/apply/hooks.rs::write_hooks_json`.
- **`fsutil` module.** Single owner of `atomic_write_0600` (parent dir 0o700, fsync before rename), `copy_dir_recursive`, and `read_optional`. Removes three duplicate implementations across `auth/`, `sync/`, and `integration/`.
- **`mcp_registry` (top-level).** Cross-cutting registry consumed by `proxy::forward`, `install::mdm::*`, and `sync::apply` — relocated from `proxy::mcp_servers` because `proxy::` mis-suggested ownership.

### Changed

- **`gateway/` split.** `gateway/mod.rs` (489 → 79 lines) into `mod` (client) + `errors` + `types` + `fetch` + `auth`.
- **`integration/cowork_plugins/emit.rs` split** (411 → 245 lines) into `emit` (publish/unpublish orchestration) + `upsert` (registry/settings file plumbing). Visibility narrowed: `mod {emit, marketplace, registry, settings}` are now `pub(crate)`; only `KNOWN_MARKETPLACES_FILE`, `publish`, `resolve_target`, `unpublish`, and the test surface stay `pub`.
- **`install/mod.rs` split** (313 → 170 lines) by extracting orchestration glue (`bootstrap_install`, `run_apply*`, `resolve_*`) to `install/apply.rs`.
- **`sync/apply/plugin.rs` split** (322 → 184 lines) by moving `materialize_hook_token`, `write_hooks_json`, and `ensure_plugin_json_hooks_field` to `sync/apply/hooks.rs`.
- **`sync/apply/mod.rs::rewrite_loopback_urls`** uses `url::Url::set_host`/`set_scheme` against `Host::Ipv4`/`Host::Ipv6` loopback checks instead of string-splitting helpers (`split_url`, `split_origin`, `is_loopback_host` deleted).
- **`SignedManifest` family moved to shared crate.** `SignedManifest`, `UserInfo`, `PluginEntry`, `PluginFile`, `SkillEntry`, `AgentEntry`, `ManagedMcpServer`, `ManifestVersion`, plus the manifest-scoped typed IDs (`PluginId`, `SkillId`, `Sha256Digest`, `ManifestSignature`, `ToolPolicy`, etc.) now live in `systemprompt_models::bridge::*`. Bridge re-exports preserve every existing call site; the bridge-side ed25519 `verify(...)` is provided via the new `SignedManifestVerify` extension trait (orphan-rule workaround).

### Fixed

- **Proxy: `/healthz` and `/otel` no longer rejected by the loopback-secret gate (`proxy/server.rs`).** The healthz short-circuit ran *after* the bearer check, and only matched `GET`, so the bridge's own `HEAD /healthz` probe (and any external poller) flooded the activity log with `403 (bad secret; presented_fp=<empty>)` every 30 s. Codex's OTLP-HTTP exporter posting to `/otel` hit the same gate — OTLP has no clean way to inject the loopback bearer. Both paths are now handled by an explicit `is_unauthenticated_path(method, path)` predicate evaluated *after* the loopback-host check and *before* the bearer check: `GET`/`HEAD /healthz` short-circuits in-process; `POST /otel` (and `/otel/*`) forwards through `forward::forward`, which already strips the inbound `Authorization` and injects the upstream bearer from `TokenCache`. Loopback-origin enforcement is unchanged. Same change folds the response building into a single shared `forward_to_gateway` helper, collapses `health_response` to two lines via `simple_response("")`, replaces the unreachable `Response::builder().unwrap_or_else` fallback with infallible `Response::new` + `HeaderValue::from_static`, fixes a multi-strip bug in the bearer parser (`trim_start_matches("Bearer ")` could strip repeats; now `strip_prefix` once), and tightens `sha256_8` and `record_stats` (callers now pass an already-computed `latency_ms` instead of `Instant`).

- **`///` rustdoc and TODO/FIXME flags purged from binary modules** (`bin/bridge/**` is a binary — `///` is banned). ~50 paraphrase blocks removed; ~20 load-bearing why-lines preserved as `//`. The `obs.rs` panic-hook ordering note is retained as a `// Why:` comment; the `gui/server.rs` focus-IPC FIXME was reworded as a deliberate-trade-off explanation (TCP+CSRF works identically across all three platforms in <100 lines).

- **Breaking — `cowork` rename completed end-to-end.** Bridge sends canonical `x-session-id` / `x-context-id` headers (issued from the new `SessionContext`) and uses the renamed gateway routes (`/v1/bridge/*`, `/v1/auth/bridge/*`). Internal macros are now `bridge_define_id!` / `bridge_define_token!`. Env vars: `SP_COWORK_*` → `SP_BRIDGE_*`. Config file: `~/.config/systemprompt/systemprompt-cowork.toml` → `systemprompt-bridge.toml`. A `0.7.x` bridge cannot talk to a `0.8.0` gateway and vice versa.

### Added

- **Heartbeat loop (`proxy/heartbeat.rs`).** Spawned next to the token-refresh loop in `proxy/server.rs::start`; POSTs `/v1/bridge/heartbeat` every 30 s with `session_id`, `bridge_version`, OS, hostname, `last_activity_at`, and a snapshot of `ProxyStats` (forwarded count, tokens in/out). The gateway records the row in `bridge_sessions`, making this bridge visible to `systemprompt admin bridge list` even between inference requests. On `401` the token cache invalidates so the next tick re-authenticates.
- `SessionContext::touch_activity()` is called on every successful messages-path forward, so the heartbeat distinguishes "alive but idle" from "alive and serving traffic".
- Bridge sends canonical `x-session-id` and content-derived `x-context-id` headers on every `/v1/messages` forward, enforcing conversation grouping at the gateway.

### Fixed

- **Tech-debt sweep on the per-agent enabled feature.**
  - `auth::setup::clean()` now also removes `~/.config/systemprompt/agents.json`. Previously a `clean` left stale enabled state behind.
  - Existing users get a one-shot migration on first run after upgrade: when no `agents.json` exists yet, `gui::run_agents_migration_if_needed` probes every registered host and auto-enables those whose `profile_state` is already `installed`. The old "everything is silently disabled" behaviour after upgrade is gone.
  - `apply_host_snapshot` no-ops (and removes any existing entry) when the host has been disabled mid-probe, so an in-flight probe that finishes after a disable can no longer re-insert the host into `state.hosts`.
  - `agents.setEnabled` is now idempotent: setting the same value twice returns `{ changed: false }` and skips both the activity-log line and the wasted manual probe.
  - Setup-wizard "Install profile" handler now records which step failed (`enable` / `generate` / `install`) on the button's `data-failed-stage` and surfaces the underlying error message in `title`, so partial failures stop being silent.
  - `proxy_probe::probe` does an actual HTTP `HEAD /healthz` after the TCP connect and reports the status on `ProxyHealth.http_status`, so a stray process listening on port 48217 no longer claims `Listening` for the bridge proxy.
  - Renamed `GatewayClient::fetch_cowork_profile` → `fetch_bridge_profile` to finish the cowork→bridge rename on the bridge side. Server endpoint path and `CoworkProfile` type are unchanged (server contract).
  - Moved `agents_state` from `gui/` to a top-level module so non-GUI builds (`auth::setup::clean`) can reference it without `cfg`-gates.
- **Setup wizard's Install button was a no-op.** It only called `host.profile.generate`, which writes a profile file but does not copy it into the OS-managed location, so the host's `profile_state` never flipped to `installed` and the UI stayed on "Install profile" forever. Now the setup-agents handler enables the host (so the new gating doesn't reject the call), generates the profile, and immediately installs the resulting path — three IPCs in sequence — matching the user's intent of "set up this agent".
- **Local proxy probe always returned `Unconfigured` until at least one host had been installed**, because `AppState::first_configured_proxy_url` derived the probe target from `host.profile_keys.inferenceGatewayBaseUrl` (which only populates after install). The bridge owns the proxy and knows its port — now `first_configured_proxy_url` returns `http://127.0.0.1:<proxy.handle.port>` when the proxy is running, falling back to the host-derived URL only if the proxy hasn't started yet. Cures the "awaiting first launch" badge that stuck even when Claude was actively routing through the proxy.

### Added

- **Per-agent enable/disable, persisted across runs.** Every registered host (Claude Desktop, Codex CLI, …) now has an explicit `enabled` flag stored in `~/.config/systemprompt/agents.json`. Hosts default to **disabled** so a fresh install never silently probes integrations the user hasn't opted into. New IPC `agents.setEnabled({ hostId, enabled })` toggles the flag, persists it, and (when re-enabling) fires a one-shot manual probe. The host card grows an Enable/Disable button; disabled cards render as a dimmed lede with the toggle and no action buttons. `host.probe`, `host.profile.generate`, `host.profile.install`, `agent.uninstall`, and `agent.openConfig` reject disabled hosts with `Conflict`. Status summaries and the rail's agent count consider only enabled hosts.

### Fixed

- **Codex (and every other registered host) was probed every 30 s even when not installed**, spamming the activity drawer with `[codex-cli] re-verifying profile and process` / `re-verify complete — profile not installed, process not running` pairs forever. Two changes: (1) the periodic ticker (`gui/hosts/tick.rs`) now skips hosts that aren't `enabled`, and (2) tick-driven probes are silent in the activity log unless the snapshot's `profile_state` *kind* or `host_running` actually flipped, in which case a single `[host] state changed — …` line is appended. User-triggered Re-verify clicks keep the existing verbose `[host] re-verifying…` / `re-verify complete — …` pair via a new `ProbeCause::{Tick,Manual}` enum threaded through `HostUiEvent::Probe{Requested,Finished}`.
- **Sync failed with no visible reason — only the literal string `sync-failure` in the activity drawer.** `i18n::t_args("sync-failure", ...)` and `i18n::t("sync-cancelled")` had no matching keys in `web/i18n/en-US/bridge.ftl`, so the fallback returned the bare key and the underlying error string was discarded. Added `sync-failure = Sync failed: { $error }` and `sync-cancelled = Sync cancelled.`, switched the error formatter in `gui/handlers/sync.rs` to `{:#}` to print the chain, and added `tracing::info!` / `tracing::error!` so the same message lands in the log file as well as the UI.
- **Sync hard-failed on a redundant directory-level hash check.** After every per-file SHA-256 (signed by the gateway) was verified, `sync/apply/plugin.rs` re-hashed the staging directory with `directory_hash` and compared against `plugin.sha256`. The bridge's hash algorithm did not match the gateway's, so the staged-vs-manifest comparison always failed (`hash mismatch for plugin enterprise-demo: expected …6930, got …495a`), aborting the entire sync. Removed the directory-level check and the now-dead `directory_hash` / `collect_files` helpers from `sync/hash.rs`. Per-file hash verification on a fresh staging dir already guarantees byte-identical contents from the signed manifest.
- **External link clicks could fail silently.** `gui/window/mod.rs::open_target` discarded the `Command::spawn` result, so when `xdg-open` / `cmd /C start` was missing or failed there was no record. Now logs the attempt at info level and the spawn error at error level.
- **Footer links now open via an explicit IPC instead of `target="_blank"`.** Added an `openExternalUrl` IPC command in `gui/command.rs` (HTTPS-only allowlist via `is_safe_external_url`, dispatches through the `opener` crate) and exposed it on the JS side as `bridge.openExternalUrl(url)`. `sp-footer` now handles the docs/licensing clicks through a `data-action="open-external"` delegate that calls the IPC, so the path no longer depends on the WebView's `with_new_window_req_handler` firing.
- **Footer rendered `v0.7.0 (unknown, unknown)` when `vergen` could not read git state.** `hasBuildMeta` only suppressed the literal string `"unknown"`. Added `isMissing()` to also catch empty values and unreplaced `__PLACEHOLDER__` sentinels, so the parens block disappears when build metadata is missing instead of leaking the fallbacks into the UI.
- **Help & Support section was poorly styled — buttons stretched to the drawer's right border with no breathing room.** Restyled `.sp-activity__help` in `web/css/drawer.css` as a self-contained card: outer margin so it no longer touches the drawer borders, panel background and rounded border for separation, larger gap between title and buttons, and constrained `.sp-btn-ghost` width with left-aligned labels and consistent vertical rhythm.

- **Windows GUI rendered a blank `about:blank` window.** wry 0.55 rewrites custom URI schemes to `http://<scheme>.<host>/...` on Windows/Android because WebView2 cannot register arbitrary schemes, so navigating to `sp://app/index.html` silently failed. Use `http://sp.app/index.html` on those targets and allow the rewritten origin in `allow_navigation`.
- **Native menu bar showed raw i18n keys** (`menu-edit`, `menu-view`, `menu-help`, …). The menu builder calls `i18n::t("menu-*")` but `web/i18n/en-US/bridge.ftl` had no matching entries, so the fallback returned the keys verbatim. Added the seven missing translations.
- **Re-verify button looked broken — actually silent.** Clicking "Re-verify" on a host card fired `host.probe`, ran the probe, applied the snapshot, and emitted `host.changed`, but appended nothing to the activity log. From the user's seat it looked like the click was lost. Added "[host] re-verifying…" before the spawn and "[host] re-verify complete — profile installed, process running" (or equivalent) when the snapshot is applied.
- **Bridge silently continued when the local proxy failed to start.** `gui::run` discarded the result of `proxy::start_default()` and proceeded to render the GUI even when the bind failed, so any profile generated afterwards pointed Claude Desktop / Codex at a dead `127.0.0.1:48217` (`ERR_CONNECTION_REFUSED`). Now: log success/failure to the activity drawer at startup, and refuse profile generation when the proxy isn't listening rather than handing out a profile that can't possibly work.
- **Loopback secret could drift between proxy and host profiles.** Both proxy startup and profile generation called `secret::load_or_mint`, which silently re-minted on a missing key file. If the file disappeared between the proxy's startup read and a later profile-gen read (or vice-versa), the proxy and the host's installed profile ended up with different keys, producing `forbidden: bad loopback secret` (HTTP 403) on every host request. Replaced the dual API with a single source of truth: `proxy_init()` (proxy startup — loads or mints, caches in a process-global `OnceLock`) and `for_profile()` (profile generation and MDM templates — read-only; errors out if the proxy hasn't started). After this change the proxy and any profile generated within the same process can never disagree.
- **Removed broken `__TOKEN__` cache-buster.** Every static asset URL carried `?t=__TOKEN__`, but `assets.rs` substituted `__TOKEN__` with the empty string — leaving meaningless `?t=` query strings that did nothing AND created two distinct module identities whenever one file imported `foo.js?t=` and another imported `foo.js`. Modules ran twice, the second `customElements.define` threw `NotSupportedError`, and the GUI rendered an empty body. Stripped the placeholder from `web/index.html`, `web/css/fonts.css`, all 27 imports in `web/js/index.js`, and the three `.replace("__TOKEN__", "")` call sites in `src/gui/assets.rs`. Embedded assets only change on rebuild, so no cache-buster was ever needed.

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
