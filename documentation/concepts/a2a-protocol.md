# The A2A protocol

How systemprompt-core models agent-to-agent (A2A) communication: agents, contexts, tasks, messages, streaming, and discovery.

A2A (agent-to-agent) is the protocol the platform uses to drive and observe long-running agent work. The platform implements the A2A 1.0.1 JSON-RPC binding, with a server-sent-events (SSE) streaming channel for incremental progress. The core types live in `crates/domain/agent`; the wire models are in `crates/shared/models/src/a2a`.

## The object model

Four objects carry A2A state. They nest from the durable conversation down to the individual message part.

```
Context  (a conversation; ContextId)
   └── Task  (one unit of agent work; TaskId, belongs to a Context)
         ├── TaskStatus  (state + optional message + timestamp)
         ├── history: [Message]
         └── artifacts: [Artifact]

Message  (role + parts + ids)
   └── Part  =  exactly one of text | raw | url | data
```

### Context

A context is the conversation envelope, identified by a `ContextId`. Tasks and messages are grouped under it, and the same `context_id` threads through audit and tracing so that AI requests, tool calls, and agent work for one conversation correlate.

### Task

A task (`crates/shared/models/src/a2a/task.rs:9`) is one unit of agent work within a context. It carries an `id` (`TaskId`), the owning `context_id`, a `status`, optional `history` (the messages so far), optional `artifacts` (produced outputs), and timestamps.

A task's lifecycle is a state machine. `TaskState` (`task.rs:56`) has these states:

```
Submitted → Working ─┬→ Completed   (terminal)
                               ├→ Failed      (terminal)
                               ├→ Canceled    (terminal)
                               └→ Rejected    (terminal)

           Working ⇄ InputRequired / AuthRequired   (await caller, then resume)
```

`Completed`, `Failed`, `Canceled`, and `Rejected` are terminal — no transition out of them is permitted. `InputRequired` and `AuthRequired` are non-terminal pauses: the task is waiting on the caller (for input, or for an authentication step) and resumes when satisfied. On the wire the states are `TASK_STATE_SUBMITTED`, `TASK_STATE_WORKING`, and so on; the lowercase strings of earlier protocol versions do not parse. Transitions are validated (`TaskState::can_transition_to`), so an illegal move is rejected rather than silently applied.

### Message and parts

A message carries a role (user or agent), a set of parts, a message id, and the context it belongs to. A `Part` (`crates/shared/models/src/a2a/message.rs`) is one object holding exactly one content field, plus optional `filename`, `mediaType` and `metadata`:

| Field | Carries |
|-------|---------|
| `text` | Plain text content. |
| `raw` | Inline file bytes, base64-encoded (used for attachments; see the file-upload handling in the agent pipeline). |
| `url` | A reference to file content held elsewhere. |
| `data` | Arbitrary structured JSON data. |

```json
{"role": "ROLE_USER", "messageId": "m-1", "parts": [
  {"text": "Summarise the attached report."},
  {"raw": "JVBERi0xLjQK...", "filename": "report.pdf", "mediaType": "application/pdf"}
]}
```

Multipart messages let a single turn mix prose, attachments, and structured payloads. File parts feed the multimodal path where the target provider supports it.

### Artifact

Artifacts are the durable outputs a task produces, attached to the task and addressable through `/api/v1/core/artifacts`.

## The HTTP and streaming surface

A2A is exposed as JSON-RPC plus SSE. The relevant routes:

| Purpose | Route |
|---------|-------|
| Core context / task / artifact resources | `/api/v1/core/{contexts,tasks,artifacts}` |
| Agent registry (discovery) | `/api/v1/agents/registry` |
| Per-agent surface | `/api/v1/agents/{id}/` |
| Streaming (SSE) | `/api/v1/stream/{contexts,agui,a2a}` |
| Inbound webhook | `/api/v1/webhook/a2a` |

### Methods and versioning

Every JSON-RPC request to an agent endpoint must carry the `A2A-Version: 1.0` header, or the `?A2A-Version=1.0` query parameter. A request without it, or with an unsupported version, is refused with `-32009 VersionNotSupportedError` and HTTP 400. Agent-card GETs are unversioned.

| Method | Result |
|--------|--------|
| `SendMessage` | `{"task": …}` or `{"message": …}` |
| `SendStreamingMessage` | An SSE stream of `StreamResponse` frames |
| `GetTask`, `CancelTask` | A `Task`; params identify it by `id` |
| `SubscribeToTask` | An SSE stream of `StreamResponse` frames for an existing task |
| `ListTasks` | The caller's tasks, paginated |

`SendMessageConfiguration.returnImmediately` asks for the task to be returned before it finishes. Requests accept an optional `tenant`. Push-notification methods are not supported and return `-32601`.

`ListTasks` only returns tasks the caller owns. It filters by `contextId` and `status`, pages with `pageSize` (1 to 100, default 50) and `pageToken`, and accepts `historyLength` and `includeArtifacts`. The result carries `tasks`, `nextPageToken`, `pageSize` and `totalSize`.

Error `data` is an array of `google.rpc.ErrorInfo` objects:

```json
{"jsonrpc": "2.0", "id": 7, "error": {"code": -32001, "message": "Task not found",
  "data": [{"@type": "type.googleapis.com/google.rpc.ErrorInfo",
            "reason": "TASK_NOT_FOUND", "domain": "a2a-protocol.org", "metadata": {}}]}}
```

Route-level details are in the [HTTP API reference](../reference/http-api.md).

### Streaming

Because agent work is long-running, progress is delivered incrementally over SSE rather than as a single blocking response.

On the A2A methods `SendStreamingMessage` and `SubscribeToTask`, each frame is a `StreamResponse` that carries the request `id` and exactly one of `task`, `message`, `statusUpdate` or `artifactUpdate`. Artifact updates carry `append` and `lastChunk`. No frame is marked final: the stream closing is the terminal signal.

```json
{"jsonrpc": "2.0", "id": 7, "result": {"statusUpdate": {"taskId": "t-1", "contextId": "c-1",
  "status": {"state": "TASK_STATE_WORKING"}}}}
```

The platform channels below carry the same events to observers. A client subscribes to the relevant `/api/v1/stream/...` channel and receives status transitions, message deltas, and artifact updates as they occur.

The streaming channel is best-effort on its final hop. Cross-replica fan-out is durable — events are appended to a Postgres outbox and announced via `NOTIFY`, and peer replicas re-inject them locally — but delivery to a connected client uses a bounded per-connection channel with no per-connection replay. A slow or briefly disconnected client can miss events; the canonical state is re-fetchable through the resource endpoints. SSE alone is therefore not an at-least-once channel. Design clients to reconcile against the task/context resources rather than assuming every event arrives.

The A2A agent server and main API handle graceful shutdown. Clients must reconnect and reconcile task state when a stream is interrupted during a deployment.

## Agent cards and discovery

Agents advertise themselves through agent cards. The discovery documents are served under `.well-known/` (`agent-card.json` for the primary card and `agent-cards` for the collection), and the agent registry at `/api/v1/agents/registry` enumerates available agents. A card describes an agent's identity and capabilities so a caller can discover what an agent does before invoking it. Each entry in `supportedInterfaces` declares `protocolVersion: "1.0"`. Required security schemes are listed in `securityRequirements`, and `capabilities.extendedAgentCard` says whether an authenticated caller can fetch an extended card.

## Where A2A sits in the platform

The agent domain depends only on infra and shared crates — it has no edge to the AI or MCP domains. It reaches those capabilities through traits and the runtime/entry wiring described in [architecture.md](architecture.md). When an agent task needs a model, the call goes out through the provider-facing [gateway](gateway.md); when it needs a tool, the call goes through [MCP](mcp.md). Each AI request and tool call is logged with the task's `context_id`, `task_id`, and `trace_id`, giving an end-to-end trace from user identity through agent, model call, tool call, and result.

## See also

- [gateway.md](gateway.md) — the provider-facing model surface agents call.
- [mcp.md](mcp.md) — the tool surface agents call.
- [authentication.md](authentication.md) — how A2A requests are authenticated.
