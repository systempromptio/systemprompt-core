# Migrating the reference agent's tests

The reference agent ships a large test suite, including tests for its desktop application. Every behaviour those tests assert that applies to the harness is re-expressed as a Rust test, in core's test workspace for engine and device behaviour and in the template for shell behaviour, so parity is something the suite checks on every run.

## 1. What the reference suite contains

A first survey, taken from a downstream fork of the reference agent and therefore an **upper bound**: some files belong to the fork, not to the reference. Lane S0-A5 repeats it against the public repository at the pinned tag and replaces this table.

| Area | Test files (upper bound) | Kind | Applies to the harness |
|---|---|---|---|
| Application UI | 433 | Component and browser-condition unit tests | Partly: behaviour yes, web mechanics no |
| Application end-to-end | 60 specifications, in suites for regression (46), stability (16, not run in its own automation), reproduction (5), user story, service worker | Browser automation against a mock server and a mock terminal | The user-visible behaviours, yes; the service worker and mobile layouts, no |
| Core engine | 333 | Unit and integration: tools, providers, sessions, version control, configuration, worktrees | Mostly yes |
| Terminal UI | 200 | Unit | No: the harness has no terminal UI. Shared behaviours (keymaps aside) are checked case by case |
| Model clients | 170 | Unit, with recorded HTTP replayed from cassettes | Yes for the two protocols the gateway serves; provider-specific cases no |
| Session UI | 88 | Component | Behaviour yes |
| Desktop application | 69: main process (49), renderer (15), installer and build configuration (4), shared (1) | Unit | Yes for process behaviour; no for the embedded-browser mechanics |
| Command line | 58 | Unit | Where the harness has the same command |
| Server | 48 | Integration | No: the harness has no HTTP server |
| Others (plugins, UI kit, utilities, schema, SDK, protocol) | about 500 | Unit | Case by case |

The reference suite also contains two things worth learning from rather than porting: a recorder that replays real HTTP traffic from deterministic cassettes, which is the same idea as the scripted instance; and written rules for its end-to-end tests (no wall-clock waits; one keeper suite per product area; assert what the user sees). The second is adopted in `01-strategy.md` section 6.

## 2. The catalogue

One row per reference test case, produced by S0-A5 and maintained as a data file beside the migrated tests.

| Column | Content |
|---|---|
| `id` | Stable identifier |
| `area` | From the table above |
| `source` | Path and case name at the pinned tag |
| `behaviour` | One sentence, in the harness's own words, stating what must be true |
| `class` | `port`, `covered`, `not-applicable`, `defer` |
| `reason` | Required for every class except `port` |
| `target` | The Rust test that asserts it, once written |
| `needs` | `unit`, `scripted-instance`, `golden`, `display` |
| `state` | `open`, `written`, `passing`, `ledger` (fails because behaviour differs; see the parity ledger) |

A summary generated from the catalogue is published in this file when S2 closes: counts by area and class, and the list of deferred areas with reasons.

## 3. Classification rules

| Class | When |
|---|---|
| `port` | The behaviour applies to the harness and no existing test asserts it |
| `covered` | An existing harness test asserts the same behaviour. `target` names it |
| `not-applicable` | The behaviour concerns something the harness does not have by design: a terminal UI, an HTTP server, a browser runtime, a plugin runtime, a provider catalogue, stored provider keys, mobile layouts |
| `defer` | Applies, but depends on a feature a later stage delivers. Names the stage |

A row may not be classified `not-applicable` because porting it is hard.

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
| Auto-update flow | Deferred to S5, where the update slice arrives | — |
| Main-to-renderer messaging, context isolation, embedded-browser security settings | None: there is no embedded browser. `not-applicable` | — |
| Renderer components | The corresponding native view's state logic | Shell tests |

End-to-end specifications are migrated at the level they can be asserted without driving a real display:

- Where the behaviour is state (a queue of pending messages, a session list, a review panel's contents), assert it on the view's state type, which the shell derives from the store's change feed.
- Where the behaviour is visual (a border, a sticky header, a scrollbar), it becomes a state on the screenshot tour and is checked by a person, not asserted.
- Mobile and service-worker specifications are `not-applicable`.

## 6. Batches

Migration is split so several lanes work at once on disjoint paths (`../stages/s2-parity-and-proof.md`).

| Batch | Areas | Path |
|---|---|---|
| 1 | File tools, search, patch, shell | core test workspace, `migrated/tools/` |
| 2 | Agent loop, sessions, permissions, configuration | core, `migrated/session/`, `migrated/config/` |
| 3 | Model clients, MCP, LSP | core, `migrated/llm/`, `migrated/mcp/`, `migrated/lsp/` |
| 4 | Desktop process and views | template, `extensions/harness/tests/migrated/desktop/` |
| Later | Anything classified `defer` | With the stage that delivers the feature |

## 7. Keeping parity over time

- The pinned reference version is raised deliberately, by a person. Raising it reruns S0-A5's survey as a diff: new cases are added as `open` rows.
- The stage gate fails if a `port` row is `open`.
- The catalogue's case count is asserted by the migrated suites, so a test that is deleted or stops running is noticed.
