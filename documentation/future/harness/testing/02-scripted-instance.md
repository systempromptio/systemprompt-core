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
| Model list | `/v1/models` and the profile | From the builder; per-host protocol filter honoured |
| Model calls | `/v1/messages`, `/v1/chat/completions` | Scripted streams (section 3) |
| Hook token | The OAuth client and `client_credentials` exchange | Test tokens |
| Telemetry | `hooks/track` | Recorded |
| Verdict | `hooks/govern` | Scripted per tool: allow, deny with a reason, delay beyond the budget, or error |
| Heartbeat | The heartbeat route | Recorded |
| Bundles and runtime files | Plugin bundle fetch; plans; receipts | Served from the builder; receipts recorded |
| Documents | The Drive routes the harness uses | An in-memory store |

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
| `ScriptedInstanceBuilder` | `user`, `pat`, `manifest`, `models`, `verdict`, `script`, `port` (default 0, an ephemeral port; always loopback), `start` |
| `ScriptedInstance` | The calls above, plus `addr` and read access to each fixture it was built with |
| `TestUser` | Typed user id, optional tenant id, email, display name, roles |
| `ManifestFixture`, `ManifestFault` | `minimal()`, `with_fault`. Faults: `StaleVersion`, `FutureNotBefore`, `WrongUser`, `ChangedKey`, `BadSignature` |
| `ModelFixture` | A typed model id and a wire protocol (core's `WireProtocol`; there is no local protocol enum); `messages(id)`, `chat(id)`, `on(id, protocol)`, `named(display)` |
| `VerdictScript`, `Verdict` | A default verdict and a map from logical tool name to verdict. `Allow`, `Deny { reason }`, `Delay { millis }`, `Error { status }`. File shape: `{"default": <verdict>, "tools": {"<name>": <verdict>}}` |
| `ModelScript`, `Turn`, `Expect`, `Respond`, `ToolCall`, `StopReason`, `Usage`, `TurnFault` | Section 3 |
| `RequestRecord`, `RecordedBody`, `Route` | Sequence number, route, method, path, query, headers, body (`Empty`, `Json`, `Bytes`), and whether it was unscripted. `Route` is a closed enum of the surfaces in section 1, with `Unrouted` for anything else |
| `Event` | `Heartbeat`, `Telemetry`, `Verdict`, `ModelCall`, `ManifestFetch`, `Receipt`, `Route(Route)` |
| `ScriptedInstanceError` | Typed: bind, serve, join, read, parse, `NotImplemented`, `WaitTimedOut` |

Every scripted structure refuses unknown keys, so a misspelt key in a scenario file fails the test that loads it.

`requests()` returns typed records in arrival order. Tests assert on them: that a model call carried the session's signing headers and the user id in its metadata; that a verdict call preceded a tool's execution; that telemetry events arrived in order.

A verdict is scripted **per tool, not per call**. A scenario that needs a sequence of verdicts for one tool needs a wider type; the lane that writes the first such scenario requests it.

The binary: `systemprompt-scripted-instance [--port N] [--script turns.json] [--verdicts verdicts.json] [--messages-model ID]... [--chat-model ID]...`. It prints one JSON line with `base_url` and `pat` on standard output, logs to standard error, and serves until interrupted.

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
- **Faults.** A turn may carry one `fault` in place of a normal answer: `{"status": 429}` (any of `401`, `429`, `500`, `529`), `{"close_after_events": 2}`, `{"stall_ms": 5000}`, or `"malformed_event"`.
- **No default.** A request with no remaining turn is refused and recorded as unscripted.

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
