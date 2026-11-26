# Operating model

How the work is run so that several agents or people can proceed at once, and so that slow, costly checks happen rarely.

## 1. Roles

| Role | Does | Does not |
|---|---|---|
| **Integrator** (one per stage) | Writes lane briefs; creates each lane's branch and working tree; merges lanes; owns shared files; runs wave and stage gates; pushes; reads the hosted runs; sends consolidated fixes | Implement lane work |
| **Lane** | Edits its owned paths to complete its brief; writes a hand-off note | Touch another lane's paths or a shared file; push to a shared branch; run level 1 or level 2 validation |
| **Reviewer** (independent, read-only) | Reviews anything on a security path against the specification | Fix what it finds; share context with the implementing lane |

A lane is one agent or one person. A lane is never reused past two rounds; a fix round gets a fresh lane that treats earlier work on the branch as an unreviewed draft.

## 2. Concurrency

- At most **four** lanes open at once across all repositories; three is the default. A wave with more lanes than the cap runs them as they free up, in the order listed.
- Read-only lanes (inventories, reviews) count toward the cap.
- Each lane has its own branch, its own working tree and its own cargo target directory.

## 3. A lane brief

One file per lane, written by the integrator before the wave opens. A lane is started with one line pointing at it.

| Section | Content |
|---|---|
| Goal | One sentence |
| Owned paths | Exact globs. Everything else is read-only |
| Inputs | Files to read first: the stage file, the relevant `architecture/` sections, the lift manifest or catalogue rows |
| Work | Numbered items, each checkable |
| Not in scope | Named neighbouring work that belongs to another lane |
| Standards | The repository's coding standards; the rules in section 6 |
| Hand-off | The note format in section 4 |

## 4. The hand-off note

Every lane ends with a note in this shape. The integrator merges on the note, not on the lane's word that it is finished.

1. Commits, in order, one line each.
2. Files changed outside the brief's work items, with the reason.
3. Public API added, changed or removed.
4. Shared-file changes requested (exact text for the integrator to apply).
5. Anything assumed and not checked.
6. Anything left undone, and why.
7. Debt removed; existing primitives reused instead of duplicated.

## 5. Three validation levels

The costly checks are few and late. A lane never waits on a build of the whole workspace or on a hosted runner.

| Level | When | Who | What | Cost |
|---|---|---|---|---|
| **L0 Lane self-check** | Before hand-off | Lane | `cargo check -p <own crate>` for crates it edited, on its own target directory. Nothing else. A lane that only moves files, or only writes data, scripts or documents, runs nothing | Seconds to a few minutes |
| **L1 Wave gate** | Once, after every lane of the wave is merged | Integrator | Offline `cargo check --workspace --keep-going` of the affected workspace; the source gates; formatting check; the cheap repository gates. No tests, no lints with all targets, no documentation build | Minutes |
| **L2 Stage gate** | Once, when the last wave of a stage has passed L1 | Integrator, on hosted runners | Lints on all targets; the full test workspace; documentation build; supply-chain checks; the three-system build, test and package jobs; the deterministic suites of `../testing/`; the evidence pack | Tens of minutes of runner time; no local cost |

Rules:

- **Nothing above L0 runs inside a lane.** No test run, no all-targets lint, no documentation build, no packaging.
- **L1 runs once per wave**, not once per lane. If it fails, the integrator sorts failures by owning lane and sends **one** message per lane.
- **L2 runs once per stage.** A stage with a known-risky wave may add one early L2 run, named in its stage file; no other early runs.
- **Hosted runners are the proof** for Windows and macOS. No local machine stands in for them.
- **A red L2 is the next thing to fix.** No new wave opens on top of it.
- **A failure that repeats is not a flake.** It gets its own lane with a deliberate reproduction.
- **No real inference, ever.** Any suite that would call a model provider uses the scripted instance (`../testing/02-scripted-instance.md`).

## 6. Standing rules for lanes

- Follow the repository's coding standards without exception. They are not relaxed for moved code; a stage that imports code which does not meet them lists the remediation as its own lane.
- No legacy shims, dual paths or compatibility stubs. Land the new form and delete the old in the same wave.
- Moves and edits never share a commit.
- Never bypass, disable or loosen a gate, hook or lint to get a change through. A gate that is wrong is reported in the hand-off note.
- A gate or test that was already failing before the lane's change is still the lane's to report, and the wave's to fix.
- Temporary files stay inside the lane's own target directory.
- Names of other projects do not appear in source, comments, strings or file names. Behaviour may be studied; text is not copied.

## 7. Security review

A separate, read-only reviewer for each change that feeds an authorization or trust decision. The reviewer receives the diff and the specification, not the implementing lane's reasoning. Stage files mark these lanes **(review)**. A stage does not close with an open finding rated high.

## 8. Decisions reserved to people

No agent performs these; a stage file lists which apply.

- Merging a programme branch into a shared integration branch of core.
- Archiving, deleting or changing the visibility or licence of a repository or crate.
- Any release, publish, signing identity or certificate.
- Any deploy; any rollout to an installed fleet; any change to an instance's minimum client version.
- Any spend.

## 9. Where things are recorded

| Record | Location |
|---|---|
| Stage state | The status table in `../README.md` |
| Decisions | `../risks-and-decisions.md` |
| Specification changes found during work | The relevant `architecture/` file, in the same change |
| Lane briefs and hand-off notes | With the integrator, outside the repository; they are working files, not documentation |
| Evidence from L2 runs | The run's own attachments and summary (`../testing/05-evidence.md`); never committed |
