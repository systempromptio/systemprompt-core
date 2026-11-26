# 01. Current state

Everything here was read on 2026-11-26. Nothing was built or run.

| Source | Ref read |
|---|---|
| `systemprompt-harness` | `0839a31` (GitHub `systempromptio/systemprompt-harness`, `main` = `next`) |
| core `future` | `d17b79b70` |
| core `next` / `main` | `dd3ac508f` (0.87.0) |
| template `future` | `f7e7c964` |

## 1. `systemprompt-harness`

A native Windows desktop coding agent in Rust on GPUI (`gpui-kit =0.9.1`), governed by a systemprompt instance. 22 production crates, 81,661 lines; 22 test crates, 24,717 lines; version `0.1.0`; BUSL-1.1; not published.

### 1.1 Crates

| Ring | Crate | Lines | Owns |
|---|---|---|---|
| engine | `harness-ids` | 277 | Typed identifiers |
| engine | `harness-tasks` | 489 | Owned background work |
| engine | `harness-config` | 2,978 | Paths, settings, agents, skills, instructions, permission rules, commands |
| engine | `harness-store` | 2,559 | SQLite store, session data model, change feed |
| engine | `harness-llm` | 2,488 | Streaming clients for `/v1/messages` and `/v1/chat/completions` |
| engine | `harness-tools` | 6,591 | `Tool` contract; shell, read, write, edit, patch, glob, grep, fetch, todo |
| engine | `harness-mcp` | 3,249 | MCP client (streamable HTTP, stdio) |
| engine | `harness-workspace` | 3,258 | Snapshots, revert, diffs, file watching, formatters |
| engine | `harness-lsp` | 3,423 | Language server client |
| engine | `harness-prompt` | 1,012 | System prompt assembly, built-in agents |
| engine | `harness-engine` | 6,289 | Agent loop, `Governor` contract, asks, subagents |
| sp | `sp-protocol` | 3,170 | Wire types, canonical JSON, manifest version order, view types |
| sp | `sp-secure` | 1,643 | DPAPI sealing, owner-only ACLs, atomic verified writes (Windows real, others stub) |
| sp | `sp-client` | 2,030 | HTTP client for an instance |
| sp | `sp-projection` | 2,267 | Manifest to files in the harness config dir |
| sp | `sp-drive` | 5,249 | Documents API and local folder sync |
| sp | `sp-accounts` | 2,645 | Instances, sign-in flows, sealed PAT, JWT renewal, key trust |
| sp | `sp-governance` | 1,483 | The real `Governor`: gateway calls, verdicts, telemetry, usage, model list |
| sp | `sp-sync` | 3,185 | Poll loop, acceptance, apply, runtime files, receipts, heartbeat |
| app | `sp-brand` | 346 | A two-variant `Brand` enum (the default and one install's) by cargo feature; channels |
| app | `harness-ui` | 5,990 | Components, theme tokens, two palettes (the default and one install's), markdown, diff |
| app | `harness-desktop` | 21,040 | Composition root, state, views, strings |

`harness-desktop` by directory: `app/` 2,488 (services, startup, single instance, deep link), `state/` 1,821, `strings/` 1,211, `views/` 15,292 (composer 1,117; docks 860; drive 2,808; frame 329; governance 2,346; pickers 704; settings 2,349; signin 1,797; transcript 2,359).

### 1.2 What is good and must survive

- Three rings with inward-only dependencies, `lint-layers` with an empty exception list.
- **Governed by construction.** The engine is built with a `Governor` and cannot make a model call without one. `NullGovernor` exists only for engine tests.
- The shell renders from the store's change feed; it holds no second copy of a session.
- A refused tool call is stored as `ToolState::Error` with a typed `denied` marker; the shell never parses text.
- Semantic theme tokens: components paint from a `Palette`, never raw toolkit colours.
- Core's clippy lint table copied unchanged; typed ids; owned tasks; separate test workspace.

### 1.3 What blocks the vision

| Area | Today |
|---|---|
| Composition | `Services::start` wires a fixed graph. An untyped `Extensions` map (`RwLock<HashMap<TypeId, _>>`) carries late handles. No member contract, no admission, no standing |
| Brand | A two-variant enum in the kernel names an install. `packaging/windows/package.ps1` repeats the table |
| Themes | Two palettes compiled in, keyed by `Brand` |
| Tools | `harness_tools::builtin(config) -> Vec<Arc<dyn Tool>>`; Drive tools added by hand in the shell |
| Views | Fixed modules under `views/` |
| Core linkage | Only `systemprompt-models` and `systemprompt-identifiers` 0.87 |
| Platforms | Windows x64 ships. macOS type-checks and runs tests. Linux is not built. 50 `cfg(windows)` sites with stubs beside them |
| Process | Commit to `main`, fast-forward `next`; no pull requests; no changelog; no release |
| Gates | 29 scripts forked from core's 54 |
| Exercise | 942 tests pass on macOS. **Never launched on Windows. Never signed in**: no instance knows host `harness` |

### 1.4 Its own open work (from its handover, still open)

1. Core: add host `harness`.
2. First run on Windows.
3. Live gate against a local instance.
4. App icon artwork.
5. Missing UI: delete a session, compact on request, choose the Drive folder.
6. Repository settings (`next` default, `main` protected).

## 2. `bin/bridge` (core)

One binary, `systemprompt-bridge`, 75,290 lines of Rust plus a web tree of 190 files and 35,372 lines. Its own cargo workspace inside core; links only `systemprompt-models` and `systemprompt-identifiers`; released with core at core's version.

### 2.1 Roles

1. **Credential helper.** Emits `{ "token", "ttl", "headers" }` on stdout for a host that asks.
2. **Sync agent.** Pulls the user's signed manifest (plugins, skills, agents, hooks, MCP allowlist) and applies it.
3. **Loopback inference proxy.** `127.0.0.1:48217`. A host presents a scoped local credential; the bridge swaps it for a fresh gateway JWT.

### 2.2 Modules

| Module | Lines | Does |
|---|---|---|
| `integration/` | 12,533 | `HostApp` per host: probe, desired profile, generate, install, remove, open, quit. Claude Desktop, Claude Code, Cowork plugins and artifacts, Codex app and CLI, OpenCode app and CLI, Hermes. Also `agent_fleet`, `agent_health`, `agent_verdict`, `managed_skills`, `enrol`, `lifecycle`, `reapply` |
| `gui/` | 12,418 | winit and wry webview, tray, first run, hosts, marketplace, inbox. Windows and macOS only |
| `install/` | 9,594 | Install and uninstall, pinned manifest key, MDM snippets, the SYSTEM policy-writer task, elevation, managed profiles |
| `proxy/` | 7,478 | Loopback server, credential classes, scoped tokens, token cache, heartbeat, usage, forwarders |
| `cli/` | 4,936 | 17 commands with a fixed exit-code contract |
| `config/` | 4,213 | Config and policy stores |
| `sync/` | 4,163 | Acceptance, replay, apply, manifest cache, process lock |
| `drive/` | 3,417 | Drive folder sync |
| `gateway/` | 2,556 | Gateway client, discovery, manifest fetch and verification, model view |
| `auth/` | 2,300 | Provider chain (session, then PAT), keyrings |
| `feedback/`, `feedback_capture/` | 2,035 | Runtime-file receipts; capture from host sessions |
| `update/` | 1,801 | Self-update with a fail-closed organisation policy |
| `wire/` | 1,304 | Payloads the webview is written against; exported to TypeScript |
| `windows_acl/` | 1,038 | DACL handling |
| `host_sync/` | 644 | `HostSync` emitters: manifest into each host's config tree |
| others | ~7,000 | `validate`, `diagnostics_state`, `single_instance`, `obs`, `elevation_gate`, `fsutil`, `proxy_probe`, `context`, `sysproc`, `instance`, `schedule`, top-level files |

### 2.3 Seams it already has

- **Brand.** `run_with_brand(&'static Brand)`; a downstream binary crate supplies its own name, paths, default gateway and assets. Wire identifiers are deliberately not brand fields. `COMPAT_VERSION` is the wire version; a displayed version on another minor is logged.
- **Four link-time registries** (`inventory`): `HostApp`, `HostSync`, `AuthProvider`, `MarketplaceSource`. The bridge is the one place core still allows `inventory`.
- **One composition root.** `BridgeContext`, built once in `cli::run_with_args`.
- **A layer order** enforced by `lint-bridge-layers`: leaves, `config`, `gateway`, `mcp_registry`, `auth`, `validate`/`update`, `proxy_probe`, `proxy`, `context`, `host_sync`, `install`, `integration`, `sync`, `wire`, `gui`, `cli`.

### 2.4 What it costs core

- Its own lockfile (49 commits in 45 days), lint tables mirrored from the root by hand.
- 13 `lint-bridge-*` scripts; a `bridge-native` matrix on Windows and macOS in `quality.yml`; `coverage-bridge.yml`.
- 36 test crate directories, 66,890 lines; 25 run in the native matrix and must stay database-free (`scripts/bridge-native-crates.txt`).
- `release-sign.yml`: Linux x64, macOS, Windows x64; CycloneDX SBOMs; provenance; cosign; GitHub release. Homebrew and Scoop manifests follow by hand and drift.
- Every core release rebuilds and re-signs it.

## 3. Core on `future`

- A member implements `Extension`: `metadata()` and one `contribute(&mut Contributions)`. Declarations only store.
- `Composition` holds members with a standing, `Core` (with privileges conferred from a fixed table) or `Install`. `.with(E)` adds an install member; `.without` removes an optional core member. Nothing replaces a core member.
- Two phases: `declare` (before any profile) and `admit` (reads the profile, drops disabled and withheld members, checks routes and providers). The runtime runs provide fns, factories and slots over the admitted set.
- `Slot`: a contribution point owned by a capability; the kernel never names one.
- `inventory` is gone from the root workspace.
- Option B: one entry crate per capability under `crates/entry/<cap>/` with `http` and `cli` features.
- `systemprompt-extension` depends unconditionally on `pg_query`, `schemars`, `systemprompt-manifest`, `systemprompt-models`.
- `HostKind` is a closed enum of seven: `claude-code`, `claude-desktop`, `codex`, `codex-cli`, `opencode`, `opencode-cli`, `hermes`.
- `crates/app/agent-runs/src/host/` has a `HeadlessHost` seam with adapters `claude_code`, `codex_cli`, `opencode`, `hermes`.
- `ServicesConfig.external_agents` already keys per-host enablement by host id.
- The programme is not finished: stages 7b and 8 remain; contract amendments run past 185.

## 4. The template on `future`

- `src/main.rs` is `systemprompt_template::composition().run()`.
- Extensions are workspace crates: `extensions/web/{shared,admin,content,site,jobs}`, `extensions/mcp/{systemprompt,shared}`, `extensions/cli/kit-export`.
- Five flavours as cargo features with `flavours/<name>/{flavour.yaml,profile.overlay.yaml,services/,README.md}`: `gateway`, `mcp`, `agents`, `site`, `full`.
- MIT. A `bridge/` directory pins a core ref for the fork's bridge build.
- `scripts/core-patch.sh on|off` flips a local `[patch.crates-io]` per worktree.
