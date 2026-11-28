# Harness: future architecture and implementation stages

> **Not current functionality.** Everything under `documentation/future/` describes
> work that is designed but not built. Nothing here is shipped, and nothing here
> has been built or run. For what the platform does today, see
> [`documentation/`](../../README.md).

This directory is the technical specification and the staged implementation
sequence for making the desktop harness a first-class part of the platform and
retiring the bridge into it. It is written so that several people or coding
agents can work on it at the same time without colliding.

## The intended state

1. **Two tiers only: core and the extension.**
2. **Core** holds the wire contract, the device composition machinery and every governed device capability, in eight crates, each capability an opt-in feature. Core links no window toolkit.
3. **The extension** is the single build site for any device product: a one-line `main`, a composition, the window and views, brand, theme, packaging and the product's own members. It lives in the template (MIT) and is forked by installs, with the layout installs already use.
4. **The bridge is retired.** Its non-GUI code becomes core device crates; its window is rewritten as native views in the extension; the no-window composition keeps its product name. Core's release stops building, testing and signing a desktop binary.
5. **The standalone harness repository is dissolved**: engine and governance into core, shell into the template.
6. **One runtime, three typed execution modes.** The loop and its tools are implemented once in core and run `Unrestricted` or `Sandboxed` on a desktop, or `Remote` in the instance's sandbox. The instance is the record of every session in every mode, so every client sees them all.

## Layout

| Path | Purpose |
|---|---|
| [`stages/overview.md`](stages/overview.md) | **Start here to do work.** Stages, waves, the crate list, the dependency graph, what runs in parallel |
| [`stages/operating-model.md`](stages/operating-model.md) | How a lane works, what it owns, and the three validation levels |
| `stages/s0-…` to `stages/s9-…` | One file per stage: waves, lanes, owned paths, gates, exit criteria |
| [`testing/`](testing/) | The deterministic test strategy: scripted instance, golden scenarios, comparison against a reference agent, migrated test catalogue, evidence |
| [`architecture/`](architecture/) | The specification the stages implement |
| [`risks-and-decisions.md`](risks-and-decisions.md) | Risks with mitigations; every decision taken; the few still open |

### Architecture reference

| File | Answers |
|---|---|
| `01-current-state.md` | What exists today in the harness repository, the bridge, core `future` and the template |
| `02-target-architecture.md` | The two tiers, the eight crates and their layers, slices, every deployment form |
| `03-composition-contract.md` | The member trait, the sink, phases, standing, reserved shapes |
| `04-host-integrations.md` | Managing third-party coding agents; host identity and what a host-owned host is; `host_extensions`; orchestration |
| `05-bridge-split.md` | The bridge module by module: core, extension, or retired |
| `06-harness-repo-disposition.md` | The harness repository crate by crate |
| `07-template-extension.md` | `extensions/harness/*`; flavour; licence; what a fork owns |
| `08-platforms-and-packaging.md` | Windows, macOS, Linux; packaging and signing |
| `09-quality-and-gates.md` | Standards, gates, which core gates device crates meet and which are scoped, security review |
| `10-release-and-fleet-handover.md` | Release after the change; moving installed bridges |
| `11-execution-modes.md` | One loop in core; the `Unrestricted`, `Sandboxed` and `Remote` modes; the sandbox policy; the instance as the record of every session |
| `12-workbench-and-automation.md` | Parallel sessions with isolated working copies, the integrated terminal, the preview pane, scheduled sessions, memory |

Phone apps built from the same extension are specified in the sibling set, [`../mobile/`](../mobile/README.md).

## Founding decisions

| # | Decision |
|---|---|
| D1 | A harness is a type of extension. One extension point builds every device product |
| D2 | Two tiers only: core and the extension. No separate kernel repository |
| D3 | The bridge is deprecated and moved into the harness. Core stops building it |
| D4 | The bridge's code is split between real core crates on `future` and the extension |
| D5 | Bridge functions extend to other coding agents; the product grows into orchestration above the agents |
| D6 | One composition serves every form: governing individual agents, a custom harness, no harness, headless, locked-down machines. Slices are thin and opt-in |
| D7 | Standard views live in the template, forked, MIT, with the same architecture as existing installations |
| D8 | The harness repository is retired first, then the result is proven, then it is restructured |
| D9 | Testing is deterministic. No stage spends money on model inference. Parity is measured against an unmodified reference agent and against a catalogue of migrated tests |

D10 to D49 (crate list, publishing, contract placement, layout, naming, Linux, deprecation, evidence, signing, execution modes, and what S0 decided about the host, the lift and the reference pin) are in [`risks-and-decisions.md`](risks-and-decisions.md) section 3.

## Status

| Stage | State |
|---|---|
| S0 Foundations | Done. The eight crate skeletons, the closure gate, the scripted-instance interface, the template scaffold, both inventories and the lift's pin at `a214511` are on `future`. Host `harness` is on `future`; merging the host variant to `next` is a person's decision |
| S1 Lift | Code complete; stage gate green (core CI, Quality and Supply Chain at `78b09798e9`; the template's `Harness` workflow on Windows, macOS and Linux and its `Gates` at `9120a496`). On `future`: both imports at `a214511`; `systemprompt-engine`, `-device`, `-drive` and `-harness` compile on core's types and meet core's gates, with the module-order, no-window and foreign-name gates; 858 of the 877 lifted tests relocated (19 retired with reasons) and passing, on the native Windows and macOS job; the device reads a manifest tolerantly (unknown hosts and malformed ids skipped, everything else strict); `ExecutionSource::HookHarness` with mcp migration 022; the shell, views, theme, brand and shared crates in the template (one brand; the second exported as a patch), its tests a workspace of their own, 269 tests passing. Open: the shell is pinned to core `8bebe1607f`, apart from the template root's rev; RUSTSEC-2026-0459 (bundled SQLite in the engine's store) is ignored with a reason until rusqlite can move; archiving the harness repository and handing the second brand to its install are a person's acts |
| S2 Parity and proof | Wave A three of four lanes in: the scripted instance serves sign-in, a signed manifest, hooks, heartbeat, both model protocols, script asides (the title call), device enrolment, plugin bundle files, receipts, session bindings and the Drive sync feed (`crates/tests/scripted-instance`); the headless `run` and `login --stdin` exist as `systemprompt_harness::command` (library; the composition binary calls it); the 24 golden scenarios are data under `crates/tests/golden/scenarios` with an offline validator. In progress: the reference runner (`crates/tests/parity/reference`, S2-A4). Not started: wave B (golden runner, comparator, ledger, migration batches 1–3), wave C (tour, evidence script, desktop batch), the first full run on real Windows and macOS machines. Open for a person: `login` on Linux has no sealer to keep a token between processes; the scripted instance serves no installation plans (a plan request is recorded and refused). `run --control` takes prompt, steer, abort, revert and permission replies as one JSON object per line on standard input (O10) |
| S3 Execution environment and sandbox | Not started |
| S4 Remote runtime and the session record | Not started |
| S5 Device composition | Not started |
| S6 Bridge split and integration | Not started |
| S7 Platforms, packaging, signing | Not started |
| S8 Fleet handover and bridge removal | Not started |
| S9 Server half and orchestration | Not started |

Update this table in the commit that closes a stage.

### Who is working where (2026-11-28)

Two agent sessions push to `future` in both repositories and cannot message each other, so claims are recorded here. Read this before opening a lane; add a row before starting work on a path; fetch and rebase before every push; do not push while the other side's last push still has a hosted run in progress.

| Session | Holds | Does not touch |
|---|---|---|
| Harness lane (commits as Edward) | S2 waves B, C and D: `crates/tests/{golden,parity,migrated,scripted-instance-tests}/**`, the template's `extensions/harness/tests/{tour,migrated}/**` and `scripts/harness/evidence/**`, and the S2 rows of this table | S7 wave A paths below |
| `harness-s1-s9-delivery` (commits as Edward Burton) | S7 wave A: `platform` modules of `systemprompt-device`, `systemprompt-engine` and `systemprompt-machine`, and `scripts/lint-platform-confinement.py`. Earlier today: the scripted-instance asides, context window, enrolment, bundles, receipts and Drive feed, and `run --control` | Everything in the row above |

Open between the two: the template's `Gates` and `Harness` workflows fail on `lint-repo-hygiene` since `6a9dab38` (six `.txt` files under `extensions/harness/tests/tour` and `scripts/harness/evidence`); it is the harness lane's to fix. S3 is unclaimed: whoever takes it adds a row first.
