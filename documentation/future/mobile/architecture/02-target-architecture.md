# 02. Target architecture

## 1. What the phone is

A client of the company's instance. Everything it shows and does happens on the server.

| The phone does | The phone does not |
|---|---|
| Sign in to an instance; hold a sealed credential | Run an agent loop |
| Chat with the instance's agents | Run tools, a shell or a language server |
| Start, watch, steer and cancel server-side agent runs | Attach to a session on someone's desktop |
| Approve or deny a run's tool requests | Call the model gateway directly |
| Receive push notifications; open the run or document they refer to | Manage other coding agents |
| Read documents and their comments | Hold provider keys or a loopback proxy |

Why this boundary: inference and governance already live on the instance, and server-side runs already exist. A phone that only observes and decides needs no engine, no file access and no background execution, which are the things phone operating systems restrict.

Named as out of scope, not forgotten: attaching to a desktop session (needs a relay through the instance); an on-device agent loop; a public store listing; a shared push relay.

## 2. Layers

```
SwiftUI app (iOS)                Jetpack Compose app (Android)        extension
  screens, navigation, system      screens, navigation, system
  integration                      integration
        │  generated Swift API           │  generated Kotlin API
        └──────────────┬─────────────────┘
              systemprompt-mobile                                      core
              the binding layer: one facade over the crates below
                       │
   systemprompt-device     systemprompt-client      shared crates
   sign-in, sealing,       typed calls, event       models, identifiers,
   key trust, sync         streams                  wire types
                       │
                  HTTPS and server-sent events
                       │
                 the instance                                          server
   identity · agents · agent runs · notifications · documents
```

- **The native apps own only presentation and system integration**: navigation, lists, text entry, the share sheet, biometric prompts, push registration, deep links, accessibility.
- **The shared core owns all behaviour**: what a session is, how a stream is consumed and resumed, when a token is renewed, what a notification means, how an approval is submitted. A rule fixed in Rust is fixed on both phones.
- **The instance owns truth**: runs, events, approvals, documents and policy.

## 3. Languages per repository

| Repository | Today | With mobile |
|---|---|---|
| Core | Rust | Rust only. One new crate, `systemprompt-mobile`, is still Rust; the Swift and Kotlin it produces are generated at build time and not committed |
| Template | Rust, web assets | Adds Swift (an Xcode project) and Kotlin (a Gradle project) under `extensions/mobile/` |
| Each install | As the template | Its fork of `extensions/mobile/`, with its own brand, bundle identifier and package name |

No Rust user-interface code is written for phones, and no Swift or Kotlin enters core.

## 4. Where each piece lives

| Piece | Home | Licence |
|---|---|---|
| Wire types, identifiers | Existing shared crates | As core |
| Typed calls for runs, approvals, streams, notifications, documents | `systemprompt-client` | As core |
| Sign-in, sealed credentials, key trust; iOS and Android platform modules | `systemprompt-device` | As core |
| The binding layer | `systemprompt-mobile` (new) | As core |
| Push channel, device records, approval step, association files, discovery entries | Existing server crates (`03-core-changes.md`) | As core |
| iOS app | Template `extensions/mobile/ios/` | MIT |
| Android app | Template `extensions/mobile/android/` | MIT |
| Brand, theme, icons | The same declarations the desktop shell uses; phone assets generated from them | MIT |
| Build and release workflows, packaging | Template | MIT |

## 5. The phone as a composition

The harness defines a device build as a composition of slices. The phone is the smallest one.

| Slice | On a phone |
|---|---|
| `device` | Yes: sign-in, key trust, sync of what the phone needs (the user's profile, policy, theme) |
| `client` (runs, agents, notifications, documents) | Yes. This slice is also what the desktop shell uses for its server-side run views |
| `governor`, `engine` | No. The phone makes no model call |
| `proxy`, host integrations, `machine`, `lockdown`, `runner`, `drive`, `update` | No. Updates come from the platform's installer |

A closure gate holds this: the phone's closure links no server crate, no window toolkit, and none of the slices marked no.

Instance control over members (`harness.members`) applies: an instance can disable a phone feature, for example documents, and the app hides it.

## 6. How phone, desktop, web and server link

| Link | Mechanism |
|---|---|
| One identity | The same account and the same instance; the phone is one more device on the user's record |
| Pairing a phone | The web admin or the desktop harness shows a QR code carrying a device-link exchange code. The phone scans it, redeems it, and is signed in without typing a password. Sign-in with a passkey or the instance's identity provider also works unaided |
| The same runs everywhere | A run started on the web or by a schedule appears on the phone; a run started on the phone appears on the web. There is one run list per user |
| Notifications to the right place | A push notification carries an identifier, not content. Tapping it opens the app at that run, approval or document by a deep link |
| Approvals | A run pauses on the server; any of the user's signed-in clients may decide; the first decision wins and the others update |
| Brand and theme | One declaration per product drives the desktop window, the iOS app and the Android app |
| Policy | The instance's manifest reaches the phone through the same sync as the desktop |
| Version floor | The discovery document carries a minimum version per platform; an app below it says so and links to the update |
