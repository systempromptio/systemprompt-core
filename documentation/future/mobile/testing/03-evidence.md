# Evidence

As for the harness, evidence is a development process, not continuous integration. Hosted runners build and test and store nothing. Screenshots and the evidence folder come from one command run by an agent session attached to a real Mac, at stage closure.

A Mac is sufficient for both platforms: it runs the iOS simulator and the Android emulator.

## 1. Two kinds of proof

| Kind | Where | Proves | Stored |
|---|---|---|---|
| Hosted runs | Linux runner (Android, core); macOS runner (iOS) | Builds, lints, unit tests, UI tests, scenarios, the drift gate | Nothing beyond the run's log and summary |
| Evidence run | A real Mac, by an attached agent session | The apps install and start; every tour state renders on both platforms; suites pass on that machine | A local, untracked folder, reported by the agent |

## 2. The screenshot tour

Simulators and emulators can be driven and captured directly, so the tour needs no window-capture workaround.

1. Build both apps against core `future`.
2. Start the scripted instance with the tour's configuration.
3. For each platform: boot a named device image; install the app; sign in with a pairing code; for each state, drive the app there with the UI test framework and capture.
4. Write each capture with its state id, platform, device image and commit.

| # | State | Themes |
|---|---|---|
| 1 | Enter an instance | Light, dark |
| 2 | Sign-in options | Light |
| 3 | Pairing-code scanner | Light |
| 4 | Key trust prompt | Light |
| 5 | Home: agents, active runs, a pending approval | Light, dark |
| 6 | Chat with streamed reply and tool activity | Light, dark |
| 7 | Run list, filtered | Light |
| 8 | A run's live events | Light, dark |
| 9 | Run outputs | Light |
| 10 | Approval request | Light, dark |
| 11 | Approval decided elsewhere; approval timed out | Light |
| 12 | Notification feed | Light |
| 13 | A document with comments | Light |
| 14 | Settings: instances, devices, notifications, appearance | Light, dark |
| 15 | Sync error; changed-key state | Light |
| 16 | Feature disabled by the instance | Light |
| 17 | Below minimum version | Light |
| 18 | Signed out by a refused credential | Light |
| 19 | Large text size | Light |
| 20 | A second brand and theme | Light, dark |

Captures are checked automatically only for being non-blank and the expected size. They are evidence for a person, laid out side by side by platform, not a pixel gate.

## 3. The evidence command and folder

One command on the Mac: build; run the scenario suite; run both UI test suites; run both tours; write the folder; print its index.

```
evidence/mobile/
  index.md                       commits, toolchains, device images, timings
  tests/                         results for every suite, with counts
  scenarios/                     per scenario: pass or fail, on the core and on each platform
  screenshots/<state>/ios.png
  screenshots/<state>/android.png
  side-by-side.md                every state, both platforms, in one page
  packages/                      artefact names, sizes, hashes
  sizes.md                       library size per target; app size per platform
```

`index.md` opens with: suites and counts; scenarios passing on core, iOS and Android; tour states captured per platform; packages built; "no provider, push or signing credential present".

The agent that ran it reports the index and the side-by-side page. A person looks at the side-by-side page; that look is an exit criterion of M5.

## 4. The manual walk

On one real iPhone and one real Android phone, before anyone outside the maintainers uses the app:

- Install: Android from the instance's download address; iOS from the beta channel.
- Sign in three ways: browser sheet, passkey, pairing code scanned from the web admin.
- Receive a real push with the app closed; confirm the lock screen shows no content; open it to the right run.
- Decide an approval from the notification after unlock; decide another inside the app.
- Biometric gate when policy asks.
- Kill the app mid-stream; reopen; the chat resumes without a gap.
- Airplane mode during a decision; reconnect; the recorded outcome is shown.
- Sign out; confirm the device's token is gone from the instance.
- On a managed phone, if one is available: the instance is pre-set and locked.

Its result is recorded in the commit that closes M5.

## 5. What a stage closure cites

The commits tested in each repository, the hosted runs that were green, and the evidence index. For M5, also the manual walk.
