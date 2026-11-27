# S2. Parity and proof

**Goal.** Deterministic evidence that the lifted engine and shell work and how it compares with an unmodified reference agent, on Windows and macOS, without spending anything on model inference.

**Entry.** S0 closed. Waves A and B add only new paths and run alongside S1 wave B. The stage closes after S1.

Before wave A opens the integrator lands, in core's shared crates, the route constants the scripted instance needs and core does not yet export: `hooks/govern` and the three model routes (`../testing/02-scripted-instance.md` section 2.3). Open decision O6 (the principal and attestation tier of a harness session) is needed by S2-A1, which issues the test tokens.

**Specification.** `../testing/` holds the design; this file is the work breakdown.

## Paths this stage creates

In core's test workspace (they test core crates):

```
scripted-instance/        the mock instance and scripted inference (the crate and its interface exist from S0-B2)
scripted-instance-tests/  sign-in, sync, heartbeat, hooks, feeds and one governed step against the scripted instance (S2-B1)
golden/scenarios/         scenario data
golden/runner/            runs scenarios against the engine through `run`
parity/reference/         runs scenarios against the reference agent
parity/compare/           normaliser, comparator, ledger
migrated/<area>/          tests re-expressed from the catalogue
```

In the template:

```
extensions/harness/tests/migrated/desktop/    shell and view behaviours
extensions/harness/tests/tour/                the screenshot tour
scripts/harness/evidence/                     the evidence script
```

Plus one production addition in core's `systemprompt-harness`: the headless `run` command and a non-interactive `login`, both real features that later stages keep.

## Wave A — four lanes, parallel (alongside S1-B)

| Lane | Repository | Owned paths | Work | Specification |
|---|---|---|---|---|
| **S2-A1 Scripted instance** | core | The scripted-instance test crate | Implement the interface fixed in S0-B2 without changing it: discovery, health, the sign-in exchange, a manifest signed with a test key, both model protocols as scripted streams, the hook and verdict endpoints, heartbeat; request recording; fault injection; `wait_for` with its timeout. Its own tests against core's verifier and decoder. The crate stays free of the dependency-unification crate so it runs on the native matrix | `testing/02-scripted-instance.md` |
| **S2-A2 Headless entry** | core | `systemprompt-harness/src/command/**` | `run`: one session from the command line, with a prompt, a workspace directory, a permission mode and a machine-readable event stream on stdout. `login --stdin` against a named instance. Both go through the same engine and governor as the window | `testing/03-golden-scenarios.md` section 4 |
| **S2-A3 Scenario corpus** | core | `golden/scenarios/**` | The scenario files: fixture workspace, scripted model turns, expected normalised trace, expected end state. Data only | `testing/03-golden-scenarios.md` section 2 |
| **S2-A4 Reference runner** | core | `parity/reference/**`; a parity job in core's workflow, Linux only (the integrator merges it) | Install the reference agent at the pinned version on the runner. Configure it to use the scripted instance as its provider. Run each scenario headless. Export its transcript in its native format | `testing/03-golden-scenarios.md` section 5 |

S2-A2 owns a new directory in `systemprompt-harness`, which S1-B4 does not touch. If it needs a change elsewhere in the crate, it requests it through its hand-off and the integrator schedules it after S1-B4 merges.

**Wave gate (L1).** Offline `cargo check` of the new test crates and of `systemprompt-harness`. Scenario files validate against their schema.

## Wave B — four lanes, parallel

| Lane | Owned paths (core's test workspace) | Work | Specification |
|---|---|---|---|
| **S2-B1 Golden runner and comparator** | `golden/runner/**`; `parity/compare/**`; `scripted-instance-tests/**` | Run every scenario against the harness through `run`. Normalise both products' transcripts to one trace form. Compare. Write the parity ledger with every difference classified. Re-express the eight behaviours of the harness's live test package against the scripted instance, with no real instance and no model call | `testing/03-golden-scenarios.md` sections 3, 6 |
| **S2-B2 Migration batch 1: tools and shell** | `migrated/tools/**` | The 67 catalogue rows classified *port* for file tools, search tools, patch application, shell selection, output handling, truncation | `testing/04-test-migration.md` |
| **S2-B3 Migration batch 2: session, store, permissions, configuration** | `migrated/session/**`, `migrated/config/**` | The 157 catalogue rows for the agent loop, steering and queueing, abort, retry, compaction, permission rules, settings, agents, skills, commands, instructions | Same |
| **S2-B4 Migration batch 3: protocol clients** | `migrated/llm/**`, `migrated/mcp/**`, `migrated/lsp/**` | The 51 catalogue rows for streaming decode on both protocols, tool-call assembly, MCP transports, LSP diagnostics | Same |

Each batch lane first replaces a file's catalogue row with one row per case, then writes the tests. 40 rows of batch 2 and 10 of batch 3 are marked as needing the scripted instance; that is a floor, and batch 2's agent-loop rows are the likely additions. Rows that carry a doubt are settled by the lane that opens them.

A migrated test that fails because the harness behaves differently is **not** fixed by the migration lane. It is recorded in the ledger as a difference and the lane moves on. Fixing behaviour is wave D.

**Wave gate (L1).** Offline `cargo check` of the new test crates.

## Wave C — two lanes in parallel, and batch 4 as successive lanes

| Lane | Repository | Owned paths | Work | Specification |
|---|---|---|---|---|
| **S2-C1 Screenshot tour** | template | `extensions/harness/tests/tour/**` | A script that, on a Windows or macOS machine: installs the packaged app; starts the scripted instance; signs in non-interactively; opens each state on the tour list; captures each | `testing/05-evidence.md` section 2 |
| **S2-C2 Evidence script** | template | `scripts/harness/evidence/**` | One command that runs the suites and the tour on the machine it is started on and writes a local, untracked evidence folder with one index page | `testing/05-evidence.md` section 3 |
| **S2-C3 Migration batch 4: desktop process and views** | template | `extensions/harness/tests/migrated/desktop/**`, one subdirectory per lane (below) | Catalogue rows for single instance, deep links, window state, installer configuration, and the view behaviours that can be asserted without a display | `testing/04-test-migration.md` section 5 |

Batch 4 is 645 of the 920 rows to port, too many for one lane. S2-C3 runs as five successive lanes by catalogue area, each a fresh lane on its own subdirectory, opened in this order as the cap allows. The first covers process behaviour; the rest are view-state logic.

| Lane | Catalogue areas | Subdirectory | Rows | Of which need a display |
|---|---|---|---|---|
| **S2-C3a** | Desktop application, Command line, Core engine | `process/` | 26 | 0 |
| **S2-C3b** | Session UI, Application end-to-end | `session/` | 145 | 45 |
| **S2-C3c** | Application UI | `app/` | 200 | 19 |
| **S2-C3d** | Terminal UI, Native client (Rust) | `composer/` | 197 | 11 |
| **S2-C3e** | GUI plugins, Others | `panels/` | 77 | 8 |

A row that needs a display is not asserted: it becomes a state on the screenshot tour (S2-C1 adds it to the tour list) or a line on the manual walk, and its catalogue row records which. The two batch-4 rows that need the scripted instance are in S2-C3d.

**Wave gate (L1).** Offline check of the new crates; the evidence script runs against a fixture directory. The gate is run when S2-C1, S2-C2 and S2-C3a have handed off, and again after the last batch-4 lane.

## First full run

After wave C's first gate the integrator pushes and reads the hosted runs (tests and the parity job), and an agent session attached to a real Windows machine and a real macOS machine runs the evidence command on each. This is the one early L2 this stage allows: nothing here can be proven below it.

## Wave D — triage and fix, up to three lanes

The integrator reads the first ledger and the migrated-test results and groups the differences.

| Group | Action |
|---|---|
| Intended difference (governance, permission surface, tool naming, prompt text) | Recorded in the ledger with its reason. No code change |
| Harness defect | A fix lane on the owning crate. One lane per ring: `engine`, `device`, `shell` |
| Reference behaviour the harness lacks and wants | Recorded as a gap with an owner stage. Not fixed here unless it blocks a scenario |
| Test wrong | Corrected by a fresh lane on the test paths |

## Stage gate (L2)

| Check | Must hold |
|---|---|
| Golden scenarios | Every scenario passes on Windows, macOS and Linux |
| Parity ledger | Every difference is classified; none is unclassified; none classified as *defect* is open |
| Migrated tests | Every catalogue row classified *port* has a test that passes, or an entry in the ledger |
| Screenshot tour | Every state on the tour list captured on a real Windows machine and a real macOS machine |
| Evidence folder | Produced on both machines by an agent session and read by a person |
| Inference spend | Zero. The workflow has no provider credential in its environment; a job that reaches a real provider host fails |
| S1 stage gate | Still green at this commit |

## Exit criteria

1. The stage gate is green at one commit.
2. The catalogue's counts are published in `testing/04-test-migration.md`: ported, already covered, not applicable, deferred, each with reasons. They replace the first-classification counts S0 published there (920 to port, 43 covered, 1,679 not applicable, 51 deferred).
3. The ledger is in the evidence folder and its counts are published in `testing/03-golden-scenarios.md`.
4. A person has looked at the screenshots from both systems.

## Decisions reserved to people

- The pinned version of the reference agent. Taken for this stage: tag `v2.4.2` (D47). Raising it is a person's decision and reruns the survey as a diff.
- Which catalogue areas are deferred. The first classification defers 51 rows to S5, S7 and S9; a `no-feature` row becomes a deferred row only by a person giving a stage the feature.
- Having a Windows machine and a macOS machine online and linked when the stage closes.
