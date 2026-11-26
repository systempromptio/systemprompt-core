# Stages: overview

Nine stages. Each is a set of **waves**; each wave is a set of **lanes** that run at the same time on disjoint paths. Validation is cheap inside a wave and expensive only at the end of a stage (`operating-model.md`).

## 1. The stages

| Stage | Outcome | Repositories |
|---|---|---|
| **S0 Foundations** | Core knows host `harness` (on `next` and `future`). The eight device crates exist in core as empty, gated skeletons. The template has an empty shell workspace and a three-system workflow. Two inventories exist: the lift manifest and the reference-test catalogue | core, template |
| **S1 Lift** | The harness repository is dissolved. Its engine and governance are core crates; its shell is in the template. Both build and pass their tests on Windows, macOS and Linux against core `future`. The repository is archived | core, template |
| **S2 Parity and proof** | A scripted instance, golden scenarios, a comparison against an unmodified reference agent, the migrated test batches, and an evidence run with screenshots from Windows and macOS | core, template |
| **S3 Server-owned sessions** | The native engine's loop runs on the instance. A device with a workspace executes tool requests behind its own rules. Every client renders a session from one resumable stream | core, template |
| **S4 Device composition** | The shared composition machinery; the engine, governance and shell as members; slices as features; a one-line binary; instance control over members in the contract | core, template |
| **S5 Bridge split and integration** | The bridge is split in place into the device crates, its registries become members, the duplicate device code is deleted, its window is rebuilt natively, and the template builds the bridge composition | core, template |
| **S6 Platforms, packaging, signing** | Platform modules with three implementations; desktop on Windows, macOS and Linux; signed artefacts and generated distribution manifests | core, template |
| **S7 Fleet handover and bridge removal** | Installed bridges upgrade to the template-built binary; when every fleet has confirmed, the bridge build leaves core | core, template, installs |
| **S8 Server half and orchestration** | The server `harness` capability, the headless adapter, the runner, the deferred features | core, template |

## 2. The crates

Decided. Eight new crates in core; everything else merges into crates that already exist.

| Crate | Holds | Arrives in |
|---|---|---|
| `systemprompt-engine` | Agent loop, tools and the tool host, MCP, LSP, workspace, configuration, prompts. The harness's eleven engine crates become its top-level modules. From S3 the loop runs on the server and a device links it for the tool host only | S1, S3 |
| `systemprompt-device` | Sign-in, keyrings, config and policy stores, sync, acceptance, platform modules | S1 (harness code), S5 (bridge code replaces it) |
| `systemprompt-harness` | The device composition, projection, update, shared client state machines | S1, S4 |
| `systemprompt-drive` | Folder sync | S1, merged with the bridge's in S5 |
| `systemprompt-proxy` | Loopback proxy, scoped tokens, heartbeat | S5 |
| `systemprompt-machine` | Install, device-management snippets, policy writer, elevation; lockdown as a feature | S5 |
| `systemprompt-hosts` | The host contract; one feature per product | S5 |
| `systemprompt-runner` | Headless adapters, shared with the server | S8 |
| `systemprompt-mobile` | The binding layer for the iOS and Android apps; a ninth crate, owned by the [mobile set](../../mobile/README.md) | Mobile M2 |

Merged into existing crates: typed ids into `systemprompt-identifiers`; owned tasks deleted for `systemprompt-traits`; wire types into `systemprompt-models::bridge`; typed instance calls into `systemprompt-client`; the composition machinery into `systemprompt-provider-contracts`.

All eight are published with core.

## 3. What depends on what

```
S0 ──► S1 ──► S3 ──► S4 ──► S5 ──► S7
 │      │      │      │      │
 │      │      │      │      └──► S6 wave C (packaging the bridge composition)
 │      │      │      └──► S8 (may overlap S5 to S7)
 │      │      └──► the mobile stages' shared core
 │      └──► S6 waves A and B (platform modules, shells)
 └──► S2 (starts with S1, closes after it; S3 needs it closed)
```

| Stage | Needs | May run alongside |
|---|---|---|
| S0 | Nothing | — |
| S1 | S0 | S2 waves A and B |
| S2 | S0; closes after S1 | S1 |
| S3 | S1 and S2 closed | S6 waves A and B |
| S4 | S3 | S6 waves A and B |
| S5 | S4 | S6 waves A and B, S8 |
| S6 | S1 for waves A and B; S5 for wave C | S3, S4, S5, S8 |
| S7 | S5, S6 | S8 |
| S8 | S4 | S5 to S7 |

**The critical path to the first goal** (harness repository retired; core and the template building and tested on Windows and macOS against `future`; parity measured) is S0, S1, S2. None of it touches the bridge.

**Why S2 comes before S3 and S4.** The deterministic suite is built around the lifted code while it still behaves as it did in its own repository. S3 moves the loop to the server and S4 restructures how the device is assembled; the suite is how each proves it changed nothing a user can see.

**Why S3 comes before S4.** Moving the loop removes device-side pieces (per-call model credentials, the local session store, local approval handling) that the composition stage would otherwise convert into members and then delete.

## 4. Parallelism at a glance

Counts are lanes that can be open at once, before the cap of four in `operating-model.md`.

| Stage | Wave | Parallel lanes | Notes |
|---|---|---|---|
| S0 | A | 5 | Host; crate skeletons; template scaffold; two read-only inventories |
| S0 | B | 2 | Review of the host change; scripted-instance interface |
| S1 | A | 2 | Two imports, one per repository. Each is serial inside its repository |
| S1 | B | 6 | Four in core by module group, two in the template |
| S1 | C | 4 | Tests, gates and workflows, the template onto core |
| S1 | D | up to 6 | One consolidated fix round from the first three-system run |
| S2 | A | 4 | Alongside S1 wave B; all new paths |
| S2 | B | 4 | Comparator and three migration batches |
| S2 | C | 3 | Tour and evidence scripts; fourth migration batch |
| S3 | A | 1 then a review | Contract first |
| S3 | B | 4 | Request channel; engine on the server; session stream; tool host |
| S3 | C | 3 | Clients onto the stream; test doubles and scenarios; removal |
| S3 | D | 3 | Independent reviews |
| S4 | A | 1 then a review | Contract first |
| S4 | B, C | 3 each | Members by crate |
| S5 | A–F | 3 each | Bottom-up by the bridge's own layer order |
| S6 | A–C | 3 to 4 | One lane per platform concern group |
| S7 | — | 3, then people | Mostly decisions and observation |
| S8 | A–C | 3 each | One lane per capability |

## 5. Rules that make the parallelism safe

1. **A lane owns paths, not topics.** Two lanes in one wave never share a file. Each stage file lists owned paths per lane.
2. **Shared files have one owner: the integrator.** Root manifests, lockfiles, a crate's `lib.rs` when several lanes own its modules, the recipe file, workflow aggregates, generated caches. A lane that needs a change there writes it in its hand-off note; the integrator applies all of them once per wave.
3. **Contract before consumers.** When a wave introduces a type other lanes use, a single contract lane lands it first, alone.
4. **Moves and edits never share a commit.**
5. **No expensive validation inside a lane.** See the three levels in `operating-model.md`.
6. **A wave closes only when every lane has handed off.** Fixes found at the gate go back as one consolidated message per lane.
7. **A stage closes only on its exit criteria**, checked on hosted runners and, for evidence, by an agent run on real Windows and macOS machines.

## 6. Reading a stage file

Each `sN-*.md` has the same sections: goal, entry conditions, waves (a lane table per wave with id, owned paths, work), wave gate, stage gate, exit criteria, decisions reserved to people.

Lane ids are `S<stage>-<wave><number>`, for example `S1-B3`.
