# 12. Workbench and automation

Five capabilities users expect from a current coding agent, placed in the stages where their foundations already exist instead of being left to the end.

| Capability | Slice | Built in |
|---|---|---|
| Parallel sessions with isolated working copies | part of `engine` | S3 |
| Integrated terminal | `terminal` | S5 |
| Preview pane | `preview` | S5 |
| Scheduled sessions | `schedules` | S4 (server), S5 (desktop view) |
| Memory | `memory` | S4 (runtime), S5 (desktop view) |

Each is an opt-in slice, written once, and works in whichever execution modes it can (`11-execution-modes.md`).

## 1. Parallel sessions with isolated working copies

Several sessions may work on one repository at the same time without colliding.

```rust
pub enum WorkspaceIsolation {
    Shared,      // the session works in the directory it was opened on
    Worktree,    // the session gets its own working copy and branch
}
```

| Aspect | Design |
|---|---|
| Mechanism | For `Worktree`, the runtime creates a linked working copy of the repository on a new branch from the chosen base, in a directory the harness owns, and runs the session there |
| Default | `Shared` for the first session on a directory; `Worktree` offered, and required, when a second session starts on a repository that already has an active one |
| Finishing | The session's work is a branch. The user merges, opens a pull request, or discards. The working copy is removed only when its branch has no unmerged, unpushed work, or on explicit confirmation |
| Not a repository | `Worktree` is unavailable, with its reason; `Shared` is refused for a second concurrent session that writes |
| Modes | `Unrestricted` and `Sandboxed`: as above; in `Sandboxed` the working copy is the writable root. `Remote`: every session already has its own checkout, so isolation is inherent |
| Snapshots and revert | Per working copy, unchanged |
| Record | The isolation, base and branch are part of the session record |

The cleanup rule is deliberate: a working copy holding work that exists nowhere else is never deleted silently.

## 2. Integrated terminal

A terminal the user types into, inside the session's workspace.

| Aspect | Design |
|---|---|
| What it is | A pseudo-terminal started through the session's `ExecutionEnvironment`. It is the user's shell, not a tool the model calls |
| Mode | It inherits the session's mode. In `Sandboxed` it runs inside the same sandbox as the agent, so a command the user types has the same limits. In `Remote` it is a terminal in the session's remote workspace, carried over the session's connection |
| The agent and the terminal | The agent does not read the terminal by default. The user can attach selected output to a prompt. Policy may forbid the terminal entirely |
| Rendering | A native terminal view in the shell: a grid, scrollback, selection, the usual control sequences. Output is treated as untrusted: sequences that could change window state or exfiltrate are filtered |
| Record | Terminal input and output are not part of the session record unless the user attaches them |

The terminal view is a shell member at a dock mount point. The pseudo-terminal support is an engine capability behind the `terminal` slice.

## 3. Preview pane

See what the work produces without leaving the app.

| Aspect | Design |
|---|---|
| Detection | The runtime notices a process it started listening on a local port and offers a preview |
| Rendering | The platform's own web view, embedded in a pane. This is the **single** place a web view is used in the shell, and it is confined: it loads only the session's own preview origin, has no access to the app's state or credentials, and exposes no bridge into the app |
| Fallback | Where no platform web view is available, or policy forbids it, the preview opens in the system browser. The slice is opt-in; a build without it has no web view at all |
| `Remote` | The instance proxies the session's port at an authenticated, per-session address that only the session's owner can open. Nothing is exposed publicly |
| `Sandboxed` | The preview origin is loopback inside the sandbox policy; the pane reaches it without widening the policy |
| Beyond web | Images and documents the session produces open in the pane with native rendering |

The original harness had a rule of no web view anywhere. The preview pane is a reasoned exception, limited as above and removable by leaving the slice out.

## 4. Scheduled sessions

Work that starts without a person.

| Aspect | Design |
|---|---|
| What it is | A stored prompt, a workspace source, an agent and a schedule. When it fires, the instance starts a `Remote` session |
| Why `Remote` only | A schedule must not depend on a user's machine being on, and must run under the instance's enforcement |
| Where it lives | Core's existing scheduler starts it; the schedule belongs to its owner and appears in their session list like any other session |
| Approvals | A scheduled session that needs approval waits and notifies its owner; an unanswered request denies. A schedule may be created with a permission mode that asks for nothing outside its declared tools |
| Limits | Per-user caps on schedules and on concurrent scheduled sessions; a schedule that fails repeatedly pauses itself and says so |
| Clients | Created, listed, paused and run on demand from the desktop, the phone and the web |

## 5. Memory

What the agent has learned carries into later sessions.

| Aspect | Design |
|---|---|
| Source | Core's organisational memory capability. The runtime does not keep its own memory store |
| Recall | Before a turn, the runtime asks for facts relevant to the prompt and the workspace, and contributes them as a prompt section through the same contract every other prompt section uses. What was recalled is recorded in the turn |
| Learning | When a turn closes, the runtime emits the turn to the memory capability, which decides what, if anything, to retain under its own validation and policy |
| Scope | Per user by default, with whatever wider scopes the instance's memory policy allows |
| Control | A user can see what was recalled for a turn, see what is held about them, and remove an item. An instance can disable the slice |
| Modes | All three. In the local modes recall and learning are requests to the instance; nothing is stored on the device |

## 6. Standards these share

- Each is a slice a build may leave out and an instance may disable.
- Each is written against the execution environment and the session record, so it has one implementation across modes and surfaces.
- Each has golden scenarios and tour states added in the stage that builds it.
- The terminal, the preview proxy and scheduled sessions are reviewed independently: they are, respectively, a user-driven execution path, a network path into a remote workspace, and unattended execution.
