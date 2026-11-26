# Scenarios and UI tests

## 1. The scenario set

Each scenario is data: the scripted instance's configuration, the steps taken through the shared core's public surface, and the expected facts afterwards (what the core reports, what the instance received). The same scenario backs a UI test on each platform where it has a screen.

| # | Scenario | Proves | Screen |
|---|---|---|---|
| 1 | Discover an instance and sign in with the browser flow | PKCE; token sealed; session reported | Sign-in |
| 2 | Sign in with a passkey | The ceremony round trip; association document consulted | Sign-in |
| 3 | Pair by code | The link exchange; no password typed | Sign-in |
| 4 | First manifest: key trust prompt, accept | Trust on first use; policy applied | Sign-in |
| 5 | Manifest signed by a changed key | Sync stops until trusted; nothing applied | Preferences |
| 6 | List agents and open a chat; streamed reply | Stream consumed; message stored | Home, Chat |
| 7 | Stream dropped mid-reply | Resume from the cursor; no duplicate, no gap | Chat |
| 8 | Queue a run and watch it finish | Idempotent queue; events in order; outputs | Runs |
| 9 | List runs with a filter | Paging; only the caller's runs | Runs |
| 10 | Cancel a run | State change observed | Runs |
| 11 | Approval requested, allowed | Pending listed; decision sent once; run continues | Approvals |
| 12 | Approval denied with feedback | Feedback reaches the instance | Approvals |
| 13 | Approval decided elsewhere first | The phone's later decision receives the recorded outcome and changes nothing | Approvals |
| 14 | Approval times out | Shown as denied by timeout | Approvals |
| 15 | Register a push token; token changes | One live token per device | Preferences |
| 16 | A notification while in the foreground | Delivered in-app; no push recorded | Home |
| 17 | A notification while in the background | Push recorded with id, category and collapse key and no content | — |
| 18 | Open from a notification id | Resolved to the right run, approval or document | Navigation |
| 19 | Read a document with comments | Typed read; rendering inputs | Documents |
| 20 | Instance disables a feature through member policy | The feature's entry point is gone | Home |
| 21 | App below the minimum version | Sign-in blocked; update offered | Sign-in |
| 22 | Sign out | Token revoked on the instance; sealed credential removed | Preferences |
| 23 | Credential refused by the instance | Signed out; state says why | Any |
| 24 | Managed configuration locks the instance | No other instance can be added | Sign-in |

## 2. Running a scenario through the shared core

On the host, in core's test workspace: start the scripted instance; build the phone composition with an in-memory keystore supplied through the platform contract; run the steps through `systemprompt-mobile`'s Rust API; assert the reported facts and the instance's request log; assert no unscripted request and no unconsumed script.

These run fast, on every stage gate, and catch nearly every behavioural defect before a simulator starts. Because screen models are in core, a scenario also asserts the snapshot each screen would render, so the same assertion covers desktop, iOS and Android.

Each scenario belongs to a feature slice and runs only in compositions that include it; one extra scenario builds a composition with a single feature and asserts the others are absent.

## 3. UI tests

| | iOS | Android |
|---|---|---|
| Framework | The platform's UI testing framework | The platform's Compose UI testing and instrumentation |
| Target | Simulator on a macOS runner | Emulator on a Linux runner |
| Instance | The scripted instance's binary on the runner, reached at the host's address | The same |
| Sign-in | Scenario 3's pairing code, injected as a launch argument that is a real, documented debug-build feature | The same |
| Waits | Accessibility identifiers set when a screen has rendered from core state | The same |
| Push | Not delivered. The background-notification path is exercised by handing the app the notification id directly | The same |

One keeper suite per feature on each platform; a new regression is a new case in that suite.

The launch-argument sign-in exists in debug builds only and is compiled out of release builds by the platform's own build configuration. It is listed in the security review.

## 4. Parity

- A scenario with a screen must pass on both platforms. One platform failing is a defect, not a difference.
- The shared list of screens and states is checked against each app's navigation table at the M3 gate.
- The tour captures the same states on both for a person to compare.

## 5. What automated tests do not cover

Real push delivery; biometrics; keystores across a reboot; the system sign-in sheet's real behaviour; installation from the instance and from the beta channel; device-management configuration on a managed phone. These are the manual walk (`03-evidence.md` section 4).
