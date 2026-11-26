# Testing strategy

**Every check is deterministic and none calls a model provider.** The harness is proven by replaying scripted model output through the real engine, the real governor and the real tools, and by comparing the result with an unmodified reference agent given the same script.

## 1. What must be shown

| Claim | Shown by |
|---|---|
| The harness builds, installs and starts on Windows and macOS | The three-system workflows; the screenshot tour run on real machines (`05-evidence.md`) |
| Its engine does the right thing for a given model output | Golden scenarios (`03-golden-scenarios.md`) |
| It is at least as capable as the reference agent where they overlap | The parity ledger (`03-golden-scenarios.md` section 6) and the migrated test catalogue (`04-test-migration.md`) |
| Every model call and tool call is governed | Scenarios whose expected trace includes the signing headers, the verdict call and the telemetry events, asserted from the scripted instance's request log |
| A later restructuring changed nothing | The same suites, unchanged, green before and after each stage |

## 2. The reference agent

OpenCode, unmodified, installed from its public release at a pinned version, used only as a comparison baseline in the parity job and as the source of a behaviour catalogue. It was chosen because users of the harness are moving from it, and because its headless mode and provider configuration allow it to be driven by a local scripted endpoint.

Nothing of it is linked, vendored or copied into the harness. Names of other projects do not appear in harness source, test names, strings or file names; the pin and the name live in workflow configuration and in this directory.

## 3. Layers

| Layer | Scope | Count today or planned | Runs at |
|---|---|---|---|
| Unit and crate tests | One crate through its public API | 942 at the lift | L2, three systems |
| Migrated tests | Behaviours catalogued from the reference agent's suite, re-expressed in Rust | Set by the catalogue | L2 |
| Scripted-instance tests | Sign-in, key trust, sync, acceptance, heartbeat, telemetry against a mock instance | About 40 planned | L2 |
| Golden scenarios | A whole session: prompt in, scripted turns, tools run, end state | 24 in the first set | L2, Windows and macOS |
| Parity comparison | The same scenarios through the reference agent; differences classified | Same 24 | L2, one system is enough |
| Screenshot tour | The packaged app in each state on a list | About 20 states | Stage closure, by an agent on real Windows and macOS machines |
| Manual walk | What no runner can show: elevation prompts, real keyrings across restarts, real displays | A written list | Before a fleet handover |

Engine and device suites live in core's test workspace; shell suites and the tour live in the template. All reach production crates only through their public interface. No test-only feature, module or environment switch exists in a production crate.

## 4. Why no inference is needed

A model's contribution to a session is a sequence of streamed messages: text, reasoning, tool calls with arguments. Given the same sequence, a correct agent does the same thing. The scripted instance serves that sequence byte for byte in each of the two wire protocols. What is under test is everything around the model: request assembly, streaming decode, tool-call assembly, permission rules, the verdict, tool execution, file effects, persistence, compaction, retries, telemetry.

What this does **not** test is prompt quality: whether a real model, given the harness's system prompt and tool descriptions, behaves well. That is a separate evaluation, it costs money, and it is out of scope for these stages. The strategy makes one concession to it at no cost: the request log records the exact prompt and tool schema each product sends, so the two can be read side by side.

## 5. Guard against accidental spend

- The workflow environment contains no provider credential.
- Tests resolve the instance and model endpoints only from the scripted instance's bound address.
- A network guard in the parity job fails the job if the reference agent or the harness opens a connection to any host other than loopback and the package sources named in the workflow.
- The scripted instance refuses a request it has no script for; it never falls back to a default reply. An unscripted request is a test failure, not a silent pass.

## 6. Determinism rules

- No wall-clock waits. A test waits for a specific event on the engine's subscription, the store's change feed, or the scripted instance's request log.
- Time, randomness and identifiers that reach an assertion are injected. Where they cannot be, they are normalised before comparison (`03-golden-scenarios.md` section 3).
- Each test owns a fresh data root and a fresh workspace directory.
- A test that never executes its body is a defect. Suites assert their own case counts against the scenario corpus and the catalogue.
- A failure that repeats is not a flake. It gets a lane with a deliberate reproduction.

## 7. Where each suite gates

| Stage | Suites that must be green to close it |
|---|---|
| S1 | Unit and crate tests on three systems |
| S2 | All of the above, golden scenarios, parity ledger classified, migrated tests, screenshot tour |
| S3 to S7 | Everything S2 established, unchanged, plus what the stage adds |

A stage that needs to change an existing scenario's expected trace says so in its stage file and has the change reviewed. Expected traces are not regenerated in bulk.
