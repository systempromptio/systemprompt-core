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
2. **Core** holds the wire contract, the device composition machinery and every governed device capability, each an opt-in feature. Core links no window toolkit.
3. **The extension** is the single build site for any device product: a one-line `main`, a composition, the window and views, brand, theme, packaging and the product's own members. It lives in the template (MIT) and is forked by installs, with the layout installs already use.
4. **The bridge is retired.** Its non-GUI code becomes core device crates; its window is rewritten as native views in the extension. Core's release stops building, testing and signing a desktop binary.
5. **The standalone harness repository is dissolved** into the template first, then its governed parts are promoted into core.

## Layout

| Path | Purpose |
|---|---|
| [`stages/overview.md`](stages/overview.md) | **Start here to do work.** Stages, waves, the dependency graph, what runs in parallel |
| [`stages/operating-model.md`](stages/operating-model.md) | How a lane works, what it owns, and the three validation levels |
| `stages/s0-…` to `stages/s8-…` | One file per stage: waves, lanes, owned paths, gates, exit criteria |
| [`testing/`](testing/) | The deterministic test strategy: scripted instance, golden scenarios, comparison against a reference agent, migrated test catalogue, evidence |
| [`architecture/`](architecture/) | The specification the stages implement |
| [`risks-and-decisions.md`](risks-and-decisions.md) | Risks with mitigations; decisions taken and open |

### Architecture reference

| File | Answers |
|---|---|
| `01-current-state.md` | What exists today in the harness repository, the bridge, core `future` and the template |
| `02-target-architecture.md` | The two tiers, where device crates sit in core's layers, slices, every deployment form |
| `03-composition-contract.md` | The member trait, the sink, phases, standing, reserved shapes |
| `04-host-integrations.md` | Managing third-party coding agents; open host identity; orchestration |
| `05-bridge-split.md` | The bridge module by module: core, extension, or retired |
| `06-harness-repo-disposition.md` | The harness repository crate by crate |
| `07-template-extension.md` | `extensions/harness/*`; flavour; licence; what a fork owns |
| `08-platforms-and-packaging.md` | Windows, macOS, Linux; packaging and signing |
| `09-quality-and-gates.md` | Standards, gates, security review |
| `10-release-and-fleet-handover.md` | Release after the change; moving installed bridges |

## Decisions taken

| # | Decision |
|---|---|
| D1 | A harness is a type of extension. One extension point builds every device product |
| D2 | Two tiers only: core and the extension. No separate kernel repository |
| D3 | The bridge is deprecated and moved into the harness. Core stops building it |
| D4 | The bridge's code is split between real core crates on `future` and the extension |
| D5 | Bridge functions extend to other coding agents; the product grows into orchestration above the agents |
| D6 | One composition serves every form: governing individual agents, a custom harness, no harness, headless, locked-down machines. Slices are thin and opt-in |
| D7 | Standard views live in the template, forked, MIT, with the same architecture as existing installations |
| D8 | The harness repository is retired first: lifted whole into the template, built and tested on Windows and macOS against core `future`, then restructured |
| D9 | Testing is deterministic. No stage spends money on model inference. Parity is measured against an unmodified reference agent and against a catalogue of migrated tests |

## Status

| Stage | State |
|---|---|
| S0 Foundations | Not started |
| S1 Lift into the template | Not started |
| S2 Parity and proof | Not started |
| S3 Device composition | Not started |
| S4 Promote the trust core | Not started |
| S5 Bridge split and integration | Not started |
| S6 Platforms, packaging, signing | Not started |
| S7 Fleet handover and bridge removal | Not started |
| S8 Server half and orchestration | Not started |

Update this table in the commit that closes a stage.
