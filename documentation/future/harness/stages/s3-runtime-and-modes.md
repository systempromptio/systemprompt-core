# S3. One runtime, three execution modes

**Goal.** The loop and its tools are one runtime in core that runs `Unrestricted`, `Sandboxed` or `Remote`; the instance is the record of every session; every client renders a session from one resumable stream.

**Entry.** S1 and S2 closed. The golden scenarios are the proof: the same scripts must produce the same normalised traces in every mode.

**Specification.** `../architecture/11-execution-modes.md`.

**Why before composition.** The execution environment is the contract every tool is written against, and the session record is what every client renders. Both must exist before the device is restructured into members.

## Wave A — contract, one lane then one review

| Lane | Repository | Owned paths | Work |
|---|---|---|---|
| **S3-A1 Contract** | core | The shared models crate (wire types, route constants); `systemprompt-engine/src/environment/**` (traits and types only); schema files (no handlers) | `ExecutionMode`, `ModeAvailability`, `ModeUnavailable`. The `ExecutionEnvironment` trait every tool effect goes through. `SandboxPolicy`. The session record: the event set in the existing AG-UI types with typed custom names; the publish and control wire types; the schema for the session log, the transcript and the control queue. The instance policy that names allowed modes |
| **S3-A2 Contract review** **(review)** | core | None | Against `11-execution-modes.md` sections 2, 6 and 10: an unavailable mode cannot start; nothing substitutes one mode for another; an instance can only narrow; mode is recorded and shown |

**Wave gate (L1).** Offline check of core.

## Wave B — four lanes, parallel

| Lane | Repository | Owned paths | Work |
|---|---|---|---|
| **S3-B1 Tools onto the environment** | core | `systemprompt-engine/src/tools/**`, `…/workspace/**`, `…/lsp/**`, `…/mcp/**` | Every effect a tool has (process, file, path, network) goes through `ExecutionEnvironment`. The host environment implements it directly. A gate fails on a tool that reaches the world another way. No behaviour change in `Unrestricted` |
| **S3-B2 Sandbox** | core | `systemprompt-engine/src/environment/sandbox/**` (platform modules) | The sandbox environment and its enforcing backends: macOS; Linux. Availability detection per platform, including the Windows cases. `Sandboxed` refuses to start without a backend. The conformance suite's fixtures |
| **S3-B3 Session record** | core | A sessions area in the agent-runs entry crate and its repository; emission in the engine's run module | Sessions as contexts with a mode. The durable per-turn log and the resumable stream with messages and state snapshots on open and deltas after. Ingest of a log published by a local runtime: ordered, resumable, idempotent. The control queue to one device: approve, deny, steer, cancel. Approvals through the existing governance approval table |
| **S3-B4 Remote runtime** | core | A run handler in the agent-runs crate; the runtime's headless role in `systemprompt-harness/src/command/**` | `Remote`: the instance starts the same binary's headless role on a runner node, in the Linux sandbox backend, over a provisioned workspace (none, a repository checkout, an uploaded bundle). Model calls and verdicts in process. Per-user and per-node limits. Per-session isolation of workspaces and credentials |

B2 and B4 both use the Linux backend: B2 owns it; B4 consumes it and waits on B2's hand-off for its last commits.

**Wave gate (L1).** Offline check of core's root workspace on Linux; the source gates.

## Wave C — three lanes, parallel

| Lane | Repository | Owned paths | Work |
|---|---|---|---|
| **S3-C1 Local runtime publishes and listens** | core | `systemprompt-engine/src/run/publish/**`; `systemprompt-device/src/control/**` | In the two local modes: append events to a local journal and upload in order, resuming after a disconnect. Receive control from the instance. Offline shown as such. The device's old session store as a source of truth is removed; the journal is a buffer |
| **S3-C2 Clients onto the stream** | template | The shell's session state and the mode selector | The desktop renders sessions from the instance's stream. Starting a session offers the modes available, each with its reason when unavailable. The mode is shown on every session |
| **S3-C3 Scenarios in every mode** | core | The golden runner and scripted doubles in the test workspace | The golden scenarios run in `Unrestricted`, `Sandboxed` and `Remote` with the same expected traces. The sandbox conformance suite on each backend. New scenarios: an unavailable mode is refused; a local session's stream opened on a second client mid-turn; upload resumes after a disconnect; control arrives while offline; a remote session with a repository checkout; a remote session with no workspace |

**Wave gate (L1).** Offline check of both repositories.

## Wave D — reviews, parallel, read-only

| Lane | Reviews |
|---|---|
| **S3-D1** **(review)** | The sandbox: each backend against the policy; no path to the host from inside; no silent start without a backend |
| **S3-D2** **(review)** | Remote: isolation between sessions; credential scope for checkouts; network egress; resource limits |
| **S3-D3** **(review)** | The session record and control: a device publishes only its own sessions; control reaches only the session's device; approvals are owner-only and a timeout denies |

## Stage gate (L2)

| Check | Must hold |
|---|---|
| Golden scenarios | The same normalised traces as at S2 in all three modes: `Unrestricted` on Windows, macOS and Linux; `Sandboxed` on macOS and Linux; `Remote` on the instance |
| Parity with the reference agent | Measured in `Unrestricted`. No ledger row moved from `same` or `intended` to `defect` |
| Sandbox conformance | Every case passes on every backend |
| Mode refusal | `Sandboxed` on a machine with no backend is refused with its reason |
| Session record | A local session is listed and readable on a second client; its stream resumes without loss |
| Reviews | No open high-rated finding |
| Evidence run | On real Windows and macOS machines: one session in each available mode, with the mode visible; the same session opened on a second client |

## Exit criteria

1. Stage gate green.
2. One loop exists, in `systemprompt-engine`. No tool reaches the world except through the execution environment.
3. Every session, in every mode, is on the instance and visible on every client.
4. `../architecture/11-execution-modes.md` matches what landed.

## Decisions reserved to people

- The default mode a desktop offers, and which modes the reference instance allows.
- The per-user and per-node limits for `Remote`.
- The default sandbox policy (writable roots, network allowlist).
- The retention period for session records.
- Merging the programme branch into `future`.
