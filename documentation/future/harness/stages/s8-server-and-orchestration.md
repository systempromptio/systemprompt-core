# S8. Server half and orchestration

**Goal.** The instance knows the harness as a capability, controls which device members run, runs the native engine headless on the server, and the device can start and observe sessions in other agents.

**Entry.** S4 closed. This stage overlaps S5 to S7; its lanes work on new paths.

**Specification.** `../architecture/02-target-architecture.md` section 2, `../architecture/04-host-integrations.md` section 5, `../architecture/03-composition-contract.md` section 4.

## Wave A — three lanes, parallel

| Lane | Repository | Work |
|---|---|---|
| **S8-A1 Server capability** | core | A `harness` capability unit on the server: host settings; publishing the `harness.members` manifest block whose shape S4 fixed; harness-targeted bundle entries (themes as data, view configuration) |
| **S8-A2 Composition digest** | core | The device already applies `harness.members` at admit (S4). This lane adds the composition digest to device evidence and the admin view that shows it |
| **S8-A3 Runner crate** | core | `systemprompt-runner`: the headless adapters lifted out of the server's agent-runs crate, with no server dependency, used by both sides |

Then **S8-A4 (review)** of A1 and A2: an instance can narrow what a device runs and can never widen what the device's own policy allows.

## Wave B — three lanes, parallel

| Lane | Repository | Work |
|---|---|---|
| **S8-B1 Scheduled native runs** | core | Schedules and other server-side origins start native sessions with a sandbox workspace on a runner node, reusing S3's run handler |
| **S8-B2 Runner slice** | core | The device starts and observes sessions in other agents through the runner facet; one transcript model, one permission surface, one telemetry stream |
| **S8-B3 Fleet view** | template | A view of every agent on the machine: health, sessions, verdicts |
| **S8-B4 Server-side features on the desktop and web** | template | Desktop views for chats with instance agents, approvals and the notification inbox over the shared client state machines. A session view in the web console: a small script consuming the same session stream, rendered server-side on first load |

## Wave C — deferred features, one lane per capability

Each is an ordinary capability unit: a domain crate, an entry crate with HTTP, command-line and device surfaces, and views in the template. They are independent of each other.

| Lane | Capability |
|---|---|
| **S8-C1** | Tasks and routines |
| **S8-C2** | Remote control |
| **S8-C3** | Brain surface on the device |

## Stage gate (L2)

- A golden scenario in which the scripted instance requires a member, disables another, and the composition admits accordingly.
- A scenario in which a scheduled native session runs in a sandbox workspace and appears in the owner's session list on every client.
- A scenario in which the device drives a second agent through the runner against the scripted instance.
- Everything earlier, unchanged.

## Exit criteria

1. Stage gate green.
2. No open high-rated review finding on `harness.members` or the runner.

## Decisions reserved to people

- Whether instance control over device members ships enabled.
- The order of the deferred features.
