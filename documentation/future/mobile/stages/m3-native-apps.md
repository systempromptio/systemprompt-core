# M3. Native apps

**Goal.** A SwiftUI app and a Jetpack Compose app that are drawn from the shared client state machines, with every feature an opt-in module and navigation built from the admitted features.

**Entry.** M2 closed.

**Specification.** `../architecture/05-native-apps.md`, `../architecture/04-shared-core-and-bindings.md` section 7.

**Rule for this stage.** Every feature is two lanes, one per platform, opened together from one brief. The brief names the feature's state machine, the domain states it reports (loading, empty, content, error, stale, waiting), its actions, and the screens to draw. Both lanes draw from the same state and forward the same actions; neither holds domain state of its own, and each shapes its own screens. The two lanes own disjoint trees (`extensions/mobile/ios/**` and `extensions/mobile/android/**`) and never wait on each other.

## Wave A — app shells, two lanes, parallel

| Lane | Platform | Owned paths | Work |
|---|---|---|---|
| **M3-A1** | iOS | `ios/App/**`, `ios/Design/**`, `ios/Features/SignIn/**` | Entry point; navigation built from the list of admitted features; the single place model callbacks hop to the UI thread; design components over generated colours and type; sign-in (instance entry, browser sheet, passkey, pairing-code scanner, key-trust prompt, sync state); a home screen assembled from whichever features are present; the localisation catalogue |
| **M3-A2** | Android | `android/app/**`, `android/design/**`, `android/feature/signin/**` | The same |

Serial before the rest: every later lane builds on the shell and the design components.

**Wave gate (L1).** Both apps compile for the simulator and the emulator with no optional feature included.

## Wave B — three features, six lanes

| Feature | iOS lane | Android lane | State machine |
|---|---|---|---|
| Runs | **M3-B1** `ios/Features/Runs/**` | **M3-B2** `android/feature/runs/**` | Session stream; lists; schedules |
| Approvals | **M3-B3** `ios/Features/Approvals/**` | **M3-B4** `android/feature/approvals/**` | Approval queue; notification actions |
| Chat | **M3-B5** `ios/Features/Chat/**` | **M3-B6** `android/feature/chat/**` | Session stream; the composer with on-device dictation, camera, photo and file attachments, and the share-into-app entry point |

Six lanes exceed the cap of four: open Runs and Approvals first, Chat as lanes free up.

## Wave C — two features and assets, five lanes

| Feature | iOS lane | Android lane | State machine |
|---|---|---|---|
| Inbox | **M3-C1** `ios/Features/Inbox/**` | **M3-C2** `android/feature/inbox/**` | Inbox; push registration; opening from a notification |
| Documents and preferences | **M3-C3** `ios/Features/{Documents,Preferences}/**` | **M3-C4** `android/feature/{documents,preferences}/**` | Documents; preferences |

| Lane | Owned paths | Work |
|---|---|---|
| **M3-C5 Brand, theme and feature selection** | `extensions/mobile/shared/{assets,features}/**` | From `BrandDecl` and `ThemeDecl`: the iOS asset catalogue and colour set, the Android resources, icons, the bundle identifier and package name; contrast checked at generation. From the composition's named slices: which feature targets the iOS project and which feature modules the Android project include |

**Wave gate (L1).** Both apps compile with every feature included and with only one; formatters and linters clean.

## Stage gate (L2)

Hosted runners, unsigned builds:

- iOS on macOS and Android on Linux: build; unit tests; lint and structure rules.
- The drift gate.
- Each app's navigation table matches the shared screen-and-state list for the features included.
- Three compositions build on each platform: every feature; runs and approvals only; chat only. A feature left out is absent from the binary.

Behavioural proof is M5's; this gate proves the apps build, are structured as specified, are the same shape, and honour opt-in.

## Exit criteria

1. Stage gate green.
2. Every feature in `05-native-apps.md` section 2 exists on both platforms and is reachable when included.
3. No networking, token handling, stream-cursor logic or domain state lives in Swift or Kotlin.
4. There is no administration screen.

## Decisions reserved to people

- The reference app's icon and name, and which features the reference composition includes.
