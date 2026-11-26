# 05. Splitting `bin/bridge`

Rule: **core owns everything that carries trust or that the server also runs; the extension owns the product surface.**

Method: **split in place first.** Inside core, the modules are carved out of `bin/bridge` into core device crates under the layer rules, with their tests, while the bridge binary still builds on them and ships a release that way. Only when that release has run in production is the remaining window code replaced by the extension. No module is refactored in the commit that moves it.

Line counts are from `future` at `d17b79b70`. The target crate names are proposals; S6 fixes them (see `02-target-architecture.md` section 3 on crate count).

## 1. To core

| Bridge module | Lines | Target | Notes |
|---|---|---|---|
| `ids.rs`, `headers.rs`, `hash.rs` | 165 | `systemprompt-identifiers`, `systemprompt-models` | Typed ids and header names spelled once |
| `gateway/` (types) | part of 2,556 | `systemprompt-models::bridge` | Discovery document, manifest envelope, model view types |
| `gateway/` (client, discovery, manifest fetch and verification) | part of 2,556 | `systemprompt-client` (device calls); verification beside the gateway's signing code | Strict Ed25519 over the payload string as signed; never re-canonicalised. Signer and verifier tested as a round trip |
| `auth/` | 2,300 | `device` | Provider chain; `AuthProvider` registry becomes a core-standing slot |
| `config/` | 4,213 | `device` | `ConfigStore`, `PolicyStore`; comment- and layout-preserving TOML edits |
| `sync/` | 4,163 | `device` | Acceptance, replay, apply, manifest cache, process lock, provisioning |
| `mcp_registry.rs`, `last_sync.rs`, `instance/`, `activity.rs`, `progress.rs` | ~950 | `device` | On-disk MCP snapshot; last-sync sentinel carrying `AutoUpdatePolicy` |
| `fsutil/`, `basedirs.rs` | 413 | `device` (`platform`) | Atomic verified writes; base directories on three systems |
| `windows_acl/` | 1,038 | `machine` (`platform`) | Windows ACL handling is unforgiving. Verbatim |
| `proxy/`, `proxy_probe/` | 7,834 | `proxy` | Loopback server, credential classes, `scoped_token`, token cache, heartbeat, usage, forwarders |
| `host_sync/` | 644 | The `HostSync` contract into `device`; emitters into each `host-*` | Registry becomes a slot |
| `integration/` | 12,533 | Contract into `device`; per-product code into `host-claude`, `host-codex`, `host-opencode`, `host-hermes`; `agent_fleet`, `agent_health`, `agent_verdict` into `device` | `HostApp` registry becomes a slot |
| `install/` | 9,594 | `machine` | Apply, bootstrap, elevated job and protocol, managed MCP, MDM, policy writer, schedule apply, templates |
| `elevation_gate/`, `schedule/`, `sysproc/`, `winproc.rs`, `takeover.rs` | ~1,250 | `machine` | |
| `drift.rs`, `diagnostics_state/`, `validate/`, `claude_policy.rs`, `gui_policy/` | ~1,750 | `machine`, `host-claude` | Desired-versus-actual profile; `doctor` and `validate` inputs |
| `drive/` | 3,417 | `drive` | Merged with the harness's `sp-drive` (5,249); one implementation |
| `feedback/`, `feedback_capture/` | 2,035 | `device` | Runtime-file receipts; capture from host sessions |
| `update/`, `release_version.rs`, `buildinfo.rs` | 1,874 | `update` | Fail-closed organisation policy stays |
| `verdict.rs` | 57 | `device` | Verdict codes |
| `cli/` | 4,936 | `cli_command` declarations on the owning members | Names, flags and exit codes are a contract (section 4) |
| `context/` | 328 | Replaced by the device composition's built state | `BridgeContext` was the single composition root |
| `obs/`, `stdio.rs`, `warn_once.rs`, `tasks.rs` | ~670 | `device`; `tasks.rs` deleted for `systemprompt_traits::{BackgroundTasks, OwnedTask}` | |
| `user_alert/`, `single_instance/`, `window_state.rs`, `probe_cache.rs` | ~730 | `device` (`platform`) | Single instance kept from the bridge, not the harness |

## 2. To the extension (template, MIT)

| Bridge module | Lines | Becomes | Notes |
|---|---|---|---|
| `gui/` | 12,418 | Native views in `extensions/harness/views`: hosts, marketplace, first run, inbox; tray in `extensions/harness/shell` | Rewritten, not moved. The harness has no webview |
| `web/` | 35,372 (190 files) | Retired | Its fixture states (`web/dev/fixtures/*.json`) are kept as the parity checklist |
| `wire/` | 1,304 | Retired with the webview. Payload shapes inform the views' state types | TypeScript bindings and `bridge-bindings` go |
| `dev_preview/`, `web_assets.rs`, `webview2.rs`, `i18n.rs`, `help.rs` | ~750 | Retired or replaced by the extension's strings bundle | |
| `brand.rs` | 176 | `BrandDecl` data type in core; the declaration in `extensions/harness/brand` | The seam already exists |
| `assets/`, `macos/`, `scripts/`, `build.rs` | n/a | `extensions/harness/brand` and `packaging/` | Icons, `Info.plist`, app bundle script |
| `main.rs` | 13 | The extension's `main.rs` | The single build site |

## 3. Deleted from core when the bridge binary goes

- `bin/bridge/` (by then a window and a `main`), its `Cargo.lock`, `CHANGELOG.md`, `clippy.toml`, `bindings/`, `gates/`.
- Scripts: `lint-bridge-css-tokens`, `-i18n`, `-js-imports`, `-no-window`, `-typed-warnings`, `-lints-sync`, `-file-size`, `check-bridge-bindings`, `bridge-native-tests`, `bridge-native-crates.txt`. `lint-bridge-layers`, `-globals`, `-verdicts` are folded into core's own layer, globals and verdict gates.
- Workflows: the `bridge-native` matrix in `quality.yml` (replaced by a scoped native check for device crates), `coverage-bridge.yml`, bridge jobs in `ci.yml` and `coverage.yml`. `release-sign.yml` moves to the template.
- Recipes: `build-bridge*`, `bundle-bridge-mac`, `bridge-preview`, `lint-bridge*`.
- The `inventory` exception in the root-workspace gate.
- `CLAUDE.md` and `AGENTS.md` bridge sections.
- The template's `bridge/` directory and its `CORE_REF`.

The 36 bridge test crate directories (66,890 lines) move with the code they test into core's test workspace under the device crates' names. The 25 that run natively keep the database-free rule; `lint-native-test-deps` stays, renamed for device crates.

## 4. Contracts that must not change

| Contract | Who depends on it |
|---|---|
| Command names, flags and exit codes (`0`, `2`, `3`, `4`, `5`, `8`, `10`, `64`) | Other agents' credential-helper configuration, schedulers, MDM scripts |
| A bare invocation with a console is the credential helper | Scheduler tasks, pipes |
| The credential envelope `{ "token", "ttl", "headers" }` | Anthropic's `inferenceCredentialHelper` contract |
| Loopback port `48217`, `/healthz`, `/__bridge/whoami`, `x-systemprompt-bridge-reason` | Written into every managed host's profile |
| Scoped-token derivation (domain-separated HMAC-SHA256) and route classes | Tokens already on disk in host profiles |
| Policy locations, the `manifestTrust` record, the `<Brand>BridgePolicyWriter` task, `binding.json`, `accepted.json` | Administrators and MDM |
| On-disk layout under `<config_dir>/<brand>/`, `org-plugins/`, sidecar ledgers | The fleet handover imports them |
| Wire ids: plugin ids, Codex marketplace and provider ids, the governance hook id | The gateway's signed manifest |
| `COMPAT_VERSION` semantics and `min_bridge_version` | The gateway's floor and the admin Devices page |

Each is pinned by a conformance fixture before any module moves.
