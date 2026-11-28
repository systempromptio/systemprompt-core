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
| S1 Lift | Code complete; stage gate green (core CI, Quality and Supply Chain at `78b09798e9`; the template's `Harness` workflow on Windows, macOS and Linux and its `Gates` at `9120a496`), and the Windows rows now also proven on a real Windows 11 machine: toolchain, shell tests, the installer packed and test-installed. On `future`: both imports at `a214511`; `systemprompt-engine`, `-device`, `-drive` and `-harness` compile on core's types and meet core's gates, with the module-order, no-window and foreign-name gates; 858 of the 877 lifted tests relocated (19 retired with reasons) and passing, on the native Windows and macOS job; the device reads a manifest tolerantly (unknown hosts and malformed ids skipped, everything else strict); `ExecutionSource::HookHarness` with mcp migration 022; the shell, views, theme, brand and shared crates in the template (one brand; the second exported as a patch), its tests a workspace of their own, 269 tests passing at the lift and 532 at template `c667ebad` (531 on Windows, 532 on macOS and Linux, none failing, in the template's `Harness` workflow, green on all three systems with `Gates` at that commit). The shell takes its single-instance lock and hand-off from core: on Linux three second starts carrying a link each exited 0 within 50 ms, left one window and navigated it; its two Windows test fixes pass on the hosted runner, and the first was also confirmed on the Windows 11 machine. macOS has run on hosted runners only. Open: RUSTSEC-2026-0459 (bundled SQLite in the engine's store) is ignored with a reason until rusqlite can move; archiving the harness repository and handing the second brand to its install are a person's acts |
| S2 Parity and proof | In progress, waves A to D done; not closed. On `future`: the scripted instance; the headless `run` and `login --stdin`; 24 golden scenarios; the comparator and parity ledger, whose side-by-side command reports 0 unclassified differences; the reference runner at the pinned tag `v2.4.2`; migration batches 1 to 3 in core and batch 4 in the template, each with a per-case `cases.md`, counted by `scripts/parity/count-cases.py` and published in `testing/04` section 1.2 (6,422 case rows: 1,222 passing, 2,403 in the ledger, 477 tour states; none `open`, `written` or `manual`, so `count-cases.py --check` exits 0, with the template's `app` and `panels` batches at no manual row). A full side-by-side run at `37968ba2ae` has all 48 harness runs the same as their expected traces and, against the reference, 8 runs the same, 34 with intended differences only, 2 with a gap (background subagents, owned by S9-C2) and 4 skipped as harness-only; no classification is marked unsure. The template's `just harness-e2e` runs the headless harness against a real template server whose model upstream is the scripted instance; its five assertions (run completes, one request row per model call, the tool call recorded as `hook_harness`, a denial shown and audited, every model call on loopback) passed in 10 runs of 10 against core `095d3bf07e`. The evidence command has run end to end on a real Windows 11 machine: toolchain, shell tests, installer packed and test-installed, the 24 golden scenarios, the device-native suite and the screenshot tour, with no inference spend; its captures are on a review page (https://claude.ai/artifact/RZ5Cf2Xn6fV5EbK2NtRgZk: 29 Windows captures, 35 of 64 states on the manual walk). The migrated tests found 17 engine defects, now fixed (8 in the shell scanner, 9 in the engine); six more permission and data-loss defects found by triage are being fixed. Open against the stage gate: those six fixes; core `Quality` at `37968ba2ae`, red on one source gate (`lint-fail-open`, `crates/domain/engine/src/store/handle/fork.rs`) while `CI`, `Supply Chain` and `Parity` are green; every tour state captured on Windows and on a real macOS machine (macOS has run on hosted runners only, with no evidence run); a person looking at the screenshots, which nobody has yet |
| S3 Execution environment and sandbox | Not started |
| S4 Remote runtime and the session record | Not started |
| S5 Device composition | Not started |
| S6 Bridge split and integration | Not started |
| S7 Platforms, packaging, signing | Wave A in, opened early on disjoint paths: every target-system condition of `systemprompt-device`, `-engine`, `-machine` and `-harness` sits in a `platform` module behind one contract with Windows, macOS and Linux implementations (`scripts/lint-platform-confinement.py`, on the gate list); the Linux sealer keeps a sign-in between processes in the kernel keyring (O9), the Secret Service store is the `secret-service` feature; one instance per data root hands a second launch's arguments to the first (a named pipe on Windows, a Unix socket elsewhere). The Windows installer has been packed and test-installed on a real Windows 11 machine in the S2 evidence run; macOS packaging has run on hosted runners only. Not started: wave B (Linux headless, macOS shell, Linux shell packaging), wave C (after S6) |
| S8 Fleet handover and bridge removal | Not started |
| S9 Server half and orchestration | Not started |

Update this table in the commit that closes a stage.
