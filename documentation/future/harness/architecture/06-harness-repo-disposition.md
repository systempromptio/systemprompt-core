# 06. Disposition of `systemprompt-harness`

**Short answer to "does it get integrated into the template?": about a third of it.** The shell, UI, brand and the feature screens go to the template. The rest goes to core as crates; of that, the device-client code that repeats the bridge's is replaced by the bridge's in S6. The repository is then archived.

Ref: `a214511`, the commit the lift pins. 30 production crates, 94,469 production lines; 31 test packages, 32,316 test lines, 1,154 test attributes. The lift manifest (lane S0-A4) and its delta (lane S0-C2) give the destination of every file.

## 1. Summary

| Destination | Lines | What |
|---|---|---|
| Core device crates | about 61,000 | The agent engine (33,383), the device client (`secure`, `client`, `accounts`, `protocol`), the governor, projection, sync, the extension contract and its members, Drive sync |
| Template `extensions/harness/*` (MIT) | about 33,000 | Window, views, the feature screens, UI components, themes, strings, brand, the shell contract, packaging |
| Deleted in S1 | 489 | `harness-tasks`: replaced by the owned-task types in `systemprompt-traits` |
| Replaced in S6 (inside the core figure) | about 10,000 | `secure`, `client`, `accounts`, `sync`: the bridge's code takes over, less what the bridge lacks (§3) |

## 2. Crate by crate

| Crate | Lines | Destination | Detail |
|---|---|---|---|
| `harness-ids` | 411 | Core `systemprompt-identifiers`, submodule `engine` | `SessionId`, `MessageId`, `CallId` and `ExtensionId` already exist in `systemprompt-identifiers` with other validation. The engine's ids live in the `engine` submodule, so both sets stay distinct by path |
| `harness-tasks` | 489 | Deleted | `systemprompt_traits::{BackgroundTasks, OwnedTask}`. What callers use and the traits lack (blocking spawns, `OwnedTask::stop`, a child token, a shutdown report, a task error) is added to `systemprompt-traits` |
| `harness-config` | 2,989 | Core `systemprompt-engine`, module `config` | Paths are injected; nothing reads another product's directories |
| `harness-store` | 2,559 | Core `systemprompt-engine`, module `store` | SQLite via `rusqlite` (bundled). Core is otherwise Postgres-only; this store is device-local and is not a `sqlx` user. `rusqlite` is pinned to the release whose `libsqlite3-sys` equals the one core's root lockfile holds (0.39 for `libsqlite3-sys` 0.37), because one workspace resolves one version of a `links` crate |
| `harness-llm` | 2,551 | Core `systemprompt-engine`, module `llm`; onto core's wire codecs in S5 | Transport over `systemprompt-wire` codecs (`anthropic`, `openai_chat`, `sse`). Verify the codecs cover the client direction before deleting |
| `harness-tools` | 6,591 | Core `systemprompt-engine`, module `tools` | Each built-in tool becomes a contribution and `builtin()` goes, in S5 |
| `harness-mcp` | 3,249 | Core `systemprompt-engine`, module `mcp` | Align types with `rmcp` and `systemprompt-models` `mcp` |
| `harness-workspace` | 3,258 | Core `systemprompt-engine`, module `workspace` | |
| `harness-lsp` | 3,423 | Core `systemprompt-engine`, module `lsp` | |
| `harness-prompt` | 1,033 | Core `systemprompt-engine`, module `prompt` | Prompts are original text and stay so |
| `harness-engine` | 7,319 | Core `systemprompt-engine`, module `run` | `Governor` trait is the reserved shape; `NullGovernor` moves to the test workspace. The engine extension contract stays at `run/extension` until S5 replaces it |
| `sp-protocol` | 3,406 | Split | Wire types into `systemprompt-models` (`bridge`, `documents`, `brain`, `notifications`) where not already there; names, origin, secret and error types into `systemprompt-device` `protocol`; canonical JSON and version order into `device` and deleted for the bridge's in S6. The view types (`view.rs`) stay in `systemprompt-device`: `accounts` and `sync` produce them, so core must be able to name them |
| `sp-secure` | 2,111 | Core `systemprompt-device` in S1; deleted in S6 | Bridge `fsutil`, `windows_acl`, keyring stores on three systems |
| `sp-client` | 2,049 | Core `systemprompt-device` in S1; deleted in S6 | Bridge `gateway/` client, moved to `systemprompt-client` |
| `sp-accounts` | 2,644 | Core `systemprompt-device` in S1; mostly deleted in S6 | Bridge `auth/`. **Keep what the bridge lacks:** several instances with one active, and "keep me signed in" off meaning memory only. These land in `device` as additions with their tests |
| `sp-sync` | 3,227 | Core `systemprompt-harness`, module `sync`, in S1; mostly deleted in S6 | It depends on governance, projection and the extension contract, and `systemprompt-device` (infra) may not depend on `systemprompt-harness` (app). Bridge `sync/`. **Keep what the bridge lacks:** the focused and unfocused poll cadence and the status surface the window reads |
| `sp-projection` | 2,267 | Core `systemprompt-harness`, module `projection` | It is the native engine's equivalent of a `HostSync` emitter and should implement that contract |
| `sp-governance` | 1,651 | Core `systemprompt-harness`, module `governance` | The reserved `Governor` |
| `sp-extension` | 791 | Core `systemprompt-harness`, module `extension` | The instance-side member contract, lifted as it is; S5 replaces it with the composition contract. Its link-time registration through `inventory` does not enter core: an explicit named registration list replaces discovery in S1, and `lint-no-inventory` is unchanged |
| `sp-drive` sync half (`client`, `http`, `sync`, `error.rs`, `hash.rs`, `upload.rs`, `lib.rs`) | 4,735 | Core `systemprompt-drive`; merged with the bridge's in S6 | One sync implementation |
| `sp-drive` engine half (`extension/**`, `agent.rs`) | 1,199 | Core `systemprompt-harness`, `members/drive` | It depends on the engine and the extension contract, which a domain crate may not; the agent tools become contributions in S5 |
| `sp-drive` `view.rs` | 212 | Template `extensions/harness/views` | Its only production user is the Drive screens |
| `sp-brain` | 2,118 | Core `systemprompt-harness`, `members/brain` | It adds headers to governed model calls, so it carries trust. Core's `crates/domain/brain` is a server crate the closure gate refuses to a device crate; the wire types are in `systemprompt-models` |
| `sp-notifications` | 1,116 | Core `systemprompt-harness`, `members/notifications` | |
| `sp-slack` | 338 | Core `systemprompt-harness`, `members/slack` | A filter over the notification inbox; follows `notifications` |
| `sp-brand` | 346 | Template `extensions/harness/brand` in S1; deleted in S5 | `BrandDecl`; the second brand's declaration moves to the install that owns it |
| `harness-ui` | 5,994 | Template `extensions/harness/shared` and `theme` | The second brand's palette moves to the install that owns it |
| `harness-shell` | 440 | Template `extensions/harness/shared/src/contract` | The shell member contract. The views and the shell binary both need it, so it sits below both. Its link-time registration becomes a named list with the instance-side half |
| `harness-drive-ui`, `harness-brain-ui`, `harness-notifications-ui` | 3,651; 2,706; 1,158 | Template `extensions/harness/views/src/{drive,brain,notifications}` | |
| `harness-desktop` `app/` | 2,297 | Template `shell` in S1; split in S5 | In S5 composition and services are deleted for the device composition; window and deep link glue stay in `extensions/harness/shell` |
| `harness-desktop` `state/` | 2,026 | Template `shell` | |
| `harness-desktop` `strings/` | 971 | Template `shared` as the default strings bundle | |
| `harness-desktop` `views/` | 12,892 | Template `views` | composer, docks, frame, governance, pickers, settings, signin, transcript, and ten top-level files |
| `harness-desktop` crate root, build script, resources | 252 | Template `shell` | |
| `packaging/windows`, `packaging/macos` | n/a | Template `packaging/harness/{windows,macos}` | Each repeats the brand table; both read `BrandDecl` through `package --describe` once it exists |
| `scripts/` (29 gates) | n/a | Deleted | Core's gates apply to the core-bound code; the install lint applies to the template. Three gates merge into core's (the lift manifest lists them) |
| `crates/tests/*` (30 packages) | 31,444 | Follow their crate | 23 to core's test workspace (26,095 lines, 877 attributes); 7 to the template's `tests/` (5,349 lines, 269 attributes) |
| `crates/tests/harness-live-tests` | 872 | Not lifted; re-expressed in S2 over the scripted instance | An opt-in package that signs in to a real instance named by environment variables; one of its 8 tests makes a real model call. The programme runs no real inference, so S1 imports none of it and S2 rewrites its eight behaviours as scripted-instance tests |
| Its handover document | n/a | Superseded by this directory | |

## 3. Where the two codebases disagree

The harness and the bridge implement the same protocol twice. The bridge's implementation survives because it is in production. Before deleting a lifted harness module, diff its behaviour against the bridge and record each difference as one of three outcomes:

| Outcome | Example to check |
|---|---|
| Bridge is right; harness behaviour dropped | Wire format and file bytes: the harness's own handover says the bridge wins |
| Harness has something the bridge lacks; port it to `device` | Several instances, memory-only sign-in, poll cadence by window focus, the 80 % JWT renewal with 30 s retry |
| They differ and neither is clearly right; decide and write it down | Tool verdict budget (harness: 3 s, fail open); data-root layout (`%LOCALAPPDATA%\<vendor>\<product>[ channel]` versus the bridge's `<config_dir>/<brand>/`) |

This diff is the main deliverable of stage 2's first lane and the content of `D2` in the overview's document set.

## 4. Licence

The harness is BUSL-1.1 throughout. What moves to core stays BUSL-1.1. What moves to the template (the UI, desktop and brand crates) is relicensed MIT as it enters. There is no interim in which governed code sits in the template. The relicensing is a maintainer decision recorded in the template's changelog.

## 5. Naming

The harness's rule that nothing is "vendored, copied or named after" another project carries into both destinations: `lint-foreign-names` and its list move to core for the device crates and to the install lint for the template.

## 6. Order of dissolution

One stage (`../stages/s1-lift.md`).

1. Two import commits, one per repository, moves only, each recording `a214511`: engine, device, governance, extension-member and Drive sources into core; UI, desktop, feature screens and brand into the template.
2. In core, crate boundaries become module boundaries inside the four crates; the model types that duplicate core's are deleted for core's.
3. The template's shell is pointed at core's crates.
4. Both repositories build and pass their tests on Windows, macOS and Linux against core `future`.
5. The repository is archived. Its history stays readable; nothing new lands.

The device-client code that duplicates the bridge's arrives in core with everything else and is replaced by the bridge's in S6.
