# Evidence: Windows and macOS runs, screenshots, the pack

Proof is produced by hosted runners on every stage-gate run and attached to that run. Nothing in this section is committed to the repository.

## 1. The three-system workflow

| Runner | Builds against | Runs |
|---|---|---|
| Windows | Core `future` | Check; the harness test workspace; migrated tests; golden scenarios; release build; installer packed; silent install; screenshot tour; evidence pack |
| macOS | Core `future` | Check; the harness test workspace; migrated tests; golden scenarios; release build; screenshot tour; evidence pack |
| Linux | Core `future` | Formatting; source gates; supply-chain checks; `engine` and `device` crates and their tests; the parity job against the reference agent |

"Against core `future`" means the harness workspace resolves the linked core crates from the tip of that branch through the template's patch mechanism, recorded as a commit in the evidence pack. A run against a released core version does not satisfy a stage gate.

Runners for public repositories carry no charge. No job holds a model-provider credential.

## 2. The screenshot tour

The packaged application is launched on the runner, pointed at the scripted instance, moved through a fixed list of states, and captured in each.

### 2.1 Method

1. Install the packaged build silently (Windows) or copy the application bundle (macOS).
2. Start the scripted instance's binary with the tour's manifest and scripts.
3. Sign in without interaction: `login --stdin` with the scripted instance's test PAT.
4. For each state: prepare it, bring the window to the state, wait for readiness, capture.
5. Write each capture with its state id, system, scale factor and commit.

| Step | Mechanism | Status |
|---|---|---|
| Preparing a state | Seed the data root by running real commands (`run` with a scenario script creates a session with tool rows, a diff, a todo list, a pending ask) | Uses S2-A2; no test-only seam |
| Reaching a state | Deep links into the running application: a session, settings pages, the governance page, the sign-in gate | **To verify in S2-C1:** which states a deep link already reaches. States it cannot reach are a short list of real navigation features to add, not hidden test hooks |
| Readiness | A structured log line the shell emits when a view has rendered from the change feed, awaited by the tour | To add in S2-C1 |
| Capture | The operating system's own window capture on the runner | **To verify in S2-C1** on both hosted images |

If the hosted Windows or macOS image cannot present a window to capture, S2-C1 reports it at once; the fallback is a self-hosted runner for the tour only. That is a decision for a person.

### 2.2 The tour list

| # | State | Theme |
|---|---|---|
| 1 | Sign-in gate, no instance | Light, dark |
| 2 | Sign-in gate, paste-code flow | Light |
| 3 | Key trust prompt on first manifest | Light |
| 4 | Empty project, signed in, synced | Light, dark |
| 5 | Session with streamed text and reasoning | Light, dark |
| 6 | Tool rows: read, edit with diff, shell with output | Light, dark |
| 7 | Todo list and a subagent row | Light |
| 8 | Permission ask in the dock | Light, dark |
| 9 | Question dock | Light |
| 10 | Tool denied by a rule; tool denied by the instance | Light |
| 11 | Composer with an attachment and a queued message | Light |
| 12 | Model picker and agent picker | Light |
| 13 | Settings: general, permissions, themes | Light, dark |
| 14 | Governance: instances, key trust, sync state | Light, dark |
| 15 | Sync error and stale-manifest states | Light |
| 16 | Drive: documents, a folder in sync | Light |
| 17 | Compaction notice | Light |
| 18 | Revert confirmation and the result | Light |
| 19 | Signed out by a refused credential | Light |
| 20 | About, with version and build provenance | Light |

Later stages extend the list: contributed views and themes (S3), the hosts, marketplace, first-run and inbox views (S5), each fork's brand (S5).

### 2.3 What the screenshots are for

They are evidence that a state renders on a real system, and material for a person to review. They are **not** compared pixel by pixel: font rendering and window chrome differ between runner images, and a pixel gate would fail for reasons unrelated to the harness. Two cheap automatic checks are made on every capture: it is not blank, and its dimensions match the window. Everything else is a human look, recorded as an exit criterion of the stage.

## 3. The evidence pack

One directory per system per run, attached to the run and summarised on its summary page.

```
evidence/<system>/
  index.md                 commit, core commit, toolchain, runner image, timings
  tests/                   results for every suite, machine-readable, with counts
  golden/                  per scenario and protocol: pass or fail, normalised trace
  parity/ledger.md         counts by classification, then every difference
  parity/requests/         the prompt and tool schema each product sent, side by side
  migrated/catalogue.md    counts by area and class; open rows
  screenshots/<state>.png
  package/                 installer name, size, hash; install log
  sizes.md                 binary size and crate closure
```

`index.md` opens with a table a person can read in ten seconds:

| Line | Example |
|---|---|
| Suites | Crate tests 942 of 942; migrated 310 of 318 (8 in ledger); golden 48 of 48 |
| Parity | 39 same, 7 intended, 2 gap, 0 defect, 0 unclassified |
| Tour | 20 of 20 states on this system |
| Package | Installer built, installed, reported its version |
| Inference spend | None; no provider credential present |

The numbers above are illustrations of the format, not results.

### 3.1 Storage

Attachments on hosted runs are subject to a storage quota. The pack is attached with a short retention on push runs and a long retention on the run that closes a stage. If the quota prevents attachment, the summary page still carries `index.md` and the ledger counts, and the run fails its "evidence attached" step so the gap is visible. Raising the quota or providing other storage is a decision for a person.

## 4. What a stage closure cites

The commit that closes a stage updates the status table in `../README.md` and cites, in its message, the run whose evidence pack satisfied the stage gate. The pack is the proof; this directory holds only the method.

## 5. The manual walk

Some things no runner shows: an elevation prompt, a keyring that survives a restart and a reboot, a non-UTF-8 console code page on a real user's shell, a second monitor at another scale factor, an administrator-pinned key. They are a written checklist walked by a person on real machines before any build reaches users outside the maintainers (`../architecture/08-platforms-and-packaging.md` section 6). The checklist's result is recorded in the closing commit of the stage that required it.
