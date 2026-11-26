# M2. Shared core

**Goal.** One Rust library does everything a phone app needs, including the state machines every screen is drawn from, and the template builds it into a Swift package and a Kotlin package.

**Entry.** M0 closed; M1 wave A closed (the contract); harness S4 closed (`systemprompt-device` is in core, and the instance records every session).

**Specification.** `../architecture/04-shared-core-and-bindings.md`, `../architecture/03-core-changes.md` (C6 to C8).

## Wave A — three lanes, parallel

| Lane | Repository | Owned paths | Work |
|---|---|---|---|
| **M2-A1 Client calls** | core | `systemprompt-client/src/{agents,runs,approvals,streams,notifications,documents}/**` | Typed calls against the route constants. Stream consumption with resume from a cursor |
| **M2-A2 Platform modules** | core | `systemprompt-device/src/platform/{ios,android}/**` | Credential sealing in each platform keystore; app-container files; base directories; the cryptography provider installed at start-up |
| **M2-A3 Binding skeleton** | core | `systemprompt-mobile/src/{lib.rs,runtime,error,lifecycle}/**` | The runtime the exported async functions run on; `MobileError`; foreground and background; the composition of the `device` and `client` slices; the version check |

**Wave gate (L1).** Offline check of core for host and for the six phone targets.

## Wave B — four lanes, parallel

| Lane | Repository | Owned paths | Work |
|---|---|---|---|
| **M2-B1 Surface: identity and session** | core | `systemprompt-mobile/src/{instances,sign_in,session,appearance,diagnostics}/**` | The exported objects for discovery, the three sign-in paths, the session, brand and theme as data |
| **M2-B2 Surface: features** | core | `systemprompt-mobile/src/{chat,runs,approvals,inbox,documents,preferences}/**` | The state machines exported across the boundary, each behind the same feature, plus the list of admitted features the apps build navigation from |
| **M2-B4 Client state machines** | core | `systemprompt-harness/src/client/{sign_in,session,approvals,inbox,documents,preferences}/**` | One state machine per feature slice (`../architecture/04-shared-core-and-bindings.md` section 7): domain state, change callback, actions, stream ownership, explicit loading, empty, error, stale and waiting states. Each behind its own facade feature. No grouping, formatting, field selection or layout |
| **M2-B3 Template binding crate, build and drift gate** | template | `extensions/mobile/core/**` (the crate that builds the final library from `systemprompt-mobile` plus the fork's own annotated members); `extensions/mobile/shared/**`; the Swift package and Gradle module that wrap the generated code; a drift job in the `mobile` workflow | Scripts that build the library per target, run the generator, assemble the XCFramework and the AAR. The gate that rebuilds bindings from the pinned core and compiles both apps against them |

**Wave gate (L1).** Offline check of core; the template scripts run to the point of invoking the toolchains.

## Wave C — review, one lane

| Lane | Reviews |
|---|---|
| **M2-C1** **(review)** | The binding surface: no token, credential or key type is exported; no exported call returns one; errors carry no secret |

## Stage gate (L2)

- Core's push run, including tests of the client calls and platform modules against the scripted instance, and the phone-target check.
- Template `mobile` workflow: the library builds for every target; the generated Swift and Kotlin compile into the blank apps; a smoke test in each blank app calls one exported function against the scripted instance on the simulator and the emulator.

## Exit criteria

1. Stage gate green.
2. A blank app on each platform signs in to the scripted instance and renders a session from its stream, using only generated code.
3. A build that names only `device` and `runs` links no other feature's code (the closure gate proves it).
4. The surface review is closed.

## Decisions reserved to people

- None beyond merging into `future`.
