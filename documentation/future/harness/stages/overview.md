# Stages: overview

Nine stages. Each is a set of **waves**; each wave is a set of **lanes** that run at the same time on disjoint paths. Validation is cheap inside a wave and expensive only at the end of a stage (`operating-model.md`).

## 1. The stages

| Stage | Outcome | Repositories |
|---|---|---|
| **S0 Foundations** | Core knows host `harness`. The template has an empty harness workspace and a three-system workflow. Two inventories exist: the lift manifest and the reference-test catalogue | core, template |
| **S1 Lift into the template** | The whole harness repository lives in the template as its own cargo workspace, builds and passes its tests on Windows and macOS against core `future`, and is packaged. The harness repository is archived | template |
| **S2 Parity and proof** | A scripted instance, golden scenarios, a comparison against an unmodified reference agent, the first migrated test batches, and an evidence pack with screenshots from Windows and macOS | template |
| **S3 Device composition** | Core's neutral composition contract. The lifted crates become members; slices are features; the binary is one line | core, template |
| **S4 Promote the trust core** | Engine, governor, projection and Drive sync move from the template into core. The template keeps only the product surface. Two tiers reached | core, template |
| **S5 Bridge split and integration** | The bridge is split in place into core device crates, its registries become members, its window is rebuilt natively, and the template builds the agents-only form | core, template |
| **S6 Platforms, packaging, signing** | Platform modules with three implementations, Linux headless, macOS and Linux packaging, signed artefacts, generated distribution manifests | core, template |
| **S7 Fleet handover and bridge removal** | Installed bridges upgrade to the template-built binary; the bridge build leaves core | core, template, installs |
| **S8 Server half and orchestration** | The server `harness` capability, instance control over device members, the headless adapter, the runner, the deferred features | core, template |

## 2. What depends on what

```
S0 ──► S1 ──► S3 ──► S4 ──► S5 ──► S7
 │      │      ▲             │
 │      │      │             └──► S6 (signing) ──► S7
 └──► S2 (starts with S1, closes after it)
               S3 ──► S8 (may overlap S4 to S7)
               S6 platform modules may start after S4
```

| Stage | Needs | May run alongside |
|---|---|---|
| S0 | Nothing | — |
| S1 | S0 wave A | S2 waves A and B |
| S2 | S0; closes after S1 | S1 |
| S3 | S1 closed, S2 closed | — |
| S4 | S3 | S8 design lanes |
| S5 | S4 | S6 waves A and B, S8 |
| S6 | S4 (platform modules); S5 (agents-only packaging) | S5, S8 |
| S7 | S5, S6 | S8 |
| S8 | S3 | S4 to S7 |

**The critical path to the first stated goal** (harness repository retired; template builds and tests on Windows and macOS against `future`; parity measured) is S0, S1, S2. Nothing in S3 or later is needed for it, and none of it touches the bridge.

**Why S2 comes before S3.** The deterministic suite is built around the lifted code while it still behaves exactly as it did in its own repository. Every later stage restructures that code; the suite is how each one proves it changed nothing.

## 3. Parallelism at a glance

Counts are lanes that can be open at once, before the concurrency cap in `operating-model.md`.

| Stage | Wave | Parallel lanes | Notes |
|---|---|---|---|
| S0 | A | 4 | Two code lanes in different repositories, two read-only inventories |
| S0 | B | 2 | Security review of the host change; scripted-instance skeleton |
| S1 | A | 1 | The import. Serial by nature: it creates every path later lanes own |
| S1 | B | 5 | Manifests, gates, workflow and packaging, API drift, brand |
| S1 | C | up to 5 | One consolidated fix round from the first three-system run |
| S2 | A | 4 | Runs alongside S1 wave B; all new paths |
| S2 | B | 4 | Comparator and three migration batches |
| S2 | C | 3 | Screenshot tour, evidence pack, remaining migration batches |
| S3 | A | 1 then 3 | Contract lane first, then members |
| S4 | A–C | 3 per wave | Bottom-up by ring |
| S5 | A–D | 3 per wave | Bottom-up by the bridge's own layer order |
| S6 | A–B | 4 | One lane per platform concern group |
| S7 | — | 2 | Mostly human decisions and observation |
| S8 | A–B | 3 | One lane per capability |

## 4. Rules that make the parallelism safe

1. **A lane owns paths, not topics.** Two lanes in one wave never share a file. Each stage file lists owned paths per lane.
2. **Shared files have one owner: the integrator.** Root `Cargo.toml`, lockfiles, `justfile`, workflow aggregates, `CHANGELOG.md`, generated caches. A lane that needs a change there writes it in its hand-off note; the integrator applies all of them once per wave.
3. **Contract before consumers.** When a wave introduces a type other lanes use, a single contract lane lands it first, alone.
4. **Moves and edits never share a commit.** A commit either relocates files unchanged or changes them.
5. **No expensive validation inside a lane.** See the three levels in `operating-model.md`.
6. **A wave closes only when every lane has handed off.** Fixes found at the gate go back as one consolidated message per lane.
7. **A stage closes only on its exit criteria**, checked on the hosted runners, not on a developer machine.

## 5. Reading a stage file

Each `sN-*.md` has the same sections: goal, entry conditions, waves (a lane table per wave with id, owned paths, work, inputs, hand-off), wave gate, stage gate, exit criteria, decisions reserved to people.

Lane ids are `S<stage>-<wave><number>`, for example `S1-B3`.
