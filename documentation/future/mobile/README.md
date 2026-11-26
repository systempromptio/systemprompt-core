# Mobile: iOS and Android apps from the extension

> **Not current functionality.** Everything under `documentation/future/` describes
> work that is designed but not built. Nothing here is shipped, and nothing here
> has been built or run. For what the platform does today, see
> [`documentation/`](../../README.md).

This directory is the specification and the staged implementation sequence for
phone apps built from the same extension as the desktop harness
([`../harness/`](../harness/README.md)). It follows the same conventions: stages
split into waves of parallel lanes with owned paths, and costly validation once
per stage.

## The intended state

1. **The phone is a client of the instance.** It chats with the instance's agents, starts and watches server-side agent runs, approves or denies their tool requests, reads documents and receives notifications. It runs no agent loop and attaches to no desktop session.
2. **One shared Rust core, two native apps.** Sign-in, sync, typed calls, event streams and the session model are Rust, written once in core. The screens are SwiftUI on iOS and Jetpack Compose on Android, in the extension.
3. **Built in the extension.** `extensions/mobile/` in the template holds the iOS and Android projects; installs fork it beside their web, MCP and desktop extensions, and ship their own branded apps.
4. **Distribution without public listings.** Android: the instance serves a signed APK. iOS: private distribution to named organisations and a beta channel. Apple offers no route that avoids its sign-off.
5. **Each instance sends its own push notifications**, with its own credentials for its own app.

## Layout

| Path | Purpose |
|---|---|
| [`stages/overview.md`](stages/overview.md) | **Start here to do work.** Stages M0 to M5, dependencies on the harness stages, what runs in parallel |
| `stages/m0-…` to `stages/m5-…` | One file per stage: waves, lanes, owned paths, gates, exit criteria |
| [`architecture/`](architecture/) | The specification the stages implement |
| [`testing/`](testing/) | Deterministic strategy, scenarios and UI tests, evidence |
| [`risks-and-decisions.md`](risks-and-decisions.md) | Risks with mitigations; decisions taken and open; what was verified at source |

The operating model (lanes, hand-off notes, the three validation levels, decisions reserved to people) is the harness's: [`../harness/stages/operating-model.md`](../harness/stages/operating-model.md).

### Architecture reference

| File | Answers |
|---|---|
| `01-current-state.md` | What core already offers a phone, and what is missing |
| `02-target-architecture.md` | What the phone is; the layers; languages per repository; how phone, desktop, web and server link |
| `03-core-changes.md` | Every change to core, by crate |
| `04-shared-core-and-bindings.md` | The Rust core on a phone and how Swift and Kotlin call it |
| `05-native-apps.md` | `extensions/mobile/`: layout, screens, brand and theme, what a fork owns |
| `06-build-and-distribution.md` | Toolchains, the build graph, signing, installing with and without stores |
| `07-push-and-approvals.md` | The push channel and the approval step |
| `08-quality-and-gates.md` | Standards and gates for three languages |

## Decisions

| # | Decision |
|---|---|
| M1 | Scope: a client of server-side runs only |
| M2 | Shared Rust core with SwiftUI and Jetpack Compose |
| M3 | iOS through Apple Business Manager custom apps and TestFlight; Android as a self-hosted APK |
| M4 | Each instance sends its own push notifications |

Further decisions and their alternatives are in [`risks-and-decisions.md`](risks-and-decisions.md).

## Status

| Stage | State |
|---|---|
| M0 Foundations | Not started |
| M1 Core server | Not started |
| M2 Shared core | Not started |
| M3 Native apps | Not started |
| M4 Build and distribution | Not started |
| M5 Parity and proof | Not started |

Update this table in the commit that closes a stage.
