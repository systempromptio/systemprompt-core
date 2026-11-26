# 06. Disposition of `systemprompt-harness`

**Short answer to "does it get integrated into the template?": about a third of it.** The shell, UI and brand go to the template. About half goes to core as crates. The rest is deleted because the bridge already has production code for the same job. The repository is then archived.

Ref: `0839a31`. 81,661 production lines, 24,717 test lines.

## 1. Summary

| Destination | Lines | What |
|---|---|---|
| Core device crates | about 41,000 | The agent engine, the governor, projection, Drive sync |
| Template `extensions/harness/*` (MIT) | about 27,000 | Window, views, UI components, themes, strings, brand, packaging |
| Deleted | about 14,000 | Instance client, accounts, sealing, sync, ids, tasks, brand enum: replaced by the bridge's code or by existing core primitives |

## 2. Crate by crate

| Crate | Lines | Destination | Detail |
|---|---|---|---|
| `harness-ids` | 277 | Core, merged | Into `systemprompt-identifiers` |
| `harness-tasks` | 489 | Deleted | `systemprompt_traits::{BackgroundTasks, OwnedTask}` |
| `harness-config` | 2,978 | Core `systemprompt-engine`, module `config` | Paths are injected; nothing reads another product's directories |
| `harness-store` | 2,559 | Core `systemprompt-engine`, a module | SQLite via `rusqlite` (bundled). Core is otherwise Postgres-only; this store is device-local and is not a `sqlx` user |
| `harness-llm` | 2,488 | Core `systemprompt-engine`, module `llm`; onto core's wire codecs in S5 | Transport over `systemprompt-wire` codecs (`anthropic`, `openai_chat`, `sse`). Verify the codecs cover the client direction before deleting |
| `harness-tools` | 6,591 | Core `systemprompt-engine`, a module | Each built-in tool becomes a contribution; `builtin()` goes |
| `harness-mcp` | 3,249 | Core `systemprompt-engine`, a module | Align types with `rmcp` and `systemprompt-models` `mcp` |
| `harness-workspace` | 3,258 | Core `systemprompt-engine`, a module | |
| `harness-lsp` | 3,423 | Core `systemprompt-engine`, a module | |
| `harness-prompt` | 1,012 | Core `systemprompt-engine`, module `prompt` | Prompts are original text and stay so |
| `harness-engine` | 6,289 | Core `systemprompt-engine`, a module | `Governor` trait is the reserved shape; `NullGovernor` moves to the test workspace |
| `sp-protocol` | 3,170 | Split | Wire types into `systemprompt-models::bridge` where the bridge's are not already there; canonical JSON and version order deleted for the bridge's; view types to the template |
| `sp-secure` | 1,643 | Core `systemprompt-device` in S1; deleted in S6 | Bridge `fsutil`, `windows_acl`, keyring stores on three systems |
| `sp-client` | 2,030 | Core `systemprompt-device` in S1; deleted in S6 | Bridge `gateway/` client, moved to `systemprompt-client` |
| `sp-accounts` | 2,645 | Core `systemprompt-device` in S1; mostly deleted in S6 | Bridge `auth/`. **Keep what the bridge lacks:** several instances with one active, and "keep me signed in" off meaning memory only. These land in `device` as additions with their tests |
| `sp-sync` | 3,185 | Core `systemprompt-device` in S1; mostly deleted in S6 | Bridge `sync/`. **Keep what the bridge lacks:** the focused and unfocused poll cadence and the status surface the window reads |
| `sp-projection` | 2,267 | Core `systemprompt-harness` | It is the native engine's equivalent of a `HostSync` emitter and should implement that contract |
| `sp-governance` | 1,483 | Core `systemprompt-harness` | The reserved `Governor` |
| `sp-drive` | 5,249 | Core `systemprompt-drive`; merged with the bridge's in S6 | One sync implementation; agent tools become contributions; views to the template |
| `sp-brand` | 346 | Deleted | `BrandDecl`; the second brand's declaration moves to the install that owns it |
| `harness-ui` | 5,990 | Template `extensions/harness/shared` and `theme` | The second brand's palette moves to the install that owns it |
| `harness-desktop` `app/` | 2,488 | Split | Composition and services deleted for the device composition; window, deep link glue to `extensions/harness/shell` |
| `harness-desktop` `state/` | 1,821 | Template `shell` | |
| `harness-desktop` `strings/` | 1,211 | Template `shared` as the default strings bundle | |
| `harness-desktop` `views/` | 15,292 | Template `views` | composer, docks, drive, frame, governance, pickers, settings, signin, transcript |
| `packaging/windows` | n/a | Template `packaging/` | Reads `BrandDecl` through `package --describe` |
| `scripts/` (29 gates) | n/a | Deleted | Core's gates apply to the core-bound code; the install lint applies to the template |
| `crates/tests/*` | 24,717 | Follow their crate | Core's test workspace or the template's `tests/` |
| Its handover document | n/a | Superseded by this directory | |

## 3. Where the two codebases disagree

The harness and the bridge implement the same protocol twice. The bridge's implementation survives because it is in production. Before deleting a harness crate, diff its behaviour against the bridge and record each difference as one of three outcomes:

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

1. Two import commits, one per repository, moves only: engine, governance and Drive sources into core; UI, desktop and brand into the template.
2. In core, crate boundaries become module boundaries inside the four crates; the model types that duplicate core's are deleted for core's.
3. The template's shell is pointed at core's crates.
4. Both repositories build and pass their tests on Windows, macOS and Linux against core `future`.
5. The repository is archived. Its history stays readable; nothing new lands.

The device-client code that duplicates the bridge's arrives in core with everything else and is replaced by the bridge's in S6.
