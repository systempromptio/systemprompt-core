# Migrating the reference agent's tests

The reference agent ships a large test suite, including tests for its desktop application. Every behaviour those tests assert that applies to the harness is re-expressed as a Rust test, in core's test workspace for engine and device behaviour and in the template for shell behaviour, so parity is something the suite checks on every run.

## 1. What the reference suite contains

Surveyed by lane S0-A5 against the reference agent's public repository at the pinned tag (D47):

| Item | Value |
|---|---|
| Reference agent | OpenCode |
| Pinned tag | `v2.4.2` |
| Tag commit | `fc31a485` |
| Licence at the tag | MIT |
| Survey date | 2026-11-26 |

Upstream keeps two release lines. The pin is the newest stable tag, on the 2.x line; its package registry's `latest` still points at the 1.x line. The parity job (S2-A4) installs the release of the pinned tag, not whatever the registry calls latest.

Counts are test files at the tag. "First survey" is the earlier count taken from a downstream fork, kept for comparison. "Applies" is the share of catalogue rows not classified `not-applicable`.

| Area | Path at the tag | Test files | First survey | Kind (rows) | Applies to the harness |
|---|---|---|---|---|---|
| Application UI | `packages/app` except `e2e/` | 387 | 433 | unit 379, e2e 53, integration 1 | Partly (49% of rows): view-state logic yes; web mechanics, tabs, voice, remote workspaces, client-of-server plumbing no |
| Application end-to-end | `packages/app/e2e` | 62 | 60 | e2e 106, unit 2 | Partly (63%): suites at the tag are regression 46, service-worker 1, stability 14, user-story 1. User-visible behaviours yes; service worker and mobile no |
| Core engine | `packages/core` | 369 | 333 | integration 314, unit 121, snapshot 6, desktop 1 | About half (44%): tools, sessions, permissions, configuration, MCP, LSP yes; provider plugins (75 files under `test/plugin`), plugin runtime, worktrees, pty, remote machines no |
| Terminal UI | `packages/tui` | 206 | 200 | unit 157, integration 70, e2e 8, snapshot 2 | Partly (42%), more than the first survey assumed: composer, prompt history, transcript, picker and permission state logic is front-end independent; rendering and keymaps no |
| Model clients | `packages/ai` | 170 | 170 | unit 140, snapshot 63, integration 1 | Little (20%): only the two gateway protocols and protocol-neutral logic; 116 of the files are per-provider |
| Session UI | `packages/session-ui` | 101 | 88 | unit 70, e2e 42, snapshot 2, integration 1 | Mostly (75%): behaviour yes |
| Desktop application | `packages/desktop` | 77 | 69 | unit 56, integration 13, desktop 12 | Partly (31%): process behaviour yes; embedded-browser mechanics, plugin host and sidecar-server connection no |
| Command line | `packages/cli` | 62 | 58 | integration 40, unit 30 | Partly (27%): the headless run yes (S2 adds the command); the editor protocol (17 files) no |
| Server | `packages/server` | 54 | 48 | integration 50, unit 8 | Mostly no (24%): rows that assert engine behaviour through the API are classified by that behaviour |
| GUI plugins (new) | `packages/gui-plugins`, `packages/plugin-browser` | 210 | inside "Others" (120 in the fork) | unit 185, integration 27, desktop 1 | Little (15%): 118 files are the embedded web browser panel; review, details and updater panels apply |
| Native client (Rust) (new) | `packages/cli-rs` | 346 | absent from the fork | integration 215, unit 192, snapshot 27, e2e 1 | Partly (34%): a Rust terminal client of the reference's own server; its client-side store and prompt/transcript logic apply |
| Others | `packages/{ui,util,schema,sdk,protocol,client,data,merman,latex,patchmark,plugin,process-daemon,codemode,simulation,http-recorder,httpapi-codegen,script,nav-menus,theme}`, `services/*` | 286 | about 500 (with the two rows above and the fork's own files) | unit 243, integration 51, snapshot 2, e2e 1 | Case by case (27%): 45 files are the website and hosted services |

2,330 files hold at least one case: 2,124 named test files and 221 Rust source files with inline tests, less 15 Rust support modules with none.

The reference suite also contains things worth learning from rather than porting: a recorder that replays real HTTP traffic from deterministic cassettes, which is the same idea as the scripted instance; its own fake-provider and test-driver tooling; a parity suite between its two clients, which is the same idea as the parity ledger; and written rules for its end-to-end tests (no wall-clock waits; one keeper suite per product area; assert what the user sees). The last is adopted in `01-strategy.md` section 6.

### 1.1 Counts from the first classification

2,693 rows over the 2,330 files.

| | `port` | `covered` | `not-applicable` | `defer` | Total |
|---|---|---|---|---|---|
| Rows | 920 | 43 | 1,679 | 51 | 2,693 |

| Area | port | covered | not-applicable | defer | Total |
|---|---|---|---|---|---|
| Application UI | 204 | 1 | 222 | 6 | 433 |
| Application end-to-end | 66 | 1 | 40 | 1 | 108 |
| Core engine | 165 | 20 | 248 | 9 | 442 |
| Terminal UI | 95 | 2 | 138 | 2 | 237 |
| Model clients | 39 | 1 | 164 | 0 | 204 |
| Session UI | 84 | 2 | 29 | 0 | 115 |
| Desktop application | 23 | 0 | 56 | 2 | 81 |
| Command line | 11 | 2 | 51 | 6 | 70 |
| Server | 11 | 2 | 44 | 1 | 58 |
| GUI plugins | 24 | 0 | 182 | 7 | 213 |
| Native client (Rust) | 125 | 10 | 287 | 13 | 435 |
| Others | 73 | 2 | 218 | 4 | 297 |
| **Total** | 920 | 43 | 1,679 | 51 | 2,693 |

`port` rows by batch (section 6): batch 1, 67; batch 2, 157; batch 3, 51; batch 4, 645.

`not-applicable` rows by reason: `no-feature` 599, `reference-internal` 293, `provider-catalogue` 240, `terminal-ui` 189, `http-server` 123, `plugin-runtime` 119, `browser-runtime` 63, `provider-keys` 30, `mobile-layout` 23. `covered` rows: 17 the same, 26 all but a trivial remainder.

`defer` rows by the stage that delivers the feature:

| Stage | Rows | Awaits |
|---|---|---|
| S5 Device composition | 10 | Members activating independently; members contributing views, themes, tools and commands |
| S7 Platforms, packaging, signing | 15 | The update flow; installers for macOS and Linux; shell selection and environment on macOS and Linux; child-process-tree kill on real platform modules |
| S9 Server half and orchestration | 26 | Remote control and pairing between devices; routines and scheduled automations; driving and observing other agents |

The working catalogue labels these three groups S3, S5 and S7: it was classified against the stage numbering that preceded the two execution-mode stages. The mapping above follows each row's stated reason. Scheduled sessions are now built in S4, so the automation rows among the 26 are re-read against S4 when that stage opens.

What the counts are, and are not:

- A **first** classification, made from each file's test declarations and from the file body where those were unclear. 64 rows carry a doubt in `note`, most of them `port` rows that hold only if the harness has the matching view or feature. The batch lane that opens a file settles its rows.
- About 19,100 cases stand behind the rows. That is a lower bound: table-driven and generated cases are under-counted.
- `covered` was judged against the names of the harness's tests, not their bodies. Only 43 rows are covered: the migrated suites are nearly all new tests.
- 59 rows (696 cases) need the scripted instance or a golden scenario: 40 in batch 2, 10 in batch 3, 2 in batch 4, 5 deferred, 2 already covered. That is a floor. Many reference tests inject an in-process fake model and were marked `unit`; batch 2's agent-loop rows are the likely additions.
- Two behaviour differences are already visible and go to the parity ledger, not to a fix: a read at an offset past the end of a file (the reference returns the file's end, the harness an error), and a shell command that times out (the reference moves it to the background, the harness kills it).

### 1.2 Counts after migration, 2026-11-28

Printed by `scripts/parity/count-cases.py --template DIR`, which recounts the rows of every `cases.md` and is the source of every figure in this section; rerun it rather than editing a figure by hand. Core batches 1 to 3 were read at core commit `617a37f4b0` (`crates/tests/migrated/`); template batch 4 at template commit `80ceb561` (`extensions/harness/tests/migrated/desktop/`).

The 920 catalogue rows first classified `port` are now 6,422 case rows. A case row counts under its class when that class is `covered`, `not-applicable` or `defer`, and under its `state` when the class is `port`. The 1,773 catalogue rows first classified otherwise (43 covered, 1,679 not applicable, 51 deferred) were not opened by a batch lane and keep the counts of section 1.1. The catalogue-row column is the catalogue's, not the script's.

By batch:

| Batch | Catalogue rows | Case rows | passing | ledger | covered | not applicable | deferred | tour | manual | open | written |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | 67 | 680 | 421 | 165 | 22 | 56 | 16 | 0 | 0 | 0 | 0 |
| 2 | 157 | 1,339 | 381 | 402 | 91 | 464 | 1 | 0 | 0 | 0 | 0 |
| 3 | 51 | 485 | 213 | 79 | 40 | 153 | 0 | 0 | 0 | 0 | 0 |
| 4 | 645 | 3,918 | 207 | 1,757 | 5 | 1,471 | 1 | 477 | 0 | 0 | 0 |
| **Total** | 920 | 6,422 | 1,222 | 2,403 | 158 | 2,144 | 18 | 477 | 0 | 0 | 0 |

By file:

| Batch | Path | Catalogue rows | Case rows | passing | ledger | covered | not applicable | deferred | tour | manual | open | written |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | `migrated/tools/` | 67 | 680 | 421 | 165 | 22 | 56 | 16 | 0 | 0 | 0 | 0 |
| 2 | `migrated/config/` | 37 | 199 | 137 | 23 | 5 | 34 | 0 | 0 | 0 | 0 | 0 |
| 2 | `migrated/session/` | 120 | 1,140 | 244 | 379 | 86 | 430 | 1 | 0 | 0 | 0 | 0 |
| 3 | `migrated/llm/` | 46 | 429 | 194 | 72 | 34 | 129 | 0 | 0 | 0 | 0 | 0 |
| 3 | `migrated/mcp/` | 5 | 56 | 19 | 7 | 6 | 24 | 0 | 0 | 0 | 0 | 0 |
| 4a | `desktop/process/` | 26 | 137 | 24 | 30 | 1 | 71 | 1 | 10 | 0 | 0 | 0 |
| 4b | `desktop/session/` | 145 | 852 | 24 | 458 | 0 | 177 | 0 | 193 | 0 | 0 | 0 |
| 4c | `desktop/app/` | 200 | 1,253 | 43 | 644 | 4 | 437 | 0 | 125 | 0 | 0 | 0 |
| 4d | `desktop/composer/` | 197 | 1,085 | 99 | 440 | 0 | 428 | 0 | 118 | 0 | 0 | 0 |
| 4e | `desktop/panels/` | 77 | 591 | 17 | 185 | 0 | 358 | 0 | 31 | 0 | 0 | 0 |

Batch 3 has no `migrated/lsp/` file: its 51 catalogue rows are all in `migrated/llm/` (46) and `migrated/mcp/` (5).

What each column means:

| Column | Meaning |
|---|---|
| passing | A Rust test asserts the case and passes |
| ledger | The harness behaves differently or lacks the behaviour; the difference is recorded on the row, with an ignored test asserting the reference behaviour where one can express it |
| covered | An existing harness test already asserts the case |
| not applicable | The case was read and falls under a reason code of section 3 |
| deferred | The case waits for a feature a later stage delivers |
| tour | The case needs a display; it is a state on the screenshot tour, checked by a person |
| manual | The shell has the behaviour, but its logic sits in a view with no seam a test can drive without a window; a line on the manual walk |
| open | Nothing written |
| written | A test exists but has not been run on the system it needs |

**`port` case rows with neither a passing test nor a ledger entry.** Of the 4,102 case rows still classified `port`, 1,222 pass, 2,403 are in the ledger and 477 are tour states. No row is `open`, `written` or `manual`: the 14 Windows-only tests of `migrated/tools/` passed in the device-native run on a Windows 11 machine on 28 November 2026. The stage gate (`../stages/s2-parity-and-proof.md`, "Migrated tests") fails on every `open`, `written` or `manual` row, and `count-cases.py --check` exits 1 on the same rows.

A `tour` row has no test and no ledger entry either. It is not counted above because section 5 makes it a state on the screenshot tour, which has its own line in the stage gate; the 477 tour rows are all in batch 4.

Every file's own totals line agrees with its rows: `count-cases.py --check` prints each disagreement, and printed none.

## 2. The catalogue

Produced by S0-A5 and maintained as a data file beside the migrated tests. The S0 catalogue is a working file held with the integrator; S2 converts it to that data file, mechanically, when the first batch lane opens.

**Granularity.** One row per reference test **file**, split into two to four rows where a file's cases fall into different classes or areas (`scope` says which cases a split row covers). One row per case is produced by the batch lane that opens the file: it replaces the file's row with case rows before it writes a test.

| Column | Content |
|---|---|
| `id` | Stable identifier: an area code and a hash of the source path, with a `.n` suffix on split rows |
| `area` | From the table above |
| `source` | Path at the pinned tag; `scope` narrows a split row to some of the file's cases. The case name is added when the row becomes case rows |
| `behaviour` | One sentence, in the harness's own words, stating what must be true |
| `kind` | What the reference test is: `unit`, `integration`, `e2e`, `snapshot`, `desktop` |
| `cases` | The number of test cases the row stands for. A lower bound until the file is opened |
| `class` | `port`, `covered`, `not-applicable`, `defer` |
| `reason` | Required for every class except `port`. Opens with a code (section 3) |
| `target` | The Rust test that asserts it, once written; for `covered`, the existing test |
| `needs` | `unit`, `scripted-instance`, `golden`, `display` |
| `batch` | The migration batch, `1` to `4`, for `port`; `later` for `defer`; none otherwise |
| `note` | Free text for the lane that picks the row up; `doubt` marks a classification worth a second look |
| `state` | `open`, `written`, `passing`, `ledger` (fails because behaviour differs; see the parity ledger) |

Section 1.1 holds the counts from the first classification. A summary generated from the data file replaces it when S2 closes: counts by area and class, and the list of deferred areas with reasons.

## 3. Classification rules

| Class | When |
|---|---|
| `port` | The behaviour applies to the harness and no existing test asserts it |
| `covered` | An existing harness test asserts the same behaviour. `target` names it. Reason code `same`, or `partial` when all but a trivial remainder is asserted |
| `not-applicable` | One of the reasons below |
| `defer` | Applies, but depends on a feature a later stage delivers. The reason code is that stage |

`not-applicable` reason codes:

| Code | Meaning |
|---|---|
| `terminal-ui`, `http-server`, `browser-runtime`, `plugin-runtime`, `provider-catalogue`, `provider-keys`, `mobile-layout` | Something the harness does not have by design |
| `no-feature` | A product feature the harness does not have and that no stage plans. The reason names the feature, so the rows can be found again if a stage is ever given it |
| `reference-internal` | The reference agent's own build scripts, code generators, website and hosted services, test tooling or framework glue |

Rules:

- A row may not be classified `not-applicable` because porting it is hard.
- `no-feature` is not a waiting room. A feature some stage does plan is `defer` with that stage, and a `defer` row stays deferred to the stage that delivers the feature; it is not reclassified to close an earlier gate (D47).
- A row that asserts engine behaviour through a surface the harness lacks (the reference's server API, its terminal client) is classified by the behaviour, not by the surface.

## 4. How a test is migrated

1. Read the reference test to understand the behaviour. Write the behaviour sentence.
2. Write a new Rust test from that sentence against the harness's public API. Structure, fixtures and names are the harness's own.
3. Do not translate the reference test line by line, and do not copy its fixtures. Where a fixture is unavoidable data (a diff, a malformed stream), construct an equivalent.
4. No name of another project appears in the test, its module, its fixtures or its comments. The link to the source lives only in the catalogue.
5. If the test fails because the harness behaves differently, do not change the harness and do not weaken the test. Set `state` to `ledger`, add the difference to the parity ledger, and move on.

The reference agent is published under the MIT licence. This method does not rely on that: it carries over behaviour, not text. If a case ever needs reference material reproduced, it is called out for a licensing decision first.

## 5. Desktop application tests

The reference desktop application is a browser runtime with a native main process. Its tests split cleanly.

| Reference test subject | Harness equivalent | Target crate |
|---|---|---|
| Single-instance lock and second-launch hand-off | Named pipe or socket hand-off | Shell tests |
| Deep-link parsing and routing | Deep-link handling | Shell tests |
| Window state persistence and restore | Window placement | Shell tests |
| Installer and build configuration | Packaging inputs from the brand declaration | Brand and packaging tests |
| Auto-update flow | Deferred to S7, where the update slice arrives | — |
| Main-to-renderer messaging, context isolation, embedded-browser security settings | None: there is no embedded browser. `not-applicable` | — |
| Renderer components | The corresponding native view's state logic | Shell tests |

End-to-end specifications are migrated at the level they can be asserted without driving a real display:

- Where the behaviour is state (a queue of pending messages, a session list, a review panel's contents), assert it on the view's state type, which the shell derives from the store's change feed.
- Where the behaviour is visual (a border, a sticky header, a scrollbar), it becomes a state on the screenshot tour and is checked by a person, not asserted.
- Mobile and service-worker specifications are `not-applicable`.

## 6. Batches

Migration is split so several lanes work at once on disjoint paths (`../stages/s2-parity-and-proof.md`).

| Batch | Areas | Path | `port` rows |
|---|---|---|---|
| 1 | File tools, search, patch, shell | core test workspace, `migrated/tools/` | 67 |
| 2 | Agent loop, sessions, permissions, configuration | core, `migrated/session/`, `migrated/config/` | 157 |
| 3 | Model clients, MCP, LSP | core, `migrated/llm/`, `migrated/mcp/`, `migrated/lsp/` | 51 |
| 4 | Desktop process and views | template, `extensions/harness/tests/migrated/desktop/` | 645 |
| Later | Anything classified `defer` | With the stage that delivers the feature | 51 |

Batch 4 is seven tenths of the work: 560 of its rows need only a unit test on view-state logic, 83 need a display and so become tour states or stay with a person, 2 need the scripted instance. It does not fit one lane. It runs as successive lanes, one per group of catalogue areas, each on its own subdirectory of `migrated/desktop/`, in the order the stage file gives.

## 7. Keeping parity over time

- The pinned reference version is raised deliberately, by a person. Raising it reruns S0-A5's survey as a diff: new cases are added as `open` rows. Every surveyed area grew in the six days between the first survey and the pin, so the diff is routine work, not an exception.
- The stage gate fails if a `port` row is `open`.
- The catalogue's case count is asserted by the migrated suites, so a test that is deleted or stops running is noticed.
