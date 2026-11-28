# S9. Server half and orchestration

**Goal.** The instance knows the harness as a capability, controls which device members run, runs the native engine headless on the server, and the device can start and observe sessions in other agents.

**Entry.** S5 closed. This stage overlaps S6 to S8; its lanes work on new paths.

**Specification.** `../architecture/02-target-architecture.md` section 2, `../architecture/04-host-integrations.md` section 5, `../architecture/03-composition-contract.md` section 4.

## Wave A — three lanes, parallel

| Lane | Repository | Work |
|---|---|---|
| **S9-A1 Server capability** | core | A `harness` capability unit on the server: host settings, including the per-marketplace settings the device reads (O7); publishing the `harness.members` manifest block whose shape S5 fixed, with `host_extensions` absorbed or kept as S5 decided (`../architecture/04-host-integrations.md` section 3.3); harness-targeted bundle entries (themes as data, view configuration) |
| **S9-A2 Composition digest** | core | The device already applies `harness.members` at admit (S5). This lane adds the composition digest to device evidence and the admin view that shows it |
| **S9-A3 Runner crate** | core | `systemprompt-runner`: the headless adapters lifted out of the server's agent-runs crate, with no server dependency, used by both sides. Two pieces of entry work come first, below. Then the fifth adapter, for the native engine: `host_for(Harness)` stops answering `Unsupported`, and the migration that admits `harness` to the agent-run host check lands with it |

**Before S9-A3 moves an adapter.** The skeleton stage found that the server cannot link the runner as its crate graph stands (`../architecture/02-target-architecture.md` section 3):

1. `systemprompt-runtime`, a kernel crate, depends on `systemprompt-agent-runs` for one type, the admitted run origins. With `agent-runs` linking the runner, that edge would put a device crate in the kernel closure, which the closure gate refuses. **The edge goes** (D49): the type moves to the shared-layer contract and the kernel names no capability crate. The integrator lands this alone, ahead of the wave, because it touches a kernel crate; the closure gate is not given an exception.
2. `agent-runs` and the runner are in different capability units, and the capability gate refuses a dependency between two. This is open decision O5. It must be taken before the wave opens; S9-A3 does not work around it with an allowlist entry.

Until both are done `agent-runs` keeps its own adapters and names no device crate.

Then **S9-A4 (review)** of A1 and A2: an instance can narrow what a device runs and can never widen what the device's own policy allows.

## Wave B — three lanes, parallel

| Lane | Repository | Work |
|---|---|---|
| **S9-B1 Scheduled native sessions** | core | Schedules and other server-side origins start `Remote` sessions, reusing S4's remote runtime |
| **S9-B2 Runner slice** | core | The device starts and observes sessions in other agents through the runner facet; one transcript model, one permission surface, one telemetry stream |
| **S9-B3 Fleet view** | template | A view of every agent on the machine: health, sessions, verdicts |
| **S9-B4 Server-side features on the desktop and web** | template | Desktop views for chats with instance agents, approvals and the notification inbox over the shared client state machines. A session view in the web console: a small script consuming the same session stream, rendered server-side on first load |

## Wave C — deferred features, one lane per capability

Each is an ordinary capability unit: a domain crate, an entry crate with HTTP, command-line and device surfaces, and views in the template. They are independent of each other.

| Lane | Capability |
|---|---|
| **S9-C1** | Routines that chain sessions and other agents, built on scheduled sessions (S4) and the runner |
| **S9-C2** | Background subagents: a task call returns at once, the child session runs while the parent continues, and its answer reaches the parent as a later message (parity scenario 19) |
| **S9-C3** | Further capabilities as they are designed |

Scheduled sessions and memory are not here: they are built in S4 and shown on the desktop in S5. Following a session from a phone is the mobile set's.

**Catalogue rows deferred to this stage.** 26 rows of the reference-test catalogue wait here (`../testing/04-test-migration.md` section 1.1): remote control and pairing between devices, routines and scheduled automations, driving and observing other agents. Each lane that delivers one of those features ports its rows in the same wave. Rows about scheduled automations are re-read against S4, which now builds scheduled sessions; rows about pairing a second device are re-read against the mobile set. A row whose feature this stage decides not to build is reclassified with its reason, by a person.

## Stage gate (L2)

- A golden scenario in which the scripted instance requires a member, disables another, and the composition admits accordingly.
- A scenario in which a scheduled `Remote` session runs and appears in the owner's session list on every client.
- A scenario in which the device drives a second agent through the runner against the scripted instance.
- Everything earlier, unchanged.

## Exit criteria

1. Stage gate green.
2. No open high-rated review finding on `harness.members` or the runner.
3. The closure gate and the capability gate hold with the runner linked by both sides and no exception entry for it.
4. No catalogue row deferred to this stage is `open` without a reason.

## Decisions reserved to people

- Whether instance control over device members ships enabled.
- The order of the deferred features.
- O5, the runner's unit, before wave A opens.
