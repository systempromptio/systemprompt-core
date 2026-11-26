# Stages: overview

Six stages, M0 to M5. Each is a set of waves; each wave is a set of lanes that run at the same time on disjoint paths. Roles, lane briefs, hand-off notes, the three validation levels and the decisions reserved to people are those of the harness: [`../../harness/stages/operating-model.md`](../../harness/stages/operating-model.md).

## 1. The stages

| Stage | Outcome | Repositories |
|---|---|---|
| **M0 Foundations** | Distribution facts confirmed at source. `systemprompt-mobile` exists as an empty, gated crate. The shared crates check for phone targets. The template has `extensions/mobile/` with empty Xcode and Gradle projects and workflows on Linux and macOS runners | core, template |
| **M1 Core server** | Push channel; device records and push tokens; runs list and the approval step; association files; discovery and download entries. Each reviewed where it touches authorization | core |
| **M2 Shared core** | Typed client calls; iOS and Android platform modules; the client state machines per opt-in feature, shared with the desktop; the binding layer; generated Swift and Kotlin packages building in the template; the drift gate | core, template |
| **M3 Native apps** | Every feature slice drawn on both platforms from the shared client state machines; navigation built from the admitted features | template |
| **M4 Build and distribution** | A signed APK served by the instance; iOS builds on the beta channel and offered as a custom app; release workflows; managed configuration | template, core (download route), installs |
| **M5 Parity and proof** | Scenario suite and UI tests on both platforms; the screenshot tour; the evidence run; the manual walk on real phones | template, core |

## 2. What depends on what

```
harness S0 ─► M0 ─► M2 ─► M3 ─► M4
                     ▲      │
            M1 ──────┘      └─► M5
   (M1 needs nothing from the harness and can start at once)

harness S1 (device crate in core) ─► M2
harness S2 (scripted instance)    ─► M5
harness S5 (brand and theme data) ─► M3 wave C
harness S4 (remote runtime and the session record) ─► M1 lane B3, M2
```

| Stage | Needs | May run alongside |
|---|---|---|
| M0 | Harness S0 (crate skeletons, template scaffold conventions) | M1 |
| M1 | Nothing for push, devices and association files; harness S4 for the sessions lane | M0, any harness stage |
| M2 | M0; M1's contract wave; harness S4 | M1's later waves |
| M3 | M2 | M4 wave A, M5 wave A |
| M4 | M3 for signed apps; people for accounts | M5 |
| M5 | M3; harness S2 | M4 |

**Earliest useful point:** M1's push channel, device records and association files need nothing from the harness and can start at once. Its sessions lane and everything after depend on the harness's S4, which is where the runtime's `Remote` mode, the session record, the stream and approvals are built for all clients.

## 3. Parallelism at a glance

Before the cap of four open lanes.

| Stage | Wave | Parallel lanes | Notes |
|---|---|---|---|
| M0 | A | 4 | Source check (read-only); crate skeleton and gates; template scaffold; workflows |
| M1 | A | 1 | Contract: route shapes and types for all five changes, alone |
| M1 | B | 4 | Push channel; device records and tokens; runs list and approval; association files with discovery |
| M1 | C | 3 | Independent reviews |
| M2 | A | 3 | Client calls; platform modules; binding layer skeleton |
| M2 | B | 4 | Client state machines; binding surface by group; template build scripts and drift gate |
| M3 | A | 2 | App shells: navigation, design components, sign-in (one lane per platform) |
| M3 | B | 6 | Three features per platform |
| M3 | C | 5 | Two more features per platform; brand and theme generation |
| M4 | A | 3 | Android signing and self-host; iOS signing and upload; managed configuration |
| M5 | A | 3 | Scenarios; iOS UI tests and tour; Android UI tests and tour |

The two platforms never share a file, so every M3 wave splits cleanly down the middle.

## 4. Sizing

Estimates, in lines of code, to show proportion. Nothing has been measured.

| Area | Language | Estimate |
|---|---|---|
| Server changes (M1): push, devices, association files, notifications on approvals, `Remote` sessions from a phone. The runtime and session record are the harness's S4 | Rust | 5,000 |
| Client calls, platform modules, client state machines, binding layer (M2) | Rust | 8,000 |
| Core tests for the above | Rust | 8,000 |
| iOS app (M3), including dictation, camera and attachments | Swift | 9,000 |
| Android app (M3), including dictation, camera and attachments | Kotlin | 9,000 |
| UI tests and tour (M5) | Swift, Kotlin | 4,000 |
| Build, signing and release scripts (M0, M4) | Shell, workflow | 1,500 |
| **Total** | | **about 44,000** |

For scale: the harness repository is about 82,000 lines and the bridge about 75,000. The phone work is roughly a quarter of the two together. The runtime, its `Remote` mode, the session record and approvals are built once in the harness's S4 for every client, which is why the mobile server share is small.

## 5. Rules

The harness's rules apply unchanged: a lane owns paths; shared files belong to the integrator; contract before consumers; moves and edits never share a commit; no expensive validation inside a lane; a stage closes only on its exit criteria.

One addition: **every feature is two lanes, one per platform, opened together from one brief** that names the feature's state machine, the domain states it reports and its actions, and lists the screens and their states. The brief and the shared screen-and-state list are the parity contract.
