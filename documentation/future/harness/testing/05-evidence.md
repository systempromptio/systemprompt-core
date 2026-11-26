# Evidence: Windows and macOS runs, screenshots, the evidence folder

Evidence is a **development process, not continuous integration**. Hosted runners build and test on three systems and store nothing. Proof that the product works on real machines, with screenshots, is produced at stage closure by an agent session attached to a real Windows machine and a real macOS machine, running one command on each.

## 1. Two kinds of proof

| Kind | Where | What it proves | Stored |
|---|---|---|---|
| Hosted runs | Core's and the template's workflows on Windows, macOS and Linux runners | It compiles, lints, passes every suite, packs an installer and the installer installs, against core `future` | Nothing beyond the run's own log and summary |
| Evidence run | A real Windows machine and a real macOS machine, by an agent session attached to them | The packaged application installs, starts, signs in, and renders each state; the suites pass on that machine too | A local, untracked folder on each machine, reported back by the agent |

"Against core `future`" means the linked core crates resolve from the tip of that branch through the template's patch mechanism; the commit is recorded in the evidence index. A run against a released core version does not satisfy a stage gate.

No job and no machine in this process holds a model-provider credential.

## 2. The screenshot tour

The packaged application is launched, pointed at the scripted instance, moved through a fixed list of states, and captured in each.

### 2.1 Method

1. Install the packaged build silently (Windows) or copy the application bundle (macOS).
2. Start the scripted instance's binary with the tour's manifest and scripts.
3. Sign in without interaction: `login --stdin` with the scripted instance's test PAT.
4. For each state: prepare it, bring the window to the state, wait for readiness, capture.
5. Write each capture with its state id, system, scale factor and commit.

| Step | Mechanism | To settle in S2-C1 |
|---|---|---|
| Preparing a state | Seed the data root by running real commands: `run` with a scenario script creates a session with tool rows, a diff, a todo list, a pending ask | — |
| Reaching a state | Deep links into the running application: a session, settings pages, the governance page, the sign-in gate | Which states a deep link already reaches. Any it cannot reach become real navigation features, not hidden test hooks |
| Readiness | A structured log line the shell emits when a view has rendered from the change feed | To add |
| Capture | The operating system's own window capture | Straightforward on a real desktop session, which is why the tour runs there |

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

Later stages extend the list: contributed views and themes (S4); the hosts, marketplace, first-run and inbox views and the tray (S5); Linux, and a sealed sign-in surviving a relaunch (S6).

### 2.3 What the screenshots are for

They are evidence that a state renders on a real system, and material for a person to review. They are **not** compared pixel by pixel: font rendering and window chrome differ between machines. Two automatic checks are made on every capture: it is not blank, and its dimensions match the window. Everything else is a human look, recorded as an exit criterion of the stage.

## 3. The evidence command and folder

One command, run on the machine it is started on:

1. Build the release binary and package it, against core `future`.
2. Run the engine, device and shell suites, the migrated tests and the golden scenarios.
3. Run the screenshot tour.
4. Write the folder and print its index.

The folder is untracked and stays on the machine.

```
evidence/<system>/
  index.md                 commit, core commit, toolchain, machine, timings
  tests/                   results for every suite, machine-readable, with counts
  golden/                  per scenario and protocol: pass or fail, normalised trace
  parity/ledger.md         copied from the hosted parity job's summary for this commit
  parity/requests/         the prompt and tool schema each product sent, side by side
  migrated/catalogue.md    counts by area and class; open rows
  screenshots/<state>.png
  package/                 installer name, size, hash; install log
  sizes.md                 binary size and crate closure
```

`index.md` opens with a table a person can read in ten seconds:

| Line | Example of the format |
|---|---|
| Suites | Crate tests 942 of 942; migrated 310 of 318 (8 in ledger); golden 48 of 48 |
| Parity | 39 same, 7 intended, 2 gap, 0 defect, 0 unclassified |
| Tour | 20 of 20 states on this system |
| Package | Installer built, installed, reported its version |
| Inference spend | None; no provider credential present |

The numbers are illustrations of the format, not results.

## 4. Who runs it, and when

- **At stage closure, once per stage**, not per push and not per wave.
- **An agent session attached to the two machines** runs the command on each, reads the index, looks at every screenshot, and reports: the index tables, the screenshots, anything that looks wrong.
- **A person looks at the screenshots** the agent returns. That look is an exit criterion.
- Both machines must be online and linked when a stage closes. If one is not, the stage stays open.
- The agent does not release, sign, deploy or change either machine beyond installing and removing the build under test in a user account.

## 5. What a stage closure cites

The commit that closes a stage updates the status table in `../README.md` and states, in its message: the commits tested in each repository, the hosted runs that were green, and the evidence index tables from both machines.

## 6. The manual walk

Some things neither a runner nor a scripted tour shows: an elevation prompt, a keyring that survives a reboot, a non-UTF-8 console code page on a real user's shell, a second monitor at another scale factor, an administrator-pinned key. They are a written checklist walked by a person on real machines before any build reaches users outside the maintainers (`../architecture/08-platforms-and-packaging.md` section 6). Its result is recorded in the closing commit of the stage that required it.
