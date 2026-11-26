# S2. Parity and proof

**Goal.** Deterministic evidence that the lifted harness works and how it compares with an unmodified reference agent, on Windows and macOS, without spending anything on model inference.

**Entry.** S0 closed. Waves A and B add only new paths and run alongside S1 wave B. The stage closes after S1.

**Specification.** `../testing/` holds the design; this file is the work breakdown.

## Paths this stage creates

```
extensions/harness/tests/
  support/scripted-instance/     the mock instance and scripted inference
  golden/scenarios/              scenario data
  golden/runner/                 runs scenarios against the harness
  parity/reference/              runs scenarios against the reference agent
  parity/compare/                normaliser, comparator, ledger
  migrated/<area>/               tests re-expressed from the catalogue
  tour/                          the screenshot tour
scripts/harness/evidence/        builds the evidence pack
```

Plus one production addition in `extensions/harness/shell/`: the headless `run` command and a non-interactive `login`, both real features that later stages keep.

## Wave A — four lanes, parallel (alongside S1-B)

| Lane | Owned paths | Work | Specification |
|---|---|---|---|
| **S2-A1 Scripted instance** | `tests/support/scripted-instance/**` | Implement the interface fixed in S0-B2: discovery, health, the sign-in exchange, a manifest signed with a test key, both model protocols as scripted streams, the hook and verdict endpoints, heartbeat; request recording; fault injection | `testing/02-scripted-instance.md` |
| **S2-A2 Headless entry** | `shell/**/src/app/headless/**`; the command declarations it needs | `run`: one session from the command line, with a prompt, a workspace directory, a permission mode and a machine-readable event stream on stdout. `login --stdin` against a named instance. Both go through the same engine and governor as the window | `testing/03-golden-scenarios.md` section 4 |
| **S2-A3 Scenario corpus** | `tests/golden/scenarios/**` | The scenario files: fixture workspace, scripted model turns, expected normalised trace, expected end state. Data only | `testing/03-golden-scenarios.md` section 2 |
| **S2-A4 Reference runner** | `tests/parity/reference/**`; the reference job in `.github/workflows/harness.yml` (a new job; the integrator merges it) | Install the reference agent at the pinned version on the runner. Configure it to use the scripted instance as its provider. Run each scenario headless. Export its transcript in its native format | `testing/03-golden-scenarios.md` section 5 |

S2-A2 edits `shell/`, which S1-B lanes do not own; S1-B4 owns `device/**` and `engine/**` sources only. If S2-A2 needs a change in `engine/`, it requests it through its hand-off and the integrator schedules it after S1-B4 merges.

**Wave gate (L1).** Offline `cargo check` of the new test crates and of `shell` where a Windows or macOS check is available to the integrator; otherwise `shell` is deferred to the stage gate. Scenario files validate against their schema.

## Wave B — four lanes, parallel

| Lane | Owned paths | Work | Specification |
|---|---|---|---|
| **S2-B1 Golden runner and comparator** | `tests/golden/runner/**`; `tests/parity/compare/**` | Run every scenario against the harness through `run`. Normalise both products' transcripts to one trace form. Compare. Write the parity ledger with every difference classified | `testing/03-golden-scenarios.md` sections 3, 6 |
| **S2-B2 Migration batch 1: tools and shell** | `tests/migrated/tools/**` | Catalogue rows classified *port* for file tools, search tools, patch application, shell selection, output handling, truncation | `testing/04-test-migration.md` |
| **S2-B3 Migration batch 2: session, store, permissions, configuration** | `tests/migrated/session/**`, `tests/migrated/config/**` | Catalogue rows for the agent loop, steering and queueing, abort, retry, compaction, permission rules, settings, agents, skills, commands, instructions | Same |
| **S2-B4 Migration batch 3: protocol clients** | `tests/migrated/llm/**`, `tests/migrated/mcp/**`, `tests/migrated/lsp/**` | Catalogue rows for streaming decode on both protocols, tool-call assembly, MCP transports, LSP diagnostics | Same |

A migrated test that fails because the harness behaves differently is **not** fixed by the migration lane. It is recorded in the ledger as a difference and the lane moves on. Fixing behaviour is wave D.

**Wave gate (L1).** Offline `cargo check` of the new test crates.

## Wave C — three lanes, parallel

| Lane | Owned paths | Work | Specification |
|---|---|---|---|
| **S2-C1 Screenshot tour** | `tests/tour/**`; the tour jobs in the workflow | On Windows and macOS runners: install the packaged app; start the scripted instance; sign in non-interactively; open each state on the tour list; capture each | `testing/05-evidence.md` section 2 |
| **S2-C2 Evidence pack** | `scripts/harness/evidence/**` | Assemble, per run and per system: test results in a machine-readable format, the parity ledger, the screenshots, the installer's hash, toolchain and commit, build sizes. One index page. Attach to the run; write the headline numbers to the run summary | `testing/05-evidence.md` section 3 |
| **S2-C3 Migration batch 4: desktop process and views** | `tests/migrated/desktop/**` | Catalogue rows for single instance, deep links, window state, installer configuration, and the view behaviours that can be asserted without a display | `testing/04-test-migration.md` section 5 |

**Wave gate (L1).** Offline check of the new crates; the evidence script runs against a fixture directory.

## Early stage-gate run

This stage is allowed one early L2 run, after wave C, because nothing here can be proven below L2: the reference agent, the screenshots and the Windows and macOS behaviour all exist only on hosted runners.

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
| Golden scenarios, harness | Every scenario passes on Windows and macOS |
| Parity ledger | Every difference is classified; none is unclassified; none classified as *defect* is open |
| Migrated tests | Every catalogue row classified *port* has a test that passes, or an entry in the ledger |
| Screenshot tour | Every state on the tour list captured on Windows and macOS |
| Evidence pack | Produced for both systems and attached to the run |
| Inference spend | Zero. The workflow has no provider credential in its environment; a job that reaches a real provider host fails |
| S1 stage gate | Still green at this commit |

## Exit criteria

1. The stage gate is green at one commit.
2. The catalogue's counts are published in `testing/04-test-migration.md`: ported, already covered, not applicable, deferred, each with reasons.
3. The ledger is published in the evidence pack and summarised in `testing/03-golden-scenarios.md`.
4. A person has looked at the screenshots from both systems.

## Decisions reserved to people

- The pinned version of the reference agent.
- Which catalogue areas are deferred.
- Raising the retention or quota for run attachments, if the evidence pack cannot be attached.
