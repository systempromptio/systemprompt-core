# Reference runner

Runs each golden scenario (`crates/tests/golden/scenarios`) through the
reference agent, with the scripted instance as its model provider, and writes
what the agent did in the agent's own formats. The comparator
(`crates/tests/parity/compare`) reads the output. The design is
`documentation/future/harness/testing/03-golden-scenarios.md` section 5.

The reference agent is named in `.github/workflows/parity.yml` and in
`documentation/future/harness`, and nowhere here. This directory calls it "the
agent"; every product-specific name the runner needs is derived from the file
name of the executable it is given, written `<agent>` below.

## Files

| File | What it is |
|---|---|
| `run.py` | The runner. One process, standard library only, Python 3.9 or later |
| `scenario.py` | Reads a scenario; re-expresses `turns.json` and the permission rules in the agent's names |
| `tools.json` | The table for that: logical tool, input field and permission names to the agent's |
| `agent.py` | The agent as a process: its environment, its configuration file, its headless server |
| `relay.py` | A recording relay between the agent and the scripted instance |
| `sse.py` | Stream encoders for the two wire protocols, used by the relay and the stub |
| `network_guard.py` | Reads a system-call trace of a run and fails on any address outside loopback |
| `tests/test_runner.py` | Tests of the parts above; they start no agent |
| `tests/stub_instance.py` | A **stub** of the scripted instance's binary, model routes only, for the tests and for work without cargo |

## Running

```
python3 crates/tests/parity/reference/run.py \
  --agent <path to the agent's executable> \
  --instance <path to systemprompt-scripted-instance> \
  --scenarios crates/tests/golden/scenarios \
  --out <output directory> [--work <scratch directory>] \
  [--only <id or number prefix>]... [--protocol messages|chat]... [--limit <seconds>] [--keep-work]
```

`--instance` takes the executable and any leading arguments, so the stub is
`--instance python3 crates/tests/parity/reference/tests/stub_instance.py`.
The runner installs and downloads nothing. It exits 1 when a run could not be
exported, when no scenario was selected, or when the corpus does not hold the
number of scenarios `corpus.toml` pins; a run that was exported but departed
from the script is a finding for the comparator, not a failure here.

```
python3 -m unittest discover -s crates/tests/parity/reference/tests
```

## The side-by-side comparison

One command runs both products over the corpus and writes the comparison:

```
scripts/parity/side-by-side.sh --out <dir> [--cache <dir>] [--agent <path>] \
  [--reference <dir>] [--only <id or number>]... [--protocol messages|chat]... [--limit <seconds>]
```

It installs the agent at the workflow's pin (the tag, asset and SHA-256 are
read from `.github/workflows/parity.yml` and the archive is verified; it is
kept under `--cache`), builds the scripted instance and `parity-ledger`, runs
this runner into `<out>/reference`, runs the golden runner for the harness's
side and copies its runs to `<out>/harness`, and runs `parity-ledger` with
`compare/classifications.toml`. `--agent` takes an executable of the pinned
version and skips the download, which is needed anywhere but Linux x64;
`--reference` takes an export that already exists and skips this runner, as
the workflow does. It needs no model credential and refuses to start when the
environment holds a provider variable. The workflow's network namespace and
trace are not applied.

| Output | What it is |
|---|---|
| `<out>/ledger.json` | `totals` per comparison (runs by verdict, differences by class) and one row per scenario, protocol and comparison with its `verdict`: `same`, `intended`, `gap`, `defect`, `reference-defect`, `unclassified`, or `error` |
| `<out>/ledger.md` | The same as the workflow's summary prints it |
| `<out>/traces/<scenario id>/<protocol>/` | `reference.json` and `harness.json`, the two normalised traces compared |
| `<out>/side-by-side.html` | One self-contained page: the totals, a row per run, then each run's two traces beside each other with the differing fields marked and each difference's class and reason |
| `<out>/summary.tsv` | The page's table, one line per scenario and protocol |

It exits 0 when every difference is classified and none is a defect, 1 when a
difference is unclassified, a defect is open or a run could not be compared,
and 2 for a usage or environment problem.

## How the agent is installed and pinned

By the workflow, not by the runner: the Linux x64 archive of the release of the
pinned tag, downloaded by its release URL and checked against a SHA-256 written
in the workflow, which is that asset's entry in the release's own checksum
file. Never the package registry's `latest`. The runner records the version
the executable prints and the SHA-256 of the executable in `index.json`, and
the workflow fails if the version is not the pinned tag.

## What one run does

For each scenario that lists the `reference` product, once per protocol it
lists:

1. Copies `workspace/` to `<scratch>/<id>/<protocol>/project/workspace`, sets
   every file's modification time to one instant, and makes the copy a git
   repository with one commit. The agent takes file snapshots only inside a
   repository, and revert needs them. `.git` is not part of the compared tree.
2. Translates `turns.json` with `tools.json` and resolves
   `{{command.<name>}}` to the `posix` dialect. The scripted instance
   compares tool names literally, so it is given the agent's names.
3. Starts the scripted instance:
   `systemprompt-scripted-instance --port 0 --script <translated> --messages-model scripted-model`
   (or `--chat-model`), and reads its `base_url` and `pat` line.
4. Starts the relay in front of it (below).
5. Writes the agent's configuration and starts
   `<agent> serve --hostname 127.0.0.1 --port 0 --print-logs` in the workspace,
   with an environment built from nothing.
6. Creates a session, sends the prompt, and follows the server's event stream:
   answers each permission request from `[[ask]]`, performs each `[[action]]`
   on its event, and stops when the session has gone idle with no action left.
7. Stops the server, exports the session with `<agent> session export <id> --standalone`,
   and copies the workspace out.

Scenarios that do not list the `reference` product are not run. They are
entered in `index.json` as `skipped` with the reason: in the first set these
are 16 and 17, which script an instance verdict the agent does not have.

### The agent's environment

`HOME` and the four `XDG_*` directories point into the run's scratch
directory, so nothing is read from or written to the user's own configuration,
and `TMPDIR` is beside them, so `../outside.txt` is in neither the workspace
nor the agent's temporary directory. `PATH` is the caller's. No other variable
of the caller's is passed: no provider variable can reach the agent. Three of
the agent's switches are set, each `<AGENT>_` followed by
`DISABLE_AUTOUPDATE`, `DISABLE_MODELS_FETCH` (it does not fetch its model
catalogue) and `DISABLE_CLAUDE_CODE` (it does not read another product's
files from the home directory). The switches are read by the pinned executable
but are not on its documentation pages; the documented `update` key is set as
well.

### The agent's configuration

Written to `$XDG_CONFIG_HOME/<agent>/<agent>.json`, the agent's global
configuration file, so no file is added to the workspace.

| Key | Value | Why |
|---|---|---|
| `providers.scripted.package` | `@<agent>/ai/providers/anthropic-compatible` for `messages`, `.../openai-compatible` for `chat` | The agent's two runtimes for a custom endpoint; they call `/v1/messages` and `/v1/chat/completions` |
| `providers.scripted.settings.baseURL` | The relay's address followed by `/v1` | Loopback; the runner refuses any other host |
| `providers.scripted.settings.apiKey` | The scripted instance's test token (`pat`) | The instance accepts it as a bearer token or `x-api-key`. It is not a provider credential |
| `providers.scripted.models.scripted-model.limit` | `context` 200000, `output` 8192 | What the scripted instance advertises; scenario 23 needs a window |
| `model` | `scripted/scripted-model` | |
| `permissions` | `[[permission.rule]]` as `{action, resource, effect}`, in order | The agent's ordered rules, last match wins. Nothing else is added: the agent's own defaults stand, as the corpus specifies |
| `update` | `disable` | No update check |
| `formatter` | `false` | Tool results are not reformatted |
| `snapshots` | `true` | The default, stated |
| `experimental.todo` | `true` | The agent's todo tool is off unless this is set; scenario 18 scripts it |

`[permission] mode` is applied by the runner when it answers: `ask` answers
from `[[ask]]` and rejects a request that has no answer left, recording a
finding; `allow` approves each once; `deny` rejects each.

### The agent's server interface

All on the loopback address the server prints, with HTTP basic authentication:
user `<agent>`, password the one the server prints at start.

| Use | Request |
|---|---|
| Events | `GET /api/event` (server-sent events) |
| Create the session | `POST /api/session` with `location.directory`, `model`, `agent` |
| Prompt, steer | `POST /api/session/{id}/prompt` with `text`, and `delivery: "steer"` for a steer |
| Answer an ask | `POST /api/session/{id}/permission/{request}/reply` with `decision` (`once`, `always`, `reject`) and `message` |
| Abort | `POST /api/session/{id}/interrupt` |
| Revert | `POST /api/session/{id}/revert/stage` with `messageID` and `files: true`, then `POST .../revert/commit` |
| Background jobs | `GET /api/experimental/job` |

Events the runner acts on: `permission.asked`; `session.step.started` (one
per model response) and `session.text.started` for `on = "text"`;
`session.execution.succeeded`, `.failed` and `.interrupted` for `on = "idle"`
and the end; `session.created` with a `parentID` for child sessions. A child
session runs in the background and wakes its parent when it finishes, so where
a session has children the runner counts the parent's stop only once no
session of the run is active and the event stream has been quiet for a second
(ten where scripted turns are still unconsumed).

Sources at the pinned version: the agent's documentation pages for the command
line, configuration, providers, permissions, tools and snapshots, its published
HTTP interface description, and `--help` of the executable.

## The relay

The scripted instance's binary prints its address and serves. It gives a
separate process no way to read its request log, and no way to script the
session-title call, which the agent makes beside its first step. The relay
stands between the two for those reasons and changes nothing else:

- It records every request the agent sends to its provider, in arrival order.
- A request that advertises no tools and carries one user message and no
  earlier turn is the title call (the corpus's rule). The relay answers it
  with `[title_call] text`, so it takes no scripted turn. `turns.json` holds
  no title call.
- Every other request goes to the scripted instance unchanged and its answer
  is streamed back as it arrives.

It listens on loopback and refuses an upstream that is not loopback.

## Output layout

```
<out>/
  index.json                    one entry per scenario and protocol
  <scenario id>/<protocol>/
    session.json                the agent's native export of the session
    session.before-revert.json  the same, taken just before a revert action, when there is one
    children/<session id>.json  the same for each child session, when there are any
    workspace/                  the workspace as the agent left it (no .git, no agent directory)
    outside/                    whatever appeared beside the workspace (scenario 6)
    requests.jsonl              the model requests, in order, and how each was answered
    events.jsonl                the agent's event stream, as received
    jobs.json                   the agent's background jobs at the end
    run.json                    what the runner did and saw
    script.json                 the translated script the scripted instance loaded
    agent-config.json           the configuration, with the token replaced
    agent.log, instance.log     standard error of the two processes
```

`<protocol>` is `messages` or `chat`.

**`index.json`**: `agent.version`, `agent.sha256`, `scenarios_in_corpus`,
`counts` by status, `problems`, and `runs`: `scenario`, `number`, `protocol`,
`status`, and `directory`, `reason`, `findings` where they apply. `status` is
`exported`, `skipped` (with `reason`) or `error` (no usable export; `reason`
says why).

**`session.json`** is the output of `<agent> session export`: an object with
`info` (the session: `id`, `title`, `model`, `outcome`, `tokens`, `location`)
and `messages`, an ordered list. A message has `type`: `user` (`text`),
`assistant` (`content`, a list of `text` items and `tool` items; a tool item
has `id`, `name` and `state` with `status`, `input`, and `content` or an
error; plus `finish`, `rawFinish`, `tokens` and `snapshot` with the files the
step changed), and `idle` (`outcome`) where a run ended. Tool names and input
fields are the agent's; `tools.json` maps them back. Call ids are the
script's (`t1`, ...).

**`requests.jsonl`**, one object a line: `sequence`, `method`, `path`,
`protocol`, `headers` (credentials replaced), `body` (the request as sent,
system prompt and tool schemas included), `status`, `response` (the stream or
body returned), `complete` (false when the agent closed the stream first),
`answered_by` (`scripted-instance` or `relay`), `title_call`, `unscripted`
(the instance refused it: no turn left, or the turn's guard failed).

**`run.json`**: `session`, `children`, `user_messages` (message number in
`prompt.txt` to the agent's message id), `asks` (each request as raised, the
scripted answer and the reply sent), `actions` performed, `stops` (the outcome
of every stop of the main session: `succeeded`, `failed`, `interrupted`),
`idles` (the stops that counted, leaving out a pause before a child session
reported back), `revert`, `relay` (`model_requests`, `title_calls`,
`unscripted`, `turns_total`, `turns_consumed`, `turns_unconsumed`),
`findings`, and `failure` when the run did not complete.

`findings` are departures from the script, stated in words: a model request
with no scripted turn, scripted turns never requested, a scripted answer never
asked for, an ask with no answer left, an action whose event never came. They
are the comparator's to classify.

## The network guard

The runner cannot reach a provider by construction: the agent's only provider
is the relay, the relay forwards only to loopback, and the agent's environment
holds no credential. The workflow adds the check the strategy asks for
(`testing/01-strategy.md` section 5): the run happens inside a network
namespace that has loopback and nothing else, under
`strace -f -e trace=connect,sendto,sendmsg`, and `network_guard.py` then fails
the job if the trace names any address outside loopback, or any address on
the DNS port, or holds no loopback connection at all.
