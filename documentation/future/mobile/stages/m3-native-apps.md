# M3. Native apps

**Goal.** A SwiftUI app and a Jetpack Compose app with the same seven screen groups, containing presentation and system integration only.

**Entry.** M2 closed.

**Specification.** `../architecture/05-native-apps.md`.

**Rule for this stage.** Every screen group is two lanes, one per platform, opened together from one brief. The brief lists the screens, each screen's states, the core calls it uses and the strings it needs. The two lanes own disjoint trees (`extensions/mobile/ios/**` and `extensions/mobile/android/**`) and never wait on each other.

## Wave A — app shells, two lanes, parallel

| Lane | Platform | Owned paths | Work |
|---|---|---|---|
| **M3-A1** | iOS | `ios/App/**`, `ios/Design/**`, `ios/Features/SignIn/**` | Entry point; navigation; the single place listener callbacks hop to the UI thread; design components over generated colours and type; the sign-in group (instance entry, browser sheet, passkey, pairing-code scanner, key-trust prompt); the localisation catalogue |
| **M3-A2** | Android | `android/app/**`, `android/design/**`, `android/feature/signin/**` | The same |

Serial before the rest: every later lane builds on the shell and the design components.

**Wave gate (L1).** Both apps compile for the simulator and the emulator (the one check a lane on its own machine may be unable to run; the integrator runs it).

## Wave B — three groups, six lanes

| Group | iOS lane | Android lane | Core calls |
|---|---|---|---|
| Home | **M3-B1** `ios/Features/Home/**` | **M3-B2** `android/feature/home/**` | Agents, recent chats, active runs, pending approvals |
| Chat | **M3-B3** `ios/Features/Chat/**` | **M3-B4** `android/feature/chat/**` | Open, send, watch |
| Runs | **M3-B5** `ios/Features/Runs/**` | **M3-B6** `android/feature/runs/**` | Queue, list, get, cancel, watch, outputs |

Six lanes exceed the cap of four: open the two Home lanes and the two Runs lanes first, Chat as lanes free up.

## Wave C — three groups and assets, seven lanes

| Group | iOS lane | Android lane | Core calls |
|---|---|---|---|
| Approvals | **M3-C1** `ios/Features/Approvals/**` | **M3-C2** `android/feature/approvals/**` | Pending, decide; notification actions |
| Notifications and documents | **M3-C3** `ios/Features/Inbox/**` | **M3-C4** `android/feature/inbox/**` | Feed, read, token registration, resolve; documents, comments |
| Settings and governance | **M3-C5** `ios/Features/Settings/**` | **M3-C6** `android/feature/settings/**` | Instances, devices, preferences, appearance, sync state, diagnostics, sign out |

| Lane | Owned paths | Work |
|---|---|---|
| **M3-C7 Brand and theme generation** | `extensions/mobile/shared/assets/**` | From `BrandDecl` and `ThemeDecl`: the iOS asset catalogue and colour set, the Android resources, icons in each platform's sizes, the bundle identifier and package name. Contrast checked at generation. Needs harness S3 for the declarations as data; before it, a local copy of the same schema |

**Wave gate (L1).** Both apps compile; formatters and linters clean.

## Stage gate (L2)

Hosted runners, unsigned builds:

- iOS on macOS: build; unit tests; the app's own lint and structure rules.
- Android on Linux: build; unit tests; lint and structure rules.
- The drift gate.
- The parity check: each app's navigation table matches the shared list of screens and states.

Behavioural proof is M5's; this gate proves the apps build, are structured as specified and are the same shape.

## Exit criteria

1. Stage gate green.
2. Every screen in `05-native-apps.md` section 2 exists on both platforms and is reachable.
3. The "no behaviour in the app" rule passes: no networking, token handling or cursor logic in Swift or Kotlin.

## Decisions reserved to people

- The reference app's icon and name.
