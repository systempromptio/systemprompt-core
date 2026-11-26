# Testing strategy

**Every check is deterministic and none calls a model provider.** The phone runs no agent loop, so there is no model on the device to script; what is scripted is the instance.

## 1. What must be shown

| Claim | Shown by |
|---|---|
| The server changes are correct and safe | Core tests in M1, with independent reviews of the three authorization paths |
| The shared core behaves correctly against an instance | Scenarios driven through `systemprompt-mobile`'s public surface against the scripted instance |
| Each app shows and does the right thing | UI tests on the simulator and the emulator against the same scripted instance |
| The two apps are the same product | The parity check on screens and states; scenarios passing on both; the tour side by side |
| It installs and starts on real systems | The evidence run; the manual walk on real phones |
| A core change did not break the apps | The drift gate on every template push |

## 2. Layers

| Layer | Scope | Runs at |
|---|---|---|
| Core unit and integration tests | Push channels against a recording stub provider; tokens; runs list; approval states; association documents; discovery | Core's push run |
| Client and platform-module tests | Typed calls and stream resume against the scripted instance; sealing against an in-memory keystore double supplied through the platform contract | Core's push run |
| Scenarios | A whole flow through the shared core | Stage gate |
| App unit tests | View state derived from core facts; formatting; navigation decisions | Stage gate, both runners |
| UI tests | The app driven on a simulator or emulator | Stage gate, both runners |
| Screenshot tour | Each state on a list, both platforms | Evidence run |
| Manual walk | Real push, real biometrics, real keystores, real installs | Before first use by anyone outside the maintainers |

## 3. The scripted instance

The harness's scripted instance (`../../harness/testing/02-scripted-instance.md`) is reused and gains what phones exercise:

| Addition | Behaviour |
|---|---|
| Runs | A scripted run: a sequence of events emitted on the per-run stream, including an approval request that waits for a decision |
| Approvals | Records each decision; scripts a timeout; scripts a competing decision from "another client" |
| Tokens | Records registrations and revocations |
| Push | A recording stub stands in for the two providers. A scenario asserts what would have been sent: the token, the category, and that the payload holds no content |
| Agents and chat | Scripted replies on the agent event stream |
| Streams | Drops a stream mid-event to exercise resume |
| Association documents and discovery | Served from the builder |

No real push is sent by any automated test. Real push is on the manual walk only.

## 4. Determinism rules

As the harness's: no wall-clock waits; a test waits for a specific event; time and identifiers that reach an assertion are injected or normalised; each test owns a fresh data root; a suite asserts its own case count; a failure that repeats gets a lane with a deliberate reproduction.

Two additions for UI tests:

- A UI test waits on an accessibility identifier that the screen sets when it has rendered from core state, never on a delay.
- Animations are disabled in the test configuration on both platforms.

## 5. Guard against spend and leakage

- No provider credential, push credential or signing secret is present in a push run.
- The apps under test resolve the instance only from the scripted instance's address.
- A scenario fails if the scripted instance receives a request it has no script for.
