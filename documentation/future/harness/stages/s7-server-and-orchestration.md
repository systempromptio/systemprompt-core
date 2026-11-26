# S7. Server half and orchestration

**Goal.** The instance knows the harness as a capability, controls which device members run, runs the native engine headless on the server, and the device can start and observe sessions in other agents.

**Entry.** S3 closed. This stage overlaps S4 to S6; its lanes work on new paths.

**Specification.** `../architecture/02-target-architecture.md` section 2, `../architecture/04-host-integrations.md` section 5, `../architecture/03-composition-contract.md` section 4.

## Wave A — three lanes, parallel

| Lane | Repository | Work |
|---|---|---|
| **S7-A1 Server capability** | core | A `harness` capability unit on the server: host settings; publishing the `harness.members` manifest block whose shape S3 fixed; harness-targeted bundle entries (themes as data, view configuration) |
| **S7-A2 Composition digest** | core | The device already applies `harness.members` at admit (S3). This lane adds the composition digest to device evidence and the admin view that shows it |
| **S7-A3 Runner crate** | core | `systemprompt-runner`: the headless adapters lifted out of the server's agent-runs crate, with no server dependency, used by both sides |

Then **S7-A4 (review)** of A1 and A2: an instance can narrow what a device runs and can never widen what the device's own policy allows.

## Wave B — three lanes, parallel

| Lane | Repository | Work |
|---|---|---|
| **S7-B1 Native headless adapter** | core | The server's agent runs gain an adapter for the native engine: spawn `run`, read its event stream, classify the outcome |
| **S7-B2 Runner slice** | core | The device starts and observes sessions in other agents through the runner facet; one transcript model, one permission surface, one telemetry stream |
| **S7-B3 Fleet view** | template | A view of every agent on the machine: health, sessions, verdicts |

## Wave C — deferred features, one lane per capability

Each is an ordinary capability unit: a domain crate, an entry crate with HTTP, command-line and device surfaces, and views in the template. They are independent of each other.

| Lane | Capability |
|---|---|
| **S7-C1** | Tasks and routines |
| **S7-C2** | Remote control |
| **S7-C3** | Brain surface on the device |

## Stage gate (L2)

- A golden scenario in which the scripted instance requires a member, disables another, and the composition admits accordingly.
- A scenario in which the server-side adapter runs the native engine against the scripted instance and the transcript appears in a device session.
- A scenario in which the device drives a second agent through the runner against the scripted instance.
- Everything earlier, unchanged.

## Exit criteria

1. Stage gate green.
2. No open high-rated review finding on `harness.members` or the runner.

## Decisions reserved to people

- Whether instance control over device members ships enabled.
- The order of the deferred features.
