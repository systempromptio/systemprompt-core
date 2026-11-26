# S3. Execution environment and sandbox

**Goal.** Every effect a tool has goes through one execution environment. The runtime runs `Unrestricted` or `Sandboxed` on a desktop, from one loop, with typed mode availability.

**Entry.** S1 and S2 closed. The golden scenarios are the proof: the same scripts must produce the same normalised traces in both local modes.

**Specification.** `../architecture/11-execution-modes.md` sections 1 to 4, 9 and 10.

This stage is local only. `Remote` and the session record are S4.

## Wave P — prototype, one lane

| Lane | Repository | Owned paths | Work |
|---|---|---|---|
| **S3-P1 Sandbox prototype** | a throwaway branch | None that is kept | Run the lifted runtime's headless role, whole, inside each candidate backend on macOS and Linux with a policy of one writable root and a network allowlist of the instance only. Run three golden scenarios. Report: what broke (language servers, MCP child processes, git, package managers, file watching); the start-up cost; which Linux mechanism to adopt; whether the same Linux backend starts inside the container hosting the instance uses |

The prototype is deleted. Its report fixes the backend choice and the default policy before wave A.

## Wave A — contract, one lane then one review

| Lane | Repository | Owned paths | Work |
|---|---|---|---|
| **S3-A1 Contract** | core | The shared models crate (mode types); `systemprompt-engine/src/environment/**` (traits and types only) | `ExecutionMode`, `ModeAvailability`, `ModeUnavailable`. The `ExecutionEnvironment` trait. `SandboxPolicy` and its presets. The instance policy that names allowed modes. The request to widen a policy for one session |
| **S3-A2 Contract review** **(review)** | core | None | An unavailable mode cannot start; nothing substitutes one mode for another; an instance can only narrow; mode is recorded and shown |

**Wave gate (L1).** Offline check of core.

## Wave B — four lanes, parallel

| Lane | Repository | Owned paths | Work |
|---|---|---|---|
| **S3-B1 Tools onto the environment** | core | `systemprompt-engine/src/tools/**`, `…/workspace/**`, `…/lsp/**`, `…/mcp/**` | Every effect a tool has (process, file, path, network) goes through `ExecutionEnvironment`. The host environment implements it directly. A gate fails on a tool that reaches the world another way. No behaviour change in `Unrestricted` |
| **S3-B2 Sandbox backends** | core | `systemprompt-engine/src/environment/sandbox/**` (platform modules) | The macOS and Linux backends, enforcing one `SandboxPolicy`. In `Sandboxed` the **whole runtime** starts inside the sandbox, so file tools, language servers, MCP child processes and anything a dependency spawns are confined by the operating system, not only by the environment trait. The instance is on the network allowlist so model calls and the session record still work |
| **S3-B3 Availability and policy** | core, template | `systemprompt-engine/src/environment/availability/**`; the shell's mode selector | Availability computed per surface and platform. `Sandboxed` is refused where no backend exists, including native Windows. A sandbox denial raises a request to widen the policy for this session (a path or a destination), answered like any approval. Policy presets, including one that admits the common package registries. The mode shown on every session |
| **S3-B4 Isolated working copies** | core | `systemprompt-engine/src/workspace/isolation/**` | `WorkspaceIsolation` (`../architecture/12-workbench-and-automation.md` section 1): a session may run in its own linked working copy on its own branch. Required when a second writing session starts on a repository that already has one. Cleanup only when the branch holds nothing unmerged and unpushed, or on confirmation. Recorded with the session |

**Wave gate (L1).** Offline check of core on Linux; the source gates.

## Wave C — two lanes, parallel

| Lane | Repository | Owned paths | Work |
|---|---|---|---|
| **S3-C1 Conformance suite** | core | The sandbox conformance area in the test workspace | For each backend: write outside a root; read outside scope; reach a disallowed destination; escape the process limits; read a filtered environment variable; a child of a child; a symlink out of a root |
| **S3-C2 Scenarios in both local modes** | core | The golden runner | Every golden scenario in `Unrestricted` and in `Sandboxed`, against the same expected traces. New scenarios: two sessions on one repository, each in its own working copy, neither seeing the other's changes; cleanup refused while a branch holds unpushed work; an unavailable mode is refused with its reason; a denial followed by an approved widening; a denial followed by a refused one |

## Wave D — review, one lane

| Lane | Reviews |
|---|---|
| **S3-D1** **(review)** | Each backend against the policy; no path to the host from inside; no silent start without a backend; the widening request cannot exceed what the instance allows |

## Stage gate (L2)

| Check | Must hold |
|---|---|
| Golden scenarios | The same normalised traces as at S2: `Unrestricted` on Windows, macOS and Linux; `Sandboxed` on macOS and Linux |
| Parity with the reference agent | Measured in `Unrestricted`. No ledger row moved to `defect` |
| Conformance | Every case passes on both backends |
| Refusal | `Sandboxed` on native Windows, and on a machine with no backend, is refused with its reason |
| Evidence run | On a real macOS machine and a real Windows machine: a session in each available mode, with the mode visible |

## Exit criteria

1. Stage gate green.
2. One loop exists, in `systemprompt-engine`. No tool reaches the world except through the execution environment.
3. `Sandboxed` confines the whole runtime, and a sandbox that cannot start refuses the session.
4. Two sessions can work on one repository at once without colliding.

## Decisions reserved to people

- The default mode a desktop offers.
- The default sandbox policy and presets, informed by the prototype's report.
- Merging the programme branch into `future`.
