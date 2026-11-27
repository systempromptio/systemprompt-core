# Golden scenarios

Each directory here is one golden scenario: a whole session with a known script
and a known outcome. The design is
`documentation/future/harness/testing/03-golden-scenarios.md`; this file states
the file formats and every choice made where that design left a shape open.
The corpus is data. Nothing here runs a session.

```
<id>/
  scenario.toml      what it proves, who runs it, permissions, runner actions, end state
  workspace/         the fixture project
  prompt.txt         the user's messages
  turns.json         the model script
  verdicts.json      optional: scripted verdicts (harness-only scenarios)
  expected/
    trace.json       the normalised trace
    results/<call>.txt   the text behind each tool_result digest
    workspace/       the fixture project as it must be afterwards
schema/              JSON Schemas for scenario.toml, turns.json, verdicts.json, trace.json
corpus.toml          the number of scenarios the corpus must hold
validate.py          the offline validator
```

## Validating

```
python3 crates/tests/golden/scenarios/validate.py
```

Standard library only, Python 3.9 or later, no network, writes nothing. It
checks every file against its schema and then the cross-file rules below:
the trace follows the script call for call, every digest is the digest of the
text beside it, the workspace digest and changed list are those of
`expected/workspace/`, and the fixtures keep the hygiene rules. Exit status 1
on any failure. Pass scenario directories to check only those.

## The first set

| # | Id | Proves | Products |
|---|---|---|---|
| 1 | `01-answer-without-tools` | Request assembly, streaming text, session persisted, title generated | both |
| 2 | `02-read-file` | Tool call assembly (split arguments, two calls in one response), read limits, result returned | both |
| 3 | `03-edit-file` | Exact-match edit; file changed; snapshot taken | both |
| 4 | `04-edit-non-unique-match` | The tool refuses; the model is told why; nothing changes | both |
| 5 | `05-write-new-file` | Creation inside the workspace, parent directory included | both |
| 6 | `06-write-outside-workspace` | The path guard asks before leaving the project; the scripted answer refuses | both |
| 7 | `07-multi-file-patch` | A patch with one bad section applies nothing; the corrected patch adds, updates and deletes | both |
| 8 | `08-glob-and-grep` | The 100-result glob limit; a directory named in `.ignore` is not searched | both |
| 9 | `09-shell-success` | Shell selection; output captured and decoded | both |
| 10 | `10-shell-nonzero-exit` | Exit status and stderr reach the model | both |
| 11 | `11-shell-long-output` | 3,000 lines are truncated | both |
| 12 | `12-background-job-abort` | A background job is killed when the run is aborted | both |
| 13 | `13-denied-by-rule` | No execution; typed denial stored; reason returned | both |
| 14 | `14-approval-approved` | The ask is raised; approval runs the tool | both |
| 15 | `15-approval-rejected-feedback` | No execution; feedback returned to the model | both |
| 16 | `16-verdict-deny` | No execution; the verdict's reason returned | harness |
| 17 | `17-verdict-timeout` | A verdict slower than its budget lets the tool run | harness |
| 18 | `18-todo-list` | Todos stored and updated across turns | both |
| 19 | `19-subagent-task` | A child session runs with its own tools and reports back | both |
| 20 | `20-steering-mid-run` | A message sent during a run is admitted at the next step | both |
| 21 | `21-abort-mid-stream` | The run stops; the partial message is stored as aborted | both |
| 22 | `22-provider-error-then-success` | 429 and 500 retried; 401 refreshes the session once | both |
| 23 | `23-compaction` | Crossing the headroom triggers a summary with the fixed sections and a verbatim tail | both |
| 24 | `24-revert-to-message` | Files return to the snapshot; later messages removed | both |

Every scenario runs on both wire protocols from one `turns.json`.

## scenario.toml

Written in a subset of TOML so the validator needs no TOML library: one
`key = value` per line; `[table]`, `[table.sub]` and `[[array.of.tables]]`
headers; values are basic strings (escapes `\"`, `\\`, `\n`, `\t`), literal
strings, integers, booleans and one-line arrays of those; comments on their own
line. Where Python's `tomllib` exists the validator also parses with it and
refuses a file the two read differently.

| Key | Meaning |
|---|---|
| `id`, `number`, `title`, `proves`, `spec` | Identity. `id` is the directory name, `NN-slug`. |
| `products` | `harness`, `reference`. A scenario with `verdicts.json` is harness-only. |
| `protocols` | `messages`, `chat`. Always both in this set. |
| `agent` | Optional. `build` (the default) or `plan`. |
| `[permission] mode` | What becomes of a request the rules leave to the user: `ask` (answered from `[[ask]]`), `allow` (approved once each), `deny` (rejected each). The rules are the product's defaults for the agent plus `[[permission.rule]]`. Every scenario in this set uses `ask`. |
| `[[permission.rule]]` | Rules laid over the mode, last match wins: `permission` (`read`, `edit`, `shell`, `glob`, `grep`, `fetch`, `todo`, `task`, `external_directory`), `pattern` (`*` wildcard; a path is relative to the workspace), `action` (`allow`, `ask`, `deny`). The runner writes them into each product's own configuration, outside the compared workspace tree. |
| `[title_call] text` | The reply to the session-title model call. See "The title call". |
| `[[ask]]` | The scripted answers to asks, in the order the asks are raised: `tool`, `permission`, `reply` (`once`, `always`, `reject`), optional `feedback`. An ask with no entry left is a failure. |
| `[[action]]` | What the runner does during the run. See "Actions". |
| `[command.<name>]` | A shell command in three dialects. See "Placeholders". |
| `[end] exit` | How `run` must exit: `ok`, `aborted`, `failed`. Symbols, so the corpus does not fix numbers the headless entry owns; they stand for its exit codes 0, 130 and 1. |
| `[end] absent_outside` | Paths relative to the workspace root that must not exist afterwards and lie outside it. |
| `[harness]` | Assertions only the harness can meet, read from the store and the scripted instance's request log, never from the normalised trace: `title` (the stored session title), `verdicts` (the logical tool of each verdict request, in order: one per call the rules allowed or the user approved), `verdict_timeouts` (calls whose verdict outran its budget), `[[harness.denial]]` (`call`, `by`: the typed denial stored on the call: `policy`, `rule`, `rejected`, `aborted`), `patch_files` (files the session's stored step patches list, sorted: the snapshot was taken), `session_refreshes` (PAT-to-session exchanges after the first). |

### Actions

`on` names an observed event, never a wait: `ask` (the `index`-th ask was
raised; the action runs before that ask is answered), `text` (the first text
of the `index`-th model response was observed on the event stream), `idle`
(the run went idle for the `index`-th time). `do` is `steer` (send
`message` while the run is busy), `prompt` (send `message` as the next
prompt), `abort`, or `revert` (revert the session to user message
`message`: that message and everything after it are removed). Model
responses and user messages are counted from 1 over the main session.

### The title call

The harness names a session with a model call that runs beside the first
step, so its position among the scripted turns is not fixed. It is therefore
not a turn. `[title_call] text` is its reply: the scripted instance answers
with it any model request that advertises no tools, carries one user message
and precedes no scripted turn's guard. `turns.json` and the trace hold no
title call. Compaction requests also advertise no tools but arrive in order,
so they are ordinary turns.

## prompt.txt

The first message is the prompt. Further messages are separated by a line
holding only `---` and are numbered from 2; an action sends each. A message is
its lines without the final newline.

## turns.json and verdicts.json

Exactly the shapes `systemprompt-scripted-instance` loads (`ModelScript`,
`VerdictScript`); the schemas refuse unknown keys as those types do. Tool names
are the logical names `shell`, `read`, `write`, `edit`, `patch`, `glob`,
`grep`, `fetch`, `todo`, `task`, and tool input uses the harness's field
names as the logical ones; the per-product table beside the comparator maps
both. Call ids are `t1`, `t2`, ... in the main session and `c1`, ... in a
child session, unique in a scenario.

A turn with a `fault` has no `stop` and no `usage`. A turn may carry
`respond` together with `{"stall_ms": N}`: the instance sends the scripted
events, then holds the stream open for `N` milliseconds before its terminal
event. The two scenarios that use it abort while it is held, so `N` is never
waited out.

Scenario 23 scripts a usage of 2,000,000,000 input tokens on its first turn so
that the next step crosses the headroom of any context window. It needs the
model to be advertised with a non-zero window.

Scenario 22 answers 429, 500 and 401 before succeeding. Without a
`retry-after-ms` header on the faulted responses it costs the backoff in real
time (2 s, then 4 s).

## expected/trace.json

The form of the design's section 3, with the fields it left open settled here.

| Step | Fields |
|---|---|
| `model_request` | `protocol` (always the placeholder `{{protocol}}`), `tool_results` (the call ids whose results this request carries for the first time), optional `status` (the HTTP status when the response was an error), optional `purpose` (`compaction`; absent for a step). Title calls are not traced. |
| `assistant_text` | `digest` of the response's text, all text items joined. One per response that has text, directly after its `model_request`. For an aborted response, the digest of the text sent before the stall. |
| `tool_call` | `call`, `tool`, `input` as scripted. |
| `permission` | `call`, `tool`, `outcome`: the one decisive outcome for the call, however many checks ran: `allowed_by_rule`, `approved`, `approved_always`, `denied_by_rule`, `denied_by_verdict`, `rejected`. |
| `tool_result` | `call`, `tool`, `status` (`ok`: the tool ran and returned output, whatever a command's exit code; `error`: it refused its input or failed; `denied`; `rejected`; `aborted`), `digest` or `null`, optional `truncated: true`. |
| `subagent` | `agent`, `steps`: the child session's steps, between the `permission` and `tool_result` of the `task` call. |
| `user_message` | `delivery` (`steer`, `prompt`), `message` (the number in `prompt.txt`). The first prompt is not a step. |
| `compaction` | `auto`, `digest` of the summary text. Directly after its `model_request`. |
| `abort`, `revert` | The runner's action, where it took effect. `revert` carries `message`. |
| `finish` | `reason`: `end`, `aborted`, `error`, `stopped` (the user rejected a call without feedback). One per run, so a scenario with a second prompt has two. |

The three steps of a call are contiguous and calls of one response follow the
order the model issued them, whatever order they ran in.

`session.messages` counts conversation messages in the model's sense, so it
does not depend on how a product stores them: each user message (steering
included), each model response that was stored (an aborted one included), and
one per response whose tool calls produced results. Messages a product writes
itself (compaction boundary, summary, continuation) and messages hidden by a
revert are not counted, nor are a child session's. `session.state` is `idle`.
`session.todos` is the final todo list and `session.jobs` the final state of
each background job, where a scenario has them.

### Digests

A text digest is `sha256:` and the hex SHA-256 of the UTF-8 text after: CRLF
and CR to LF; spaces and tabs stripped from the end of every line; newlines
stripped from the end. A `tool_result` digest is taken over the output or
error text stored on the call, the text the model is sent. The text is kept in
`expected/results/<call>.txt` so a digest can be read and reviewed; the
validator refuses a digest that is not its file's.

`digest` is `null`, and there is no file, where the text depends on the
machine: the truncated output of scenario 11 (it names the spill file and a
byte count that differs with the line ending) and the background-job notice of
scenario 12 (job id, log path, shell name). The comparator skips a null digest.

The result texts were derived from the engine's source at the commit this
corpus was written against, not captured from a run. A first run that
disagrees is settled by a reviewed edit of the text file and its digest
(design section 7).

`workspace.tree_digest` is `sha256:` and the hex SHA-256 of the lines
`<path> NUL <hex SHA-256 of the content> LF`, one per file, sorted by path as
UTF-8 bytes; paths are relative with forward slashes; content has CRLF and CR
made LF when it is UTF-8 without a NUL byte, and is taken raw otherwise.
Directories are not entries, so a directory emptied by a delete does not count.
A product's own configuration directory is not part of the tree.
`workspace.changed` lists, sorted the same way, every path added, removed or
different from the fixture.

### Placeholders

`{{protocol}}` in the trace stands for the protocol the run used.

`{{command.<name>}}` in `turns.json` and the trace stands for the entry of
`[command.<name>]` for the shell family the product runs commands in: `posix`
(bash, sh, Git Bash), `powershell` (7 and 5.1) or `cmd`. The runner
substitutes before it loads the script and before it compares. Commands that
read the same in all three (`echo hello`) are written directly.

## What the runner must do for the traces to hold

- Copy `workspace/` to `<fresh directory>/workspace`, where the fresh
  directory is not under a location the product allows by default (its own
  temporary directory), so `../outside.txt` in scenario 6 is outside both.
- Set every copied file's modification time to one instant. Glob and grep list
  newest first and break ties by path; scenario 8 depends on the tie.
- Keep formatters and language servers out of the run. Fixtures are `.txt`
  for that reason; a product configuration that formats `.txt` would change
  the results.
- Apply `[[permission.rule]]` outside the workspace tree, or exclude the file
  it writes from the tree digest.

## Fixture rules

Text only, UTF-8, LF, a final newline, at most 16 KB a file; no timestamps and
no absolute paths. `.gitattributes` here pins `eol=lf`, so a Windows checkout
has LF fixtures; where one arrives with CRLF anyway the digests above are
unchanged, because both normalise line endings. A fixture that needs ignore
rules uses `.ignore`: a `.gitignore` inside a fixture would hide its files
from this repository. For the same reason no fixture path uses a name the
repository's own `.gitignore` matches (`docs/`, `target/`, ...): check a new
fixture with `git status` before relying on it. File names avoid the words the
repository hygiene gate refuses.

## Changing a scenario

An expected trace changes only by a reviewed edit that says why. There is no
regeneration command.
