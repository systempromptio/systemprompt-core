# The A2A protocol

How systemprompt-core models agent-to-agent (A2A) communication: agents, contexts, tasks, messages, streaming, and discovery.

A2A (agent-to-agent) is the protocol the platform uses to drive and observe long-running agent work. It is a JSON-RPC interface with a server-sent-events (SSE) streaming channel for incremental progress. The core types live in `crates/domain/agent`; the wire models are in `crates/shared/models/src/a2a`.

## The object model

Four objects carry A2A state. They nest from the durable conversation down to the individual message part.

```
Context  (a conversation; ContextId)
   └── Task  (one unit of agent work; TaskId, belongs to a Context)
         ├── TaskStatus  (state + optional message + timestamp)
         ├── history: [Message]
         └── artifacts: [Artifact]

Message  (role + parts + ids)
   └── Part  =  Text | File | Data
```

### Context

A context is the conversation envelope, identified by a `ContextId`. Tasks and messages are grouped under it, and the same `context_id` threads through audit and tracing so that AI requests, tool calls, and agent work for one conversation correlate.

### Task

A task (`crates/shared/models/src/a2a/task.rs:9`) is one unit of agent work within a context. It carries an `id` (`TaskId`), the owning `context_id`, a `status`, optional `history` (the messages so far), optional `artifacts` (produced outputs), and timestamps.

A task's lifecycle is a state machine. `TaskState` (`task.rs:56`) has these states:

```
Pending → Submitted → Working ─┬→ Completed   (terminal)
                               ├→ Failed      (terminal)
                               ├→ Canceled    (terminal)
                               └→ Rejected    (terminal)

           Working ⇄ InputRequired / AuthRequired   (await caller, then resume)
```

`Completed`, `Failed`, `Canceled`, and `Rejected` are terminal — no transition out of them is permitted. `InputRequired` and `AuthRequired` are non-terminal pauses: the task is waiting on the caller (for input, or for an authentication step) and resumes when satisfied. An `Unknown` state exists as a fallback. Transitions are validated (`TaskState::can_transition_to`), so an illegal move is rejected rather than silently applied.

### Message and parts

A message carries a role (user or agent), a set of parts, a message id, and the context it belongs to. A `Part` (`crates/shared/models/src/a2a/message.rs`) is one of three variants:

| Part | Carries |
|------|---------|
| `Text` | A `TextPart` — plain text content. |
| `File` | A `FilePart` — file content (used for attachments; see the file-upload handling in the agent pipeline). |
| `Data` | A `DataPart` — arbitrary structured JSON data. |

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

Request and response shapes for these endpoints belong in the reference material, not here.

### Streaming

Because agent work is long-running, progress is delivered incrementally over SSE rather than as a single blocking response. A client subscribes to the relevant `/api/v1/stream/...` channel and receives status transitions, message deltas, and artifact updates as they occur.

The streaming channel is best-effort on its final hop. Cross-replica fan-out is durable — events are appended to a Postgres outbox and announced via `NOTIFY`, and peer replicas re-inject them locally — but delivery to a connected client uses a bounded per-connection channel with no per-connection replay. A slow or briefly disconnected client can miss events; the canonical state is re-fetchable through the resource endpoints. SSE alone is therefore not an at-least-once channel. Design clients to reconcile against the task/context resources rather than assuming every event arrives.

The A2A agent server, unlike the main API server, wires graceful shutdown, so an agent process drains in-flight streaming work on stop.

## Agent cards and discovery

Agents advertise themselves through agent cards. The discovery documents are served under `.well-known/` (`agent-card.json` for the primary card and `agent-cards` for the collection), and the agent registry at `/api/v1/agents/registry` enumerates available agents. A card describes an agent's identity and capabilities so a caller can discover what an agent does before invoking it.

## Where A2A sits in the platform

The agent domain depends only on infra and shared crates — it has no edge to the AI or MCP domains. It reaches those capabilities through traits and the runtime/entry wiring described in [architecture.md](architecture.md). When an agent task needs a model, the call goes out through the provider-facing [gateway](gateway.md); when it needs a tool, the call goes through [MCP](mcp.md). Each AI request and tool call is logged with the task's `context_id`, `task_id`, and `trace_id`, giving an end-to-end trace from user identity through agent, model call, tool call, and result.

## See also

- [gateway.md](gateway.md) — the provider-facing model surface agents call.
- [mcp.md](mcp.md) — the tool surface agents call.
- [authentication.md](authentication.md) — how A2A requests are authenticated.
