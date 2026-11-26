# S5. Bridge split and integration

**Goal.** The bridge's non-GUI code is core device crates; its registries are members; the duplicate device code from the harness is gone; the template builds the agents-only form with native views. The bridge binary still exists and still ships.

**Entry.** S4 closed.

**Specification.** `../architecture/05-bridge-split.md`, `../architecture/04-host-integrations.md`.

**Principle.** Split in place. Each module moves out of the bridge into a core crate, unchanged, with its tests, while the bridge keeps building on it. One bridge release ships that way before anything is merged or deleted.

## Wave A — fixtures and diff, three lanes, parallel

| Lane | Repository | Owned paths | Work |
|---|---|---|---|
| **S5-A1 Conformance fixtures** | core | A new conformance area in the test workspace | Record from the unmodified bridge every contract in `architecture/05-bridge-split.md` section 4: manifests and signatures, acceptance cases, scoped tokens, heartbeat bodies, each host's written profile, command exit codes |
| **S5-A2 Behaviour diff** (read-only) | core | None | The promoted device crates against the bridge's modules: each difference classified as bridge wins, port to the bridge's code, or decide (`architecture/06-harness-repo-disposition.md` section 3) |
| **S5-A3 Host scenarios** | template | `tests/golden/scenarios/hosts/**` | Golden scenarios for each managed host: profile written, drift repaired, profile removed, credential helper answering |

**Wave gate (L1).** Offline check of the fixture crate; fixtures green against the unmodified bridge is checked at the early L2 below.

**Early L2.** One run to prove the fixtures pass against the bridge before any module moves.

## Wave B — carve the lower layers, three lanes, parallel

Order follows the bridge's own layer gate, bottom-up.

| Lane | Bridge modules | Destination |
|---|---|---|
| **S5-B1** | `ids`, `headers`, `hash`, `gateway` types and client, `mcp_registry` | Wire module, identifiers, client |
| **S5-B2** | `fsutil`, `basedirs`, `config`, `auth`, `windows_acl` | Device crates; platform modules |
| **S5-B3** | `sync`, `feedback`, `feedback_capture`, `last_sync`, `instance`, `activity`, `verdict` | Device crates |

**Wave gate (L1).** Offline check of core and of the bridge workspace.

## Wave C — carve the upper layers, three lanes, parallel

| Lane | Bridge modules | Destination |
|---|---|---|
| **S5-C1** | `proxy`, `proxy_probe` | `proxy` |
| **S5-C2** | `install`, `elevation_gate`, `schedule`, `drift`, `validate`, `diagnostics_state`, `update`, `drive` | `machine`, `update`, `drive` |
| **S5-C3** | `integration`, `host_sync` | The host contract, and one crate per product |

**Wave gate (L1).** As wave B.

**Early L2, and a release.** The bridge ships a release built on the carved crates. This is the proof the cut is right. Reserved to people.

## Wave D — reviews, parallel, read-only

| Lane | Reviews |
|---|---|
| **S5-D1** **(review)** | Credentials and trust: `auth`, `config`, manifest verification, acceptance |
| **S5-D2** **(review)** | Elevated code: the policy writer, elevation, ACLs |
| **S5-D3** **(review)** | The proxy: credential classes, scoped tokens, route acceptance |

## Wave E — unify, three lanes, parallel

| Lane | Repository | Work |
|---|---|---|
| **S5-E1 One device client** | core | Apply the behaviour diff. Port what the bridge's code lacked (several instances, memory-only sign-in, focus-aware polling). Delete the promoted duplicates |
| **S5-E2 Registries to members** | core | The four link-time registries become slots; each host is a member with its facets; each capability a slice; commands declared with reserved paths and exit codes; the bridge's composition root replaced by built state |
| **S5-E3 Extension hosts** | core | The typed host id; instance-declared hosts; refusal of undeclared ids on the server; withholding on the device |

Followed by **S5-E4 (review)** of E3.

## Wave F — the extension, three lanes, parallel

| Lane | Repository | Owned paths | Work |
|---|---|---|---|
| **S5-F1 Hosts and marketplace views** | template | `extensions/harness/views/hosts/**`, `…/marketplace/**` | Native views over the device state, to parity with the bridge's recorded fixture states |
| **S5-F2 First run, inbox, tray** | template | `extensions/harness/views/first_run/**`, `…/inbox/**`, `extensions/harness/shell/tray/**` | The same; background presence with the window closed |
| **S5-F3 Agents-only composition** | template | The composition and flavour files (through the integrator) | The `agents-only` named composition; every windowless and tray form in `architecture/02-target-architecture.md` section 5 as a named composition |

## Stage gate (L2)

- Conformance fixtures green against the bridge binary **and** against the template-built agents-only binary.
- Every named composition builds and boots against the scripted instance.
- Host scenarios green; the new views on the screenshot tour on Windows and macOS.
- Everything from S1 to S4, unchanged.
- No open high-rated review finding.

## Exit criteria

1. Stage gate green.
2. One implementation of sign-in, sealing, sync and the instance client exists.
3. No link-time registry remains in a device crate.
4. The bridge binary is a window, command wiring and a `main` over core crates.
5. `architecture/05-bridge-split.md` records where each module landed.

## Decisions reserved to people

- Releasing the bridge on the carved crates.
- The open decisions on on-disk layout, the agents-only product name, the verdict budget, extension hosts and lockdown (`../risks-and-decisions.md`).
