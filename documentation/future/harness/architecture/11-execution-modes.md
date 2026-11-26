# 11. One runtime, three execution modes

**Decision.** The agent loop and its tools are one runtime, implemented and tested in a single place in core. It runs in one of three typed modes. The loop is identical in all three; only where the runtime is placed and how its tools are confined differ. The instance is the record of every session, whichever mode ran it.

This applies to the native engine. Third-party coding agents run their own loops and are governed through the proxy, as before.

An earlier draft of this file split the loop (on the server) from the tools (on the device), with each tool call crossing the network. That design is withdrawn: no shipped product documents it, every tool call paid a round trip, and it needed a new durable execution channel. What replaces it is the shape every shipped product uses: loop and tools together, placed locally or remotely.

## 1. The modes

```rust
pub enum ExecutionMode {
    Unrestricted,
    Sandboxed,
    Remote,
}
```

| Mode | Where the runtime runs | What its tools can reach | For |
|---|---|---|---|
| `Unrestricted` | On the user's machine | Everything the user can: all local files, any process, the network. The user's permission rules and asks still apply | Desktop work at parity with other local coding agents |
| `Sandboxed` | On the user's machine | Only what the session's sandbox policy grants: named writable roots, an allowlist of network destinations, bounded processes. Enforced by the operating system, not by the runtime's good behaviour | Desktop work where the machine must be protected from the agent |
| `Remote` | On the instance, in its sandbox | A workspace the instance provisions for the session. Nothing on any user's machine | Phones; the web; chat and planning; scheduled and delegated work; organisations that require server-side execution |

The mode is a property of a session, chosen when it starts and recorded with it. It does not change mid-session.

## 2. Where each mode is available

| Surface | `Unrestricted` | `Sandboxed` | `Remote` |
|---|---|---|---|
| Desktop, macOS | Yes | Yes | Yes |
| Desktop, Linux | Yes | Yes | Yes |
| Desktop, Windows | Yes | Only where an enforcing backend is present (section 4); otherwise the mode is reported unavailable | Yes |
| Phone (iOS, Android) | No | No | Yes |
| Web console | No | No | Yes |
| Scheduled and server-originated sessions | No | No | Yes |

Availability is typed and computed, never assumed:

```rust
pub enum ModeAvailability {
    Available,
    Unavailable { reason: ModeUnavailable },
}

pub enum ModeUnavailable {
    NotOnThisSurface,
    NoSandboxBackend,
    DisabledByInstance,
    DisabledByAdministrator,
}
```

Two rules follow from the type:

- A mode that is unavailable cannot be started. In particular `Sandboxed` never degrades silently to `Unrestricted` when its backend is missing; the session is refused with the reason.
- An instance's policy names the modes it allows. An organisation that needs hard enforcement allows `Remote` only; a managed machine may forbid `Unrestricted`.

## 3. One loop

The single place: `systemprompt-engine`. It holds the loop (prompt admission, one model call per step from reloaded history, tool dispatch, steering and queueing, abort, retry, compaction, subagents), the tools, and the contract tools execute against.

```rust
pub trait ExecutionEnvironment: Send + Sync {
    fn mode(&self) -> ExecutionMode;
    // spawn a process, open a file, resolve a path, reach the network:
    // every effect a tool has on the world goes through this trait
}
```

| Mode | Environment the loop is given |
|---|---|
| `Unrestricted` | The host environment: effects performed directly |
| `Sandboxed` | The sandbox environment: effects performed inside the platform's enforcing backend under the session's policy |
| `Remote` | The same runtime started on a runner node of the instance, with the sandbox environment over a provisioned workspace |

No tool knows which mode it is in. A tool that reaches the world other than through the environment is a gate failure. This is what makes the loop one implementation: the mode is an argument, not a fork.

`Remote` does not have its own loop or its own tools. It is the same binary's headless role, started by the instance.

## 4. One sandbox policy, several enforcers

```rust
pub struct SandboxPolicy {
    pub writable_roots: Vec<AbsolutePath>,
    pub readable_roots: ReadScope,          // everything, or named roots
    pub network: NetworkPolicy,             // none, or an allowlist of destinations
    pub process: ProcessLimits,
    pub environment: EnvironmentPolicy,     // which variables pass through
}
```

One policy type. Each platform enforces it with what that platform provides.

| Platform | Enforcing backend | Notes |
|---|---|---|
| macOS | The system's built-in sandbox profile mechanism | Nothing to install |
| Linux (desktop and the instance's runner nodes) | Kernel namespaces with filesystem and system-call restriction | The same backend serves `Sandboxed` on a Linux desktop and `Remote` on the instance, so those two are literally the same sandbox |
| Windows | None in the platform that matches the policy. `Sandboxed` is available when the session runs through the Linux subsystem or a local container, which use the Linux backend | Stated plainly to users; never presented as sandboxed when it is not |
| Any desktop, opt-in | The instance's own container image run locally | An identical environment to `Remote`, for teams that want it. It cannot build software that needs the host's own toolchain |

What the other shipped agents do, read at their documentation on 2026-11-26, is the same pattern: the built-in system sandbox on macOS; namespace or kernel-level restriction on Linux; and on native Windows either no sandbox or one the vendor had to build because the platform lacks it.

Strength differs by backend. What does not differ is the policy, and a conformance suite proves each backend honours it (section 9).

## 5. The instance is the record

Whatever the mode, a session exists on the instance: listed for its owner, readable on every client, auditable.

| Mode | How the record is kept |
|---|---|
| `Remote` | The runtime is on the instance and writes the session log directly |
| `Unrestricted`, `Sandboxed` | The local runtime publishes its events to the instance as they happen, and receives control from it |

**Publishing.** The runtime appends each event to a local journal and uploads in order with sequence numbers. The upload is resumable and idempotent: after a disconnect it continues from the last acknowledged number. The local journal is a buffer, not a second source of truth.

**Control.** A small downlink carries what other clients may send to a running local session: an approval or denial, a steering message, a cancel. It is low-volume control, not tool execution. If the machine is offline, the session shows as such on other clients and the control waits or expires.

**The stream.** One ordered event log per turn, published on a resumable stream in the AG-UI vocabulary core already defines (run and step lifecycle, text, reasoning, tool call start, arguments, end and result, state snapshot and delta, custom). Every client renders it. No client holds a second copy of a session.

**Approvals.** One mechanism in every mode: the governance approval table core already has, keyed by call id. In local modes the user at the machine usually answers; any of the owner's clients may.

## 6. Governance in each mode

| | `Unrestricted` | `Sandboxed` | `Remote` |
|---|---|---|---|
| Model calls | Through the instance's gateway, signed per call | Same | Through the gateway, in process |
| Verdict before a tool | Asked of the instance by the runtime | Same | Evaluated in process |
| What bounds the agent if the runtime misbehaves or is modified | Nothing beyond the user's own privileges | The operating system's sandbox | The instance's sandbox, on infrastructure the user does not control |
| Suitable when | The user is trusted with their own machine | The machine must be protected from the agent | The organisation must be certain of enforcement |

This is the honest trade-off. In the two local modes the runtime runs on a machine its user controls; governance of tool calls there is as strong as the client is unmodified. The server always controls model calls. An organisation that cannot accept that allows `Remote` only.

## 7. Remote workspaces

A `Remote` session needs something to work on.

| Workspace | Use |
|---|---|
| None | Chat, planning, questions, work on documents. Nothing is provisioned beyond a scratch directory |
| A repository checkout | The instance clones a repository the user has connected, at a named ref, into the session's sandbox. Changes leave as a branch or a patch |
| An uploaded bundle | Files the user attaches |

The sandbox policy for a remote workspace is the instance's, not the user's. Network egress is the instance's allowlist.

A session can be continued in another mode by starting a new session from its history and, for code, from the branch it produced. Mode itself never changes inside a session.

## 8. What core has and lacks

Read on core `future`.

| Need | Today |
|---|---|
| An iterative loop | **Absent** on the server: agents plan once, run up to twenty tool calls in sequence, and answer. The engine is therefore the one loop, not a second |
| A durable host for a remote runtime | **Present**: leased, cancellable, resumable runs with an ordered event log and a stream that resumes from `Last-Event-ID`. A seam for starting another program per run already exists and is how other agents are run headless |
| A node role for it | **Present**: `runner` |
| The event vocabulary | **Present**; snapshot and delta are defined and never emitted |
| A durable wait for a person | **Present**: the governance approval table |
| A per-user list of conversations | **Present**: contexts. Message parts lack first-class tool call, tool result and reasoning parts |
| Ingest of a session log published by a device | **Absent** |
| A control downlink to one device | **Absent**: the device stream fans out to all of a user's connections and has no replay |
| A sandbox | **Absent** in core. The bridge has none |
| Per-user limits on concurrent remote sessions | **Absent** |

## 9. How one implementation is kept one

| Guard | What it prevents |
|---|---|
| The loop and tools exist only in `systemprompt-engine` | A second loop anywhere: in the shell, the server, a fork |
| Tools reach the world only through `ExecutionEnvironment` | A tool that behaves differently by mode |
| The golden scenarios run in all three modes with the same expected traces | Behaviour drifting between modes |
| A sandbox conformance suite runs against every backend: writes outside the roots, reads outside the scope, disallowed network destinations, process escape, environment leakage | A backend that enforces less than the policy says |
| A fork's tools and members compile into the same runtime | A fork reimplementing execution |
| The remote runtime is the same binary's headless role | The instance and the device running different code |
| `Sandboxed` refuses to start without a backend | A sandbox that silently is not one |

## 10. Security

| Rule | Statement |
|---|---|
| Mode is explicit and visible | Every session shows its mode on every client. `Unrestricted` is labelled as such |
| No silent downgrade | An unavailable mode is refused, never substituted |
| The policy is the instance's to narrow | An instance may forbid modes and tighten a sandbox policy; it cannot widen what an administrator's machine policy forbids |
| Permission rules apply in every mode | Allow, ask and deny rules and the instance verdict are evaluated before a tool runs, sandbox or not |
| Sensitive targets ask | Writes to files that control execution or approval (version-control hooks, agent and editor settings, MCP configuration) ask in every local mode |
| Output is neutralised | Control sequences in tool output are stripped before display |
| Remote workspaces are isolated per session | One session cannot read another's workspace; credentials for a checkout are scoped to that session |
| Audit | Mode, policy, verdicts, approvals and results are in the session log and the governance journal |

## 11. Prior art

Read at the vendors' own documentation on 2026-11-26.

| Product | Local | Remote |
|---|---|---|
| Claude Code | Loop and tools on the user's machine, with an optional operating-system sandbox; other devices attach as viewers and controllers | Loop and tools together in a hosted machine |
| Codex | Loop and tools on the user's machine, with sandbox modes and an approval policy | Loop and tools together in a cloud workspace |
| OpenCode | A local server process runs both; clients attach over HTTP | Not documented |

None sends individual tool calls across the network between a remote loop and a local machine. This design follows them, and adds two things they do not document together: one typed mode enum over all three placements, and the instance as the record of every session regardless of mode.
