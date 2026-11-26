# S3. Server-owned sessions

**Goal.** The native engine's loop runs on the instance. A device with a workspace executes tool requests behind its own permission rules. Every client renders a session from one resumable stream.

**Entry.** S1 and S2 closed. The golden scenarios are the proof: the same scripts must produce the same normalised traces after this stage as before it.

**Specification.** `../architecture/11-server-owned-sessions.md`.

**Why before composition.** This stage removes things the composition stage would otherwise convert and then delete: per-call model credentials on the device, the device's session store as the source of truth, and local approval handling.

## Wave A — contract, one lane then one review

| Lane | Repository | Owned paths | Work |
|---|---|---|---|
| **S3-A1 Contract** | core | Wire types and route constants in the shared models crate; the engine's tool-transport trait; the schema files (no handlers) | The device request and result types and their states. The session stream's event set in the existing AG-UI types, with typed custom names. The two waiting states. The workspace descriptor. The tool-transport trait the engine calls. The schema for the engine's transcript tables, the device request table, and the added run-event kinds |
| **S3-A2 Contract review** **(review)** | core | None | Against `11-server-owned-sessions.md` section 10: a request is bound to one device and one session; an expired or unknown outcome can only deny or fail; nothing lets the server alter a device's rules |

**Wave gate (L1).** Offline check of core.

## Wave B — four lanes, parallel

| Lane | Repository | Owned paths | Work |
|---|---|---|---|
| **S3-B1 Device request channel** | core | A new device-requests area in the gateway entry crate and its repository | The request table; the per-device resumable stream; the idempotent result route; the waiter woken by commit notification; expiry; cancel; cross-node delivery through the existing relay; per-device authentication |
| **S3-B2 Engine on the server** | core | `systemprompt-engine/src/{run,store}/**`; a new run handler beside the existing one in the agent-runs crate | The engine under a `RunHandler`: one persisted step at a time, resumable after reclaim. The transcript in engine-owned tables. Model calls through the gateway pipeline in process. Policy evaluated in process before dispatch. Approvals through the existing governance approval table. Per-user and per-node limits. The device's local session store removed |
| **S3-B3 Session stream and routes** | core | The agent-runs entry crate's session routes; emission in the run handler | Sessions as contexts: list, open, prompt, steer, cancel, answer. The durable per-turn event log published in the AG-UI vocabulary, with messages and state snapshots on open and state deltas after |
| **S3-B4 Tool host** | core | `systemprompt-engine/src/tools/host/**`; `systemprompt-device/src/requests/**` | The device side: subscribe to its request stream with resume; check each request against the session's roots and the device's own rules at time of use; execute with the existing tools; post results and progress; kill the process tree on cancel. Sensitive-target prompts. Output neutralised |

**Wave gate (L1).** Offline check of core's root workspace; the source gates.

## Wave C — three lanes, parallel

| Lane | Repository | Owned paths | Work |
|---|---|---|---|
| **S3-C1 Clients onto the stream** | core, template | `systemprompt-harness/src/command/**`; the template shell's session state | `run` starts a session on the instance with this machine as workspace device and prints the stream. The desktop shell renders sessions from the server stream and no longer from a local store |
| **S3-C2 Test doubles and scenarios** | core | The scripted instance and golden runner in the test workspace | A scripted model behind the real gateway pipeline; a scripted device that answers tool requests, drops its connection, answers late, and answers twice. The golden scenarios run through the server-hosted loop. New scenarios: device offline then returns; request expires; duplicate result; cancel during a device wait; restart of the runner mid-turn |
| **S3-C3 Removal** | core, template | The device-side governor signing, per-call model credentials, and local session store | Deleted, with their tests. No dual path remains |

**Wave gate (L1).** Offline check of both repositories.

## Wave D — reviews, parallel, read-only

| Lane | Reviews |
|---|---|
| **S3-D1** **(review)** | The device request channel: addressing, authentication, idempotency, expiry, cross-node delivery |
| **S3-D2** **(review)** | The tool host: roots at time of use, the device's rules as a gate the server cannot move, sensitive targets, command judgement, output handling |
| **S3-D3** **(review)** | The loop: policy before dispatch on every path including subagents; approvals; limits; that a reclaimed turn cannot repeat a side effect it already completed |

## Stage gate (L2)

| Check | Must hold |
|---|---|
| Golden scenarios | The same normalised traces as at S2, with the loop on the server and tools behind the channel, on Windows, macOS and Linux |
| New scenarios | Device offline and return; expiry; duplicate result; cancel while waiting; runner restart mid-turn |
| Parity ledger | No row moved from `same` or `intended` to `defect` |
| Reviews | No open high-rated finding |
| Latency | The added time per tool call, measured against a local loop on the same scenarios, is recorded |
| Evidence run | On real Windows and macOS machines: a session whose tools execute on that machine; the same session opened on a second client mid-turn shows the same state |

## Exit criteria

1. Stage gate green.
2. No model credential for the native engine exists on a device; no session is stored only on a device.
3. A session started on one client is visible and steerable from another.
4. `../architecture/11-server-owned-sessions.md` matches what landed.

## Decisions reserved to people

- The per-user and per-node limits.
- The request expiry and the offline-notification interval.
- The retention period for server-held transcripts.
- Merging the programme branch into `future`.
