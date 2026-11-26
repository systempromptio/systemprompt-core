# 11. Server-owned sessions

**Decision.** A session of the native engine lives on the instance. The agent loop runs on the server; a device only executes the tools that must touch the user's machine, on request, and keeps its own permission rules as a second gate. Every client renders the session from one server-published stream.

This applies to the native engine only. Third-party coding agents run their own loops on the device and are governed through the proxy, as before.

## 1. Why

| | Loop on the device | Loop on the server, tools on the device |
|---|---|---|
| Session visible on phone, web, another desktop | No | Yes, live |
| Governance of a tool call | The device asks for a verdict and could skip it | The server decides before it issues the call |
| Approvals | Two mechanisms: local asks and a server step | One |
| Changing prompts or loop logic | A client release per fork | A server release |
| Starting work from a phone that runs on your desktop | A separate relay | The same mechanism |
| Extra data leaving the device | — | None: tool results already travel to the instance inside the next model call |
| Working without the instance | No (there are no models without it) | No |
| Server load | Stateless per model call | Holds each session's loop and history |
| New infrastructure | None | A durable request channel from server to device |

The second column also removes two weaknesses of the first design: device-side governance that a modified client could skip, and two unrelated session models.

## 2. What core has and lacks for this

Read on core `future`.

| Need | Today |
|---|---|
| An iterative loop (model, tools, results, model again) | **Absent.** Server-side agents plan once, run up to twenty tool calls in sequence, and answer; the model never sees a result and asks for more. No compaction, no subagents, no resumption after a restart. The harness engine is therefore *the* loop, not a second one |
| A durable host for a long-running run | **Present.** The agent-runs machinery: claim with a lease that is renewed, stale reclaim, release on shutdown, cancel, an ordered event log, and a stream that resumes from `Last-Event-ID` without loss. Its `RunHandler` seam is generic; a handler for an in-process engine fits beside the existing one that spawns other agents |
| A role to run them in | **Present.** The `runner` node role |
| An event vocabulary for clients | **Present, partly unused.** The AG-UI event types exist in full: run and step lifecycle, text, tool call start, arguments, end and result, state snapshot and delta, messages snapshot, custom. Snapshot and delta are never emitted |
| A durable "wait for a person" | **Present, unused in core.** A governance approval table keyed by call id, with a waiter |
| A policy engine callable in process | **Present.** The governance engine evaluated by the gateway and the external tool proxy. The in-process tool executor does not call it today |
| A per-user list of conversations | **Present.** Contexts, with tasks and messages. Message parts are text, file or data only: no first-class tool call, tool result or reasoning part |
| Addressing one device | **Absent.** The device stream fans out to every connection of a user |
| A durable downlink with replay | **Absent.** The device stream is live-only: no event ids, and frames are dropped on a full buffer |
| A route for a device to return a result | **Absent** |
| A waiter for a device's answer, with expiry and cancel | **Absent** |
| A tool that executes on a client | **Absent** |
| Per-user limits on concurrent loops | **Absent** |

## 3. The shape

```
client (desktop, phone, web)          server                              device with a workspace
        │   prompt, steer, cancel       │                                         │
        ├──────────────────────────────►│  session (a context)                    │
        │                               │  turn (a run, leased by a runner node)   │
        │                               │   ├ model call through the gateway       │
        │                               │   ├ policy verdict, in process           │
        │                               │   ├ approval, if the mode asks           │
        │                               │   ├ tool request ───────────────────────►│ own permission rules
        │                               │   │                                      │ execute in the workspace
        │                               │   ◄─────────────────────── tool result ──┤
        │   one ordered event stream    │   └ next model call                      │
        ◄───────────────────────────────┤                                         │
```

| Term | Meaning |
|---|---|
| Session | A conversation with the native engine. One per-user list, shown on every client |
| Turn | One user prompt and everything the engine does in answer. Hosted as a run: leased, cancellable, resumable |
| Workspace device | The device a session's local tools execute on. Zero or one per session. A session with none uses only tools that run on the server |
| Tool host | The part of a device build that executes tool requests |

## 4. The loop on the server

- The engine crate runs inside a `RunHandler`, on `runner` nodes.
- It already makes one model call per step from reloaded history. Each step is persisted before the next begins, so a turn survives a server restart: a reclaimed run continues from its last completed step.
- Model calls go through the gateway pipeline in process. Quotas, audit and safety apply as for any gateway traffic. No device holds a model credential for the native engine.
- Before any tool call is dispatched, the governance engine is evaluated in process. A denial is returned to the model as a refusal and never reaches a device.
- Subagents are child runs of the same kind.
- Compaction, title generation, retry with backoff, steering and queueing of prompts sent mid-run are the engine's, unchanged.
- Limits: a per-user cap on concurrent turns, and a per-instance cap per runner node.

## 5. Where a tool executes

| Tool | Executes on | Why |
|---|---|---|
| Shell, read, write, edit, patch, glob, grep; snapshots and revert; formatters; language-server diagnostics | The workspace device | They act on the user's files and processes |
| A local MCP server over stdio | The workspace device | It is a process on that machine |
| A remote MCP server; a managed MCP server | The server | Core is already an MCP client with session recovery |
| Fetch | The server, through the guarded outbound client | One egress policy |
| Todo, subagent, skill, question | The server | They are loop state |
| Documents | The server | They live there |

A request is a whole tool call (the tool's name and its input), not a sequence of low-level file and terminal operations. A search expressed as individual file reads over a network would be unusable; a search executed on the device and returned once costs one round trip.

A server-side workspace remains possible: a run whose tool host is a sandbox on a runner node, as scheduled agent runs use. The engine does not know which it has; it speaks to a tool transport.

## 6. The device request channel

New in core. It carries tool requests to one device and results back, durably.

| Element | Design |
|---|---|
| Addressing | A request names one device record. A device subscribes to its own stream only |
| Store | One row per request: run, device, tool, input, state (`pending`, `delivered`, `completed`, `failed`, `expired`, `cancelled`), and a per-device sequence number |
| Downlink | Server-sent events on a per-device route, each event carrying its sequence number. A reconnect with `Last-Event-ID` replays everything not yet answered. This is the agent-runs stream's proven pattern, applied per device |
| Uplink | `POST` of the result to the request's own route. Idempotent: a repeated post with the same body is accepted, a different body is refused |
| Waiting | The run parks in `awaiting_device`. The waiter is woken by a commit notification, with a periodic re-read as backstop; it does not poll on a timer |
| Multi-node | The request row and a notification reach the node holding the device's connection through the existing cross-node event relay |
| Progress | A device may post partial output for a long-running command; it is appended to the session stream |
| Cancel | Cancelling the turn marks open requests cancelled and tells the device, which kills the process tree |
| Expiry | A request not answered within its deadline expires. The model is told the tool failed because the device did not answer. It is never treated as success |
| Device offline | The request waits, the session shows "waiting for your desktop", and a notification is raised after a short interval. The turn can be cancelled from any client |
| Transport | Server-sent events and `POST` only; no WebSocket |

Waiting for a device and waiting for a person are the same state machine with two request kinds. Approvals use the governance approval table that already exists, keyed by call id; the turn parks in `awaiting_approval` and any of the owner's clients may answer.

## 7. The session stream

One ordered, durable event log per turn, published on a resumable stream, in the AG-UI vocabulary core already defines.

| Client need | Event |
|---|---|
| Open a session on any client | Messages snapshot, then a state snapshot |
| Streamed text and reasoning | Text and reasoning message events |
| A tool call's progress | Tool call start, arguments, end, result |
| Todos, queue, waiting state, usage | State delta (a JSON patch against the last snapshot) |
| Turn lifecycle | Run started, step started and finished, run finished or error |
| Anything specific to the engine | Custom, with a typed name |

- Every client (desktop, iOS, Android, web) is a renderer of this stream. None holds a second copy of a session.
- Resumption is by sequence number; a client that reconnects loses nothing.
- Client actions are ordinary requests: send a prompt, steer, cancel, answer an approval, answer a question.

## 8. Storage

| Data | Home |
|---|---|
| The session and the per-user list | A context, as server-side agents use |
| A turn | A run row with its lease, state and totals |
| The ordered event log | Run events, with kinds added for the engine's parts and for the two waiting states |
| The transcript the engine reloads each step (messages and parts with full fidelity, including reasoning and provider continuation data) | Tables owned by the engine member |
| Tool requests and results | The device request table |
| File snapshots for revert | On the workspace device; the server holds only snapshot identifiers |

Retention and user purge follow the instance's existing rules; the new tables join the purge inventory.

The engine's own store on the device (a local database of sessions) is removed. A device keeps only its workspace snapshots and tool-host state.

## 9. What the workspace device sends at the start of a session

A workspace descriptor: the roots the session may touch; the instruction files and skills found in the project; the local MCP servers available; the tools this device can execute. The server builds the system prompt and the tool list from it.

A phone sends no descriptor and is never a workspace device.

## 10. Security

Sending tool requests from a server to a user's machine is remote execution by design. An instance that proxies the model could already cause a tool call by forging model output; this design makes the path explicit and must make it safe.

Rules, taken from the published guidance of the Agent Client Protocol for exactly this split and applied here:

| Rule | Statement |
|---|---|
| The device's own checks stand | A request from the server never replaces the device's allow, ask and deny rules. The server cannot change or bypass them |
| Roots at time of use | The tool host checks every path against the session's roots when it executes, not when the session opened. Out-of-root requests are refused |
| Sensitive targets ask | Writes to files that control execution or approval (version-control hooks, agent and editor settings, MCP configuration) ask the user regardless of rules |
| The whole command is judged | Policy applies to the full command line, arguments, working directory and environment |
| What is shown is what runs | An approval shows the full command and paths. Automatic approval never relies on a title |
| Unknown is not approval | An unanswered, unparseable or expired decision denies |
| Output is neutralised | Control sequences in tool output are stripped before display |
| Binding | A request is bound to one device record and one session. A device executes only requests addressed to it for a session it opened a workspace for |
| The channel is authenticated per device | The device's own credential, not only the user's |
| Audit | Request, verdict, approval, device, result digest and timing are in the session log and the governance journal |

A compromised instance remains a threat to every connected desktop within the limits those rules set. That is stated to administrators, and the device-side rules are the control that bounds it.

## 11. What changes in the rest of this specification

| Area | Change |
|---|---|
| The `Governor` contract | Still the engine's only route to a model and a verdict. Its one real implementation is now in process on the server |
| Device slices (`02-target-architecture.md`) | `governor` and `engine` are no longer device slices. A device with a workspace opts into `tool-host`. Feature slices (`chat`, `runs`, `approvals`, `inbox`, `documents`) are the session and server-side features every client shares |
| Per-call model credentials on the device | Gone for the native engine. They remain for third-party agents through the proxy |
| The headless `run` command | Starts a session on the instance with this machine as its workspace device, and prints the stream |
| Scheduled server-side runs of the native engine | The same thing with a sandbox as the workspace |
| Phones | See every native session, start one, and may start one bound to the user's desktop when it is online. A phone still executes nothing |
| Golden scenarios | Unchanged in content. They prove the move: the same scripts must produce the same traces when the loop runs on the server and the tools behind the channel |

## 12. Protocol alignment and prior art

Read at the sources on 2026-11-26.

| Protocol | Relevance | Use here |
|---|---|---|
| Agent Client Protocol | An agent asks its client to read and write files, run terminals and request permission, with capability negotiation and published security guidance. Stable version 1; its only specified transport is standard input and output to a local process; a remote transport is a draft | Its split and its security rules are adopted. Its wire format is not: the remote transport is unfinished, and its low-level file and terminal methods would cost too many round trips. If the remote transport stabilises, an adapter is possible |
| AG-UI | An event vocabulary for agent-to-client streams, including state snapshot and delta as JSON patches. Its interrupt and resume fields are marked draft | The session stream uses the vocabulary core already has. Waiting states are this design's own, not AG-UI's draft interrupts |
| Model Context Protocol | Its 2026-07-28 revision removed server-initiated requests and stream resumption; long-running work is an optional tasks extension | Not a vehicle for server-to-device requests. Used only for what it is for: calling MCP servers |

No shipped product was found that documents a loop in the cloud driving tools on the user's own machine. The published products either run both on the device (with other clients as viewers) or both in a hosted sandbox. This is therefore new ground: the benefit is real and so is the lack of precedent to learn from.
