# M5. Parity and proof

**Goal.** Deterministic evidence that both apps do the same right things against an instance, with screenshots, at no inference cost.

**Entry.** M3 closed; harness S2 closed (the scripted instance). Runs alongside M4.

**Specification.** `../testing/`.

## Wave A — three lanes, parallel

| Lane | Repository | Owned paths | Work |
|---|---|---|---|
| **M5-A1 Scenarios** | core | Scenario data and a runner in core's test workspace, under a mobile area; scripted-instance additions for push, tokens, runs list and approvals | The scenario set of `../testing/02-scenarios-and-ui-tests.md` section 1, driven through `systemprompt-mobile`'s public surface on the host. A recording stub in place of the two push providers |
| **M5-A2 iOS UI tests and tour** | template | `ios/UITests/**`; the iOS half of the tour script | UI tests that drive the app on the simulator against the scripted instance for each scenario that has a screen; the screenshot tour |
| **M5-A3 Android UI tests and tour** | template | `android/app/src/androidTest/**`; the Android half of the tour script | The same on the emulator |

**Wave gate (L1).** Offline check of the core test crates; the UI test targets compile.

## First full run

The integrator pushes and reads the hosted runs. Failures are grouped: a core defect (one fix lane in core), an app defect (one fix lane per platform), a test defect (a fresh lane on the test paths). The two platforms disagreeing on a scenario is always a defect in one of them, never an accepted difference.

## Stage gate (L2)

| Check | Must hold |
|---|---|
| Scenarios through the shared core | All pass on the host and for the phone targets where a runner exists |
| UI tests | All pass on the iOS simulator (macOS runner) and the Android emulator (Linux runner) |
| Parity | Each scenario with a screen passes on both platforms |
| Inference spend | None; no provider credential anywhere |
| Evidence run | An agent session on a real Mac runs the evidence command: both tours complete, the evidence folder is produced, a person looks at the screenshots side by side |
| Manual walk | Done on one real iPhone and one real Android phone: sign-in by browser, passkey and pairing code; a real push received and opened; an approval decided from the notification; biometric gate; sign-out removes the token |

## Exit criteria

1. Stage gate green at one commit per repository.
2. The manual walk's result is recorded in the closing commit.
3. `documentation/future/mobile/` content that now describes shipped behaviour is moved into the current documentation.

## Decisions reserved to people

- Having the Mac online and linked for the evidence run.
- Providing the two real phones for the manual walk.
