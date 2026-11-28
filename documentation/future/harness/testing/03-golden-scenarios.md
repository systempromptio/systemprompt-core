# Golden scenarios and the parity comparison

A golden scenario is a whole session with a known script and a known outcome. The same scenario runs through the harness and through the reference agent; the two outcomes are normalised and compared.

## 1. Anatomy of a scenario

```
golden/scenarios/<id>/          (core's test workspace)
  scenario.toml      id, title, what it proves, permission mode, which products run it
  workspace/         the fixture project the session works in
  prompt.txt         the user's message (and any follow-up or steering messages)
  turns.json         the model script (02-scripted-instance.md section 3)
  verdicts.json      optional: scripted verdicts, a default and one per tool (02-scripted-instance.md section 2)
  expected/
    trace.json       the normalised trace (section 3)
    workspace/       the fixture project as it must be afterwards
```

## 2. The first set

Twenty-four scenarios. Each runs on both wire protocols.

| # | Scenario | Proves |
|---|---|---|
| 1 | Answer without tools | Request assembly, streaming text, session persisted, title generated |
| 2 | Read a file | Tool call assembly, read limits, result returned to the model |
| 3 | Edit a file | Exact-match edit; file changed; snapshot taken |
| 4 | Edit with a non-unique match | The tool refuses; the model is told why; nothing changes |
| 5 | Write a new file | Creation inside the workspace |
| 6 | Write outside the workspace | Refused by the path guard |
| 7 | Multi-file patch | Atomic application; partial failure leaves nothing applied |
| 8 | Glob and grep | Search limits; ignore rules honoured |
| 9 | Shell command, success | Shell selection; output captured and decoded |
| 10 | Shell command, non-zero exit | Exit status and stderr reach the model |
| 11 | Shell command, long output | Truncation with head and tail kept |
| 12 | Background job, then abort | The process tree is killed |
| 13 | Tool denied by a permission rule | No execution; typed denial stored; reason returned |
| 14 | Tool needing approval, approved | The ask is raised; approval runs the tool |
| 15 | Tool needing approval, rejected with feedback | No execution; feedback returned to the model |
| 16 | Tool denied by the instance verdict | No execution; the verdict's reason returned. Harness only |
| 17 | Verdict slower than its budget | The documented behaviour on timeout. Harness only |
| 18 | Todo list | Todos stored and updated across turns |
| 19 | Subagent task | A child session runs with its own tools and reports back |
| 20 | Steering mid-run | A message sent during a run is admitted at the next step |
| 21 | Abort mid-stream | The run stops; the partial message is stored as aborted |
| 22 | Provider error then success | `429` and `5xx` retried with backoff; `401` refreshes the session once |
| 23 | Compaction | Crossing the headroom triggers a summary with the fixed sections and a verbatim tail |
| 24 | Revert to a message | Files return to the snapshot; later messages removed |

Later sets, added by the stages that need them: an MCP tool over stdio and over HTTP against a local test server; a fetch against a local server; LSP diagnostics after an edit against a stub language server; managed skills, rules and a managed MCP server arriving by manifest; each managed host's profile (S6).

## 3. The normalised trace

Both products' transcripts are reduced to one form before comparison. Everything not listed is dropped.

```json
{
  "steps": [
    { "kind": "model_request", "protocol": "messages", "tool_results": [] },
    { "kind": "tool_call", "tool": "read", "input": { "path": "src/lib.rs" } },
    { "kind": "permission", "tool": "read", "outcome": "allowed_by_rule" },
    { "kind": "tool_result", "tool": "read", "status": "ok", "digest": "sha256:…" },
    { "kind": "model_request", "protocol": "messages", "tool_results": ["t1"] },
    { "kind": "assistant_text", "digest": "sha256:…" },
    { "kind": "finish", "reason": "end" }
  ],
  "workspace": { "tree_digest": "sha256:…", "changed": ["src/lib.rs"] },
  "session": { "messages": 4, "state": "idle" }
}
```

Normalisation rules:

| Field | Rule |
|---|---|
| Tool names | Mapped to a logical name through a per-product table kept beside the comparator |
| Tool input | Mapped field by field through the same table; paths made relative to the workspace root with forward slashes |
| Tool output | Reduced to status and a digest of the content after line-ending and trailing-whitespace normalisation |
| Identifiers, timestamps, durations, token counts | Dropped |
| System prompt and tool descriptions | Dropped from the trace; kept in the request log for side-by-side reading |
| Workspace | A digest over relative paths and file contents, line endings normalised |
| Platform | Path separators and line endings normalised so one expected trace serves Windows and macOS |

## 4. Running a scenario through the harness

Through the headless `run` command added in S2-A2, which uses the same engine and tools as the window. At S2 that is the only mode. From S3 every scenario runs in both local modes, and from S4 in all three execution modes (`Unrestricted`, `Sandboxed`, `Remote`) with the same scenario files and the same expected traces, which is what proves one loop serves all three. Parity with the reference agent is measured in `Unrestricted`.

1. Start the scripted instance with the scenario's manifest, turns and verdicts.
2. Create a fresh data root; sign in with `login --stdin` and the instance's test PAT.
3. Copy the fixture workspace to a temporary directory.
4. `run` with the prompt, the workspace, the permission mode and scripted answers to asks; read the event stream.
5. Normalise the events, the store and the workspace; compare with `expected/`.
6. Assert the scripted instance has no unconsumed turns and recorded no unscripted request.
7. For harness-only assertions: check the request log for signing headers, the verdict call before each tool, and the telemetry events.

## 5. Running a scenario through the reference agent

In the parity job only, on Linux:

1. Install the reference agent from its public release at the pinned version (tag `v2.4.2`, D47): the release of that tag, not the package registry's `latest`, which is on the older release line.
2. Start the scripted instance's binary. The reference agent needs only the model routes; it is configured with a custom provider whose base URL is the scripted instance.
3. Copy the fixture workspace; run the reference agent headless with the same prompt and an equivalent permission configuration.
4. Export its session in its native format; normalise with the reference table.

The exact command line, configuration keys and export format are confirmed by lane S2-A4 from the reference agent's own documentation at the pinned version, and recorded beside the runner. They are not assumed here.

Scenarios 16 and 17 are not run against the reference agent: it has no instance verdict.

## 6. The parity ledger

The comparator writes one row per scenario and protocol, and one row per difference.

| Classification | Meaning | Gate |
|---|---|---|
| `same` | Normalised traces and workspaces are equal | — |
| `intended` | A difference that follows from the harness's design: governance, the permission surface, a tool the harness does not offer, a stricter guard. Carries a written reason | Reason required |
| `gap` | The reference agent does something useful the harness does not. Carries an owner stage | Owner required |
| `defect` | The harness is wrong | Must be fixed to close S2 |
| `reference-defect` | The reference agent is wrong and the harness is right. Carries the evidence | — |
| `unclassified` | Not yet judged | None may remain at a stage gate |

The ledger is generated on every L2 run, printed to the run's summary, and written into the evidence folder. Classifications and their reasons are kept in a reviewed file beside the comparator; a difference that is not in that file is `unclassified` and fails the gate. The counts by classification are published here when S2 closes.

## 7. Changing an expected trace

An expected trace changes only by a reviewed edit that states why. There is no bulk regeneration command in the repository.

## 8. Ledger counts, 2026-11-28

Read at core commit `6d079361b8`, from the scenario corpus (`golden/scenarios/`), the comparator (`parity/compare/`) and its reviewed file (`parity/compare/classifications.toml`). No session was run to produce this section. These are scenario and ledger rows; the per-case counts of the migrated tests are in `04-test-migration.md` section 1.2, printed by `scripts/parity/count-cases.py`.

**What the ledger holds.** One row per scenario, protocol and comparison. `harness-vs-expected` compares a harness run with `expected/`; `harness-vs-reference` compares it with the reference agent's normalised export. A row's result is `same`, `differs` (one entry per difference, each `intended`, `gap`, `defect`, `reference-defect` or `unclassified`) or `error` (the run could not be judged).

| Count | Value |
|---|---|
| Scenarios | 24 |
| Protocols per scenario | 2 (`messages`, `chat`) |
| `harness-vs-expected` rows | 48 |
| `harness-vs-reference` rows | 44 (scenarios 16 and 17 are harness only) |
| Rows in a full ledger | 92 |
| Reviewed classifications | 0: `intended` 0, `gap` 0, `defect` 0, `reference-defect` 0 |

**Classified differences.** None, for any scenario or protocol. The reviewed file holds no entry, so every row of a ledger generated at this commit is either `same` or carries `unclassified` differences, and no `defect` is open because none is recorded.

**Matching and unclassified.** Not published, because no generated ledger exists to read. `ledger.json` and `ledger.md` are run output; none is tracked in either repository. What the sources fix:

- The golden test fails a scenario on any unclassified `harness-vs-expected` difference. With no reviewed entry, a green golden run on a system therefore means all 48 of those rows are `same` on that system, and a red one names the scenarios and paths that are not.
- The `harness-vs-reference` rows are written only by the `parity-ledger` binary when it is given the reference export. No workflow or script in core at this commit starts that binary, and nothing under the template's evidence script or tour does at template commit `63cf625e`; the parity workflow runs the reference agent and uploads its export, and stops there. Those 44 rows have no result until a job joins the export to the golden runs.

Per scenario, each on both protocols:

| # | Scenario | `harness-vs-expected` rows | `harness-vs-reference` rows | Classified differences | Matching | Unclassified |
|---|---|---|---|---|---|---|
| 1 | `01-answer-without-tools` | 2 | 2 | 0 | not recorded | not recorded |
| 2 | `02-read-file` | 2 | 2 | 0 | not recorded | not recorded |
| 3 | `03-edit-file` | 2 | 2 | 0 | not recorded | not recorded |
| 4 | `04-edit-non-unique-match` | 2 | 2 | 0 | not recorded | not recorded |
| 5 | `05-write-new-file` | 2 | 2 | 0 | not recorded | not recorded |
| 6 | `06-write-outside-workspace` | 2 | 2 | 0 | not recorded | not recorded |
| 7 | `07-multi-file-patch` | 2 | 2 | 0 | not recorded | not recorded |
| 8 | `08-glob-and-grep` | 2 | 2 | 0 | not recorded | not recorded |
| 9 | `09-shell-success` | 2 | 2 | 0 | not recorded | not recorded |
| 10 | `10-shell-nonzero-exit` | 2 | 2 | 0 | not recorded | not recorded |
| 11 | `11-shell-long-output` | 2 | 2 | 0 | not recorded | not recorded |
| 12 | `12-background-job-abort` | 2 | 2 | 0 | not recorded | not recorded |
| 13 | `13-denied-by-rule` | 2 | 2 | 0 | not recorded | not recorded |
| 14 | `14-approval-approved` | 2 | 2 | 0 | not recorded | not recorded |
| 15 | `15-approval-rejected-feedback` | 2 | 2 | 0 | not recorded | not recorded |
| 16 | `16-verdict-deny` | 2 | 0 (harness only) | 0 | not recorded | not recorded |
| 17 | `17-verdict-timeout` | 2 | 0 (harness only) | 0 | not recorded | not recorded |
| 18 | `18-todo-list` | 2 | 2 | 0 | not recorded | not recorded |
| 19 | `19-subagent-task` | 2 | 2 | 0 | not recorded | not recorded |
| 20 | `20-steering-mid-run` | 2 | 2 | 0 | not recorded | not recorded |
| 21 | `21-abort-mid-stream` | 2 | 2 | 0 | not recorded | not recorded |
| 22 | `22-provider-error-then-success` | 2 | 2 | 0 | not recorded | not recorded |
| 23 | `23-compaction` | 2 | 2 | 0 | not recorded | not recorded |
| 24 | `24-revert-to-message` | 2 | 2 | 0 | not recorded | not recorded |
| | **Total** | 48 | 44 | 0 | not recorded | not recorded |

Exit criterion 3 of `../stages/s2-parity-and-proof.md` is met for the classified counts only. The matching and unclassified counts are filled in from the first `ledger.md` an L2 run or an evidence run writes.
