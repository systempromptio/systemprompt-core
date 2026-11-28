# The scripted instance

A test-support crate that stands in for a systemprompt instance and for the model behind it. It serves exactly what a test scripts and records exactly what it receives.

Location: core's test workspace. It is a library, started in-process by a test on an ephemeral loopback port, and also a small binary so the reference agent and the packaged app can be pointed at it. The template's tour uses that binary, built from the core checkout the template is patched to.

Starting point for the model half: core's `mock-inference` test crate, which already serves fixed responses in both wire protocols with latency and failure modes. That crate is built for load testing and returns one fixed reply; this one is scripted per request. They may later share the stream encoders.

## 1. What it serves

| Surface | Routes | Behaviour |
|---|---|---|
| Discovery and health | The well-known bridge document; health | Static, from the builder |
| Sign-in | The device-link URL, code exchange, PAT to session | Issues test tokens signed with a key the builder generates. The loopback-callback and paste-code flows both complete without a browser |
| Identity | `whoami` | The scripted user |
| Manifest | The signed manifest, with `ETag` and `304` | Signed with the builder's Ed25519 key over the canonical payload string. The builder can serve a stale version, a future `not_before`, a wrong user, a changed key, or a bad signature, to exercise acceptance and key trust |
| Model list | `/v1/models` and the profile | From the builder. The manifest's `host_model_protocols` names the client families of the models (`claude`, `openai`, `gemini`), as core's gateway writes it; the profile lists one configured provider per family serving that family's models |
| Model calls | `/v1/messages`, `/v1/chat/completions` | Scripted streams (section 3) |
| Hook token | The OAuth client and `client_credentials` exchange | Test tokens |
| Telemetry | `hooks/track` | Recorded |
| Verdict | `hooks/govern` | Scripted per tool: allow, deny with a reason, delay beyond the budget, or error |
| Heartbeat | The heartbeat route | Recorded |
| Device enrolment | `POST` the bridge device route | Issues a device credential (`Route::DeviceEnrolment`); plans, receipts and session bindings take it |
| Bundles and runtime files | Plugin bundle fetch; plans; receipts; session bindings | Bundle files from the manifest fixture's plugins; a plan for a published plugin, for the harness host, laid out as core's marketplace lays one out; receipts and bindings recorded and acknowledged (a repeat answers as the first did) |
| Documents | The Drive routes the harness uses: the sync feed, and the browsing listing of documents and folders that tells the app the instance has a Drive | An in-memory store |

Routes and headers are taken from the linked core crates' constants, never spelled again here. Section 2.3 lists the four that have no constant yet.

## 2. Interface

Fixed by S0-B2 as crate `systemprompt-scripted-instance` in core's test workspace (`crates/tests/scripted-instance`), a library and a binary of the same name. S2-A1 fills in behaviour behind these signatures; it does not change them without a hand-off request.

```rust
let instance = ScriptedInstance::builder()
    .user(TestUser::default())
    .manifest(ManifestFixture::minimal())
    .models([ModelFixture::messages("m-a"), ModelFixture::chat("m-b")])
    .verdict(VerdictScript::allow_all())
    .script(ModelScript::from_file("scenarios/edit-file/turns.json")?)
    .start()
    .await?;

instance.base_url();                         // "http://127.0.0.1:<port>", no trailing slash
instance.pat().as_str();                     // for non-interactive sign-in
instance.requests();                         // everything received, in order
instance.unscripted_requests();              // requests no turn answered
instance.unconsumed_turns();                 // turns no request reached
instance.wait_for(Event::Heartbeat).await?;  // the matching record
instance.shutdown().await?;                  // dropping it also stops the server
```

Where the fixed interface differs from the sketch this file first carried:

| Sketch | Fixed | Why |
|---|---|---|
| `ModelScript::from_file(path)` returns the script | Returns `Result`; `from_json` reads the same shape from a string. `VerdictScript` has both too | A missing or malformed scenario file is a typed error at the call site, not a panic inside the builder |
| `wait_for(event)` returns nothing | Returns `Result<RequestRecord, ScriptedInstanceError>` | The test gets the record it waited for, and a wait that cannot be met is an error (`WaitTimedOut`) |
| `pat()` returns the token text | Returns `&TestPat`; `.as_str()` gives the text | The fixture token is a type; it is built from core's API-key prefix so the device's own token validation accepts it |

The types:

| Type | Shape |
|---|---|
| `ScriptedInstanceBuilder` | `user`, `pat`, `manifest`, `models`, `verdict`, `script`, `title_reply` (section 3.1), `document(path, bytes)` (the Drive store starts holding it; the change feed records it as created), `port` (default 0, an ephemeral port; always loopback), `start` |
| `ScriptedInstance` | The calls above, plus `addr` and read access to each fixture it was built with |
| `TestUser` | Typed user id, optional tenant id, email, display name, roles |
| `ManifestFixture`, `ManifestFault` | `minimal()`, `with_fault`, `with_plugin`, `with_extension` (a host extension listed for the harness host). Faults: `StaleVersion`, `FutureNotBefore`, `WrongUser`, `ChangedKey`, `BadSignature`; each is also taken by its hyphenated name (`stale-version`, and so on) |
| `PluginFixture` | `new(id)`, `governance(id)` (owns the governance hooks, so the harness mints a hook token and asks for verdicts), `file(path, bytes)`, `published()` (a managed publication: the manifest entry names it and its plan is served). The entry's digests are derived from the files; a plugin manifest is served when the fixture brings none |
| `ModelFixture` | A typed model id and a wire protocol (core's `WireProtocol`; there is no local protocol enum); `messages(id)`, `chat(id)`, `on(id, protocol)`, `named(display)`, `context_window(tokens)`, `family()` (core's `ClientFamily` for the protocol) |
| `VerdictScript`, `Verdict` | A default verdict and a map from logical tool name to verdict. `Allow`, `Deny { reason }`, `Delay { millis }`, `Error { status }`. File shape: `{"default": <verdict>, "tools": {"<name>": <verdict>}}` |
| `ModelScript`, `Turn`, `Expect`, `Respond`, `ToolCall`, `StopReason`, `Usage`, `TurnFault`, `Aside` | Section 3 |
| `RequestRecord`, `RecordedBody`, `Route` | Sequence number, route, method, path, query, headers, body (`Empty`, `Json`, `Bytes`), whether it was unscripted, and the `purpose` of a model call answered beside the turns (`Aside::TITLE` for a title call). `Route` is a closed enum of the surfaces in section 1 (`DeviceEnrolment`, `PluginBundle`, `Plan`, `Receipt`, `SessionBinding`, `Drive` among them), with `Unrouted` for anything else |
| `Event` | `Heartbeat`, `Telemetry`, `Verdict`, `ModelCall`, `ManifestFetch`, `Receipt`, `Route(Route)` |
| `ScriptedInstanceError` | Typed: bind, serve, join, read, parse, `NotImplemented`, `WaitTimedOut` |

Every scripted structure refuses unknown keys, so a misspelt key in a scenario file fails the test that loads it.

`requests()` returns typed records in arrival order. Tests assert on them: that a model call carried the session's signing headers and the user id in its metadata; that a verdict call preceded a tool's execution; that telemetry events arrived in order.

A verdict is scripted **per tool, not per call**. A scenario that needs a sequence of verdicts for one tool needs a wider type; the lane that writes the first such scenario requests it.

The binary is `systemprompt-scripted-instance`. It prints one JSON line with `base_url` and `pat` on standard output, logs to standard error, and serves until interrupted. Every switch is optional and maps to one builder call:

| Switch | Builder call | Effect |
|---|---|---|
| `--port N` | `port` | The loopback port; 0, the default, takes an ephemeral one |
| `--script turns.json` | `script(ModelScript::from_file(..))` | The turns and asides (section 3) |
| `--verdicts verdicts.json` | `verdict(VerdictScript::from_file(..))` | The verdict per tool; without it every tool is allowed |
| `--messages-model ID` (repeatable) | `models` with `ModelFixture::messages` | A model on the Messages protocol |
| `--chat-model ID` (repeatable) | `models` with `ModelFixture::chat` | A model on the Chat Completions protocol |
| `--context-window TOKENS` | `ModelFixture::context_window` on every model | The context window advertised for every model |
| `--title-reply TEXT` | `title_reply` | The reply to every session-title call (section 3.1) |
| `--governance-plugin ID` | `ManifestFixture::with_plugin(PluginFixture::governance(..))` | The manifest lists a plugin that owns the governance hooks, so verdicts are asked |
| `--manifest-fault FAULT` | `ManifestFixture::with_fault` | The manifest carries that fault: `stale-version`, `future-not-before`, `wrong-user`, `changed-key` or `bad-signature`. Any other name is a usage error that lists these five |
| `--pat TEXT` | `pat(TestPat::new(..))` | The personal access token the instance accepts, and prints, in place of the fixture's. A restart with another value refuses a credential stored against the earlier one. A device accepts only a token that starts with core's API-key prefix |
| `--document PATH=FILE` (repeatable) | `document(path, bytes)` | The Drive store starts with a document at `PATH` holding the bytes of the local file `FILE`; the sync feed and the browsing listing (`GET /api/v1/documents`) both name it. The value is split at its first `=`; a file that cannot be read stops the start |
| `--keys FILE` | `keys(path)` | Sign with the keys held in `FILE` (three Ed25519 seeds), writing new ones there owner-only when it does not exist, in place of new keys on every start. A restart on the same file serves a manifest key an app that pinned the first start still trusts, so `--manifest-fault stale-version` after a healthy start serves an older version from its first manifest, a real older-version refusal. A file of any other length stops the start |
| `--extension ID` (repeatable) | `ManifestFixture::with_extension` | The manifest's `host_extensions` lists `ID` for the harness host, so the device's extension policy enables it (`drive` turns on Drive). An id that is not kebab-case is a usage error |

### 2.1 What exists at the end of S0

The builder, binding and serving on loopback, the accessors, shutdown, loading scripts and verdicts from files, `Event::matches` and `VerdictScript::verdict_for` are real. The router serves the health route only and records nothing: `requests()` is empty, `unconsumed_turns()` equals the script's length, and `wait_for` returns an already-recorded match or the typed `NotImplemented` error. No signing, no tokens, no streams. All of that is S2-A1.

### 2.2 No dependency-unification crate

Decided (D48). Core's test workspace has a generated crate that unifies dependency features across its members, and it pulls the server stack and the database driver. The scripted instance does not depend on it and is listed among its exclusions, because the golden suites link the scripted instance and run on Windows and macOS, where that stack is not built. A test crate that depends on the scripted instance stays on the native matrix; one that also needs the database fixtures does not belong on it. The scripted instance has no third-party dependency the test workspace did not already declare.

### 2.3 Constants the shared crates do not export yet

Section 1's rule is that no route or header is spelled here. Most already have a constant: the discovery document, the sign-in and session routes, `whoami`, the manifest and its public key, the profile, the heartbeat, plugin bundles and files, the Drive routes, the hooks base and `hooks/track`, receipts, the gateway base, and the headers in `systemprompt-identifiers`. Four do not, and S2-A1 cannot meet the rule until they do:

| Surface | Today | Needed |
|---|---|---|
| `hooks/govern` | A string literal in the bridge's hook writer; the shared path table has the hooks base and `track` only | A constant beside those two |
| The Messages route, `/v1/messages` | Literals in the gateway's router, in the reserved-path list and in the bridge's forwarder | One constant, relative to the gateway base, used by all three |
| The Chat Completions route, `/v1/chat/completions` | The same three places; the bridge has a private constant | The same |
| The model list, `/v1/models` | The same | The same |

These are shared-crate changes, so the integrator lands them before S2-A1 opens (contract before consumers). The device-link URL is instance configuration, not a constant: the scripted instance serves it at the path its own discovery document advertises. Whether the plan routes have constants was not established in S0; S2-A1 checks before it serves them.

## 3. Model scripts

A script is an ordered list of turns. Each turn matches the next model request and produces one streamed response.

```json
{
  "turns": [
    {
      "expect": { "tools_include": ["read", "edit"] },
      "respond": [
        { "text": "I will read the file first." },
        { "tool_call": { "id": "t1", "name": "read", "input": { "path": "src/lib.rs" } } }
      ],
      "stop": "tool_use",
      "usage": { "input": 120, "output": 18 }
    },
    {
      "expect": { "tool_results": ["t1"] },
      "respond": [ { "text": "Done." } ],
      "stop": "end"
    }
  ]
}
```

- **One script, two protocols.** The encoder renders a turn as a Messages stream or a Chat Completions stream according to the route called. A scenario therefore runs on both without two scripts.
- **Tool names are logical.** A script names tools by a neutral name; a per-product map translates to the name that product advertises (`03-golden-scenarios.md` section 3). The same script drives the harness and the reference agent.
- **`expect` is a guard, not a full match.** It checks that the request is the one the turn was written for (the right tool results are present, a required tool is advertised). A request that does not satisfy the next turn's guard fails the test with both shown.
- **Chunking is part of the script when it matters.** A tool call may carry `argument_chunks`: how its arguments are split across stream events, to exercise partial-argument assembly.
- **Stops.** `stop` is `end`, `tool_use`, `max_tokens` (Messages `max_tokens`, Chat `length`) or `content_filter` (Messages `refusal`, Chat `content_filter`). It defaults to `tool_use` when the turn has a tool call, else `end`.
- **Faults.** A turn may carry one `fault` in place of, or around, a normal answer: `{"status": 429}` (any status; a protocol-shaped error body and `retry-after-ms: 1`), `{"close_after_events": 2}`, `{"stall_ms": 5000}` (the answer, then silence before the terminal frames), `"malformed_event"`, `"context_overflow"` (400 with the body each provider sends when the prompt does not fit: Messages `prompt is too long: N tokens > W maximum`, Chat `context_length_exceeded`; `W` is the model's advertised window), or `{"headers_after_ms": 5000}` (the whole response, status line included, is held that long, then the turn answers; the turn is taken on arrival, so a client that times out and retries reaches the next turn). Every fault takes its turn.
- **No default.** A request with no remaining turn is refused and recorded as unscripted.

### 3.1 Calls beside the turns

A product makes some model calls outside its steps, whose place among the turns is not fixed. Two ways answer them without taking a turn; each records the request with its `purpose` and never counts it unscripted:

- **`title_reply(text)`** on the builder answers every session-title call: a model request that advertises no tool and carries exactly one message besides system ones, from the user. A golden scenario's `[title_call] text` is passed here. Without it a title call is an ordinary request and takes the next turn. Compaction requests also advertise no tools but carry the conversation, so they stay ordinary turns.
- **`asides`** in `turns.json` (`{"purpose", "system_contains", "respond"}`) answer a request whose system prompt contains the fragment. An aside is checked before the title reply.

## 4. What it deliberately is not

- Not a model. It never generates.
- Not a general instance. It implements only the routes a device calls.
- Not shipped. It lives in the test workspace and is never a dependency of a production crate.

## 5. Its own tests

The scripted instance is tested against the linked core wire types: a manifest it signs verifies with the production verifier; a stream it encodes decodes with the production decoder to the scripted turn. If core changes a wire type, these tests fail before any scenario does.

## 6. Doubles for the three execution modes

From S3 (local modes) and S4 (`Remote`) a scenario runs in each mode against the same expected trace.

| Mode | What runs | What is scripted |
|---|---|---|
| `Unrestricted`, `Sandboxed` | The real runtime on the test machine, publishing its session to the scripted instance | The model, the verdicts, and the instance's side of publish and control |
| `Remote` | The real runtime started by a real runner in the Linux sandbox | The model behind the real gateway pipeline; the workspace source (none, a fixture repository, a bundle) |

Additional doubles:

| Double | Role |
|---|---|
| Second client | Opens the same session mid-turn and asserts it reconstructs the same state from snapshot and deltas |
| Flaky link | Drops the local runtime's connection mid-upload and asserts the session log resumes with no gap and no duplicate |
| Sandbox probes | The conformance cases: write outside a root, read outside scope, reach a disallowed destination, escape the process limits, read a filtered environment variable |
