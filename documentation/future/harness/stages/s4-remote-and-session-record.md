# S4. Remote runtime and the session record

**Goal.** The same runtime runs `Remote` in the instance's sandbox. The instance is the record of every session in every mode, and every client renders a session from one resumable stream.

**Entry.** S3 closed (the execution environment and the Linux sandbox backend exist).

**Specification.** `../architecture/11-execution-modes.md` sections 5 to 8 and 10.

## Wave P — prototypes, two lanes, parallel

| Lane | Repository | Owned paths | Work |
|---|---|---|---|
| **S4-P1 Remote isolation prototype** | a throwaway branch | None that is kept | Start the headless runtime on a runner node at each isolation level (`../architecture/11-execution-modes.md` section 7): the Linux sandbox alone; a container per session; where the hosting offers it, a small virtual machine per session. In the container hosting the instance actually uses. Report: which levels start; cold-start time and memory per session; whether one session can observe another; egress control |
| **S4-P2 Record and checkout prototype** | a throwaway branch | None that is kept | Publish a local session's events to a stub ingest with a forced disconnect and resume. Clone a repository into a session workspace with a credential scoped to that session, using what the instance's existing connectors provide. Report: what the connectors offer and lack; upload volume for a typical session |

Both are deleted. Their reports fix the default isolation level and the checkout design before wave A.

## Wave A — contract, one lane then one review

| Lane | Repository | Owned paths | Work |
|---|---|---|---|
| **S4-A1 Contract** | core | The shared models crate (wire types, route constants); schema files (no handlers) | The session record: the event set in the existing AG-UI types with typed custom names; the provenance of a record (written by the instance, or reported by a device); the publish and control wire types; the schema for the session log, the transcript and the control queue. `RemoteIsolation`. The workspace source (none, a repository checkout, an uploaded bundle) |
| **S4-A2 Contract review** **(review)** | core | None | A device publishes only its own sessions; control reaches only the session's device; approvals are owner-only and a timeout denies; a device-reported record is marked as such everywhere it is shown |

**Wave gate (L1).** Offline check of core.

## Wave B — three lanes, parallel

| Lane | Repository | Owned paths | Work |
|---|---|---|---|
| **S4-B1 Session record** | core | A sessions area in the agent-runs entry crate and its repository; emission in the engine's run module | Sessions as contexts with a mode. The durable per-turn log and the resumable stream, with messages and state snapshots on open and deltas after. Approvals through the existing governance approval table |
| **S4-B2 Remote runtime** | core | A run handler in the agent-runs crate; the headless role in `systemprompt-harness/src/command/**` | The instance starts the same binary's headless role on a runner node at the configured isolation level, over a provisioned workspace. Model calls and verdicts in process. Per-user and per-node limits. Per-session isolation of workspaces and credentials. Changes leave a checkout as a branch or a patch |
| **S4-B3 Local publish and control** | core | `systemprompt-engine/src/run/publish/**`; `systemprompt-device/src/control/**`; ingest and control routes in the gateway entry crate | In the two local modes: append events to a local journal and upload in order, resuming after a disconnect. A control queue to one device: approve, deny, steer, cancel, with replay after a reconnect. Offline shown as such. The device's old session store as a source of truth is removed; the journal is a buffer |

**Wave gate (L1).** Offline check of core's root workspace; the source gates.

## Wave C — four lanes, parallel

| Lane | Repository | Owned paths | Work |
|---|---|---|---|
| **S4-C1 Clients onto the stream** | template | The shell's session state | The desktop renders sessions in every mode from the instance's stream. A record reported by a device is labelled. Starting a session offers `Remote` with its workspace choices |
| **S4-C2 Scenarios** | core | The golden runner and doubles | Every golden scenario in `Remote` against the same expected traces. New scenarios: a local session's stream opened on a second client mid-turn; upload resumes after a disconnect with no gap and no duplicate; control sent while the device is offline; a remote session with no workspace; one with a repository checkout; two remote sessions that cannot see each other; a remote session at its resource limit |
| **S4-C3 Scheduled sessions** | core | A schedules area in the agent-runs entry crate; a job in core's scheduler | A stored prompt, workspace source, agent and schedule that starts a `Remote` session when it fires (`../architecture/12-workbench-and-automation.md` section 4). Create, list, pause, run on demand. Per-user caps; a schedule that fails repeatedly pauses itself. An unanswered approval denies |
| **S4-C4 Memory** | core | `systemprompt-engine/src/memory/**` | Recall before a turn through core's memory capability, contributed as a prompt section and recorded in the turn. The closed turn emitted for learning. Routes for a user to see what was recalled, see what is held, and remove an item. An opt-in slice an instance can disable (section 5) |

## Wave D — reviews, parallel, read-only

| Lane | Reviews |
|---|---|
| **S4-D1** **(review)** | Remote: isolation between sessions; credential scope for checkouts; network egress; resource limits |
| **S4-D2** **(review)** | The record and control: ownership, addressing, replay, provenance |
| **S4-D3** **(review)** | Scheduled sessions (unattended execution: limits, approvals, who may create) and memory (what is retained, per-user scope, removal) |

## Stage gate (L2)

| Check | Must hold |
|---|---|
| Golden scenarios | The same normalised traces in all three modes |
| Session record | A session in any mode is listed and readable on a second client; its stream resumes without loss |
| Isolation | The two-session and resource-limit scenarios pass at the configured isolation level |
| Scheduled sessions | A schedule fires and its session appears in the owner's list; a failing schedule pauses itself |
| Memory | A fact learned in one session is recalled in a later one, shown as recalled, and gone after the user removes it |
| Reviews | No open high-rated finding |
| Evidence run | A `Remote` session started from one client and followed on another; a local session followed on a second client |

## Exit criteria

1. Stage gate green.
2. Every session, in every mode, is on the instance and visible on every client, with its provenance.
3. The remote runtime is the same binary as the local one.
4. `../architecture/11-execution-modes.md` matches what landed.

## Decisions reserved to people

- The default `RemoteIsolation` for the reference instance, informed by the prototype.
- The per-user and per-node limits.
- The retention period for session records.
- Merging the programme branch into `future`.
