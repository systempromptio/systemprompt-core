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

Routes and headers are taken from the linked core crates' constants, never spelled again here.

## 2. Interface

Fixed in S0-B2 so later lanes can build against it. Shape, not final signatures:

```rust
let instance = ScriptedInstance::builder()
    .user(TestUser::default())
    .manifest(ManifestFixture::minimal())
    .models([ModelFixture::messages("m-a"), ModelFixture::chat("m-b")])
    .verdict(VerdictScript::allow_all())
    .script(ModelScript::from_file("scenarios/edit-file/turns.json"))
    .start()
    .await?;

instance.base_url();          // loopback, ephemeral port
instance.pat();               // for non-interactive sign-in
instance.requests();          // everything received, in order
instance.wait_for(Event::Heartbeat).await;
```

`requests()` returns typed records: route, headers, decoded body, arrival order. Tests assert on them: that a model call carried the session's signing headers and the user id in its metadata; that a verdict call preceded a tool's execution; that telemetry events arrived in order.

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
- **Chunking is part of the script when it matters.** A turn may specify how a tool call's arguments are split across stream events, to exercise partial-argument assembly.
- **Faults.** A turn may answer with a status code (`401`, `429`, `500`, `529`), close the stream midway, stall, or send a malformed event.
- **No default.** A request with no remaining turn is refused and recorded as unscripted.

## 4. What it deliberately is not

- Not a model. It never generates.
- Not a general instance. It implements only the routes a device calls.
- Not shipped. It lives in the test workspace and is never a dependency of a production crate.

## 5. Its own tests

The scripted instance is tested against the linked core wire types: a manifest it signs verifies with the production verifier; a stream it encodes decodes with the production decoder to the scripted turn. If core changes a wire type, these tests fail before any scenario does.

## 6. Doubles for the three execution modes

From S3 a scenario runs in each mode against the same expected trace.

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
