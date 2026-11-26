# 02. Target architecture

## 1. What the phone is

A client of the company's instance. Everything it shows and does happens on the server. The native engine has three typed execution modes ([harness set](../../harness/architecture/11-execution-modes.md)). A phone uses one: `Remote`. A session started from a phone runs in the instance's sandbox, which covers chatting, planning and work on a connected repository. Because the instance is the record of every session in every mode, the phone also sees and can steer or approve sessions that are running on the user's desktop. A phone executes nothing itself.

| The phone does | The phone does not |
|---|---|
| Sign in to an instance; hold a sealed credential | Run an agent loop |
| Chat with the instance's agents | Run tools, a shell or a language server |
| Start, watch, steer and cancel server-side agent runs | Run a session locally: `Unrestricted` and `Sandboxed` are desktop modes |
| Approve or deny a run's tool requests | Call the model gateway directly |
| Receive push notifications; open the run or document they refer to | Manage other coding agents |
| Read documents and their comments | Hold provider keys or a loopback proxy |

Why this boundary: inference and governance already live on the instance, and server-side runs already exist. A phone that only observes and decides needs no engine, no file access and no background execution, which are the things phone operating systems restrict.

Named as out of scope, not forgotten: running the runtime on a phone; a public store listing; a shared push relay.

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

- **The native apps own only drawing and system integration**: layout, lists, text entry, the share sheet, biometric prompts, push registration, deep links, accessibility.
- **The shared core owns all behaviour.** A state machine per feature holds the domain state, consumes the streams and offers the actions. A rule fixed in Rust is fixed on both phones and on the desktop. How a screen groups and presents that state is each app's own.
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

## 5. The phone as a composition of opt-in features

The harness defines a device build as a composition of slices. A phone build is the `device` slice plus whichever feature slices it opts into. Nothing is included by default.

| Slice | Brings | Opt-in |
|---|---|---|
| `device` | Sign-in, sealed credential, key trust, sync, the session | Always present |
| `chat` | Conversations with the instance's agents | Yes |
| `runs` | Start, list, watch and cancel server-side runs | Yes |
| `approvals` | Pending requests and decisions | Yes; needs `runs` |
| `inbox` | Notification feed and push | Yes |
| `documents` | Read documents and comments | Yes |
| `preferences` | The user's own settings: appearance, notification preferences, sign out, diagnostics | Yes |

Each feature slice is one unit across all its parts: a state machine in core, a feature module in each app, its scenarios, and its tour states. Opting in happens in three places that agree:

1. **At build:** the composition names the slices; the Android project includes only those feature modules and the iOS project only those feature targets. A slice left out is not in the binary.
2. **By the instance:** member policy (`harness.members`) can disable a slice that was built in; the app hides it.
3. **In the app:** navigation is built from the list of admitted slices, never hard-coded.

Not on the phone at all, in any composition: administration of the instance, users, devices, policy or the marketplace; `governor`, `engine`, `proxy`, host integrations, `machine`, `lockdown`, `runner`, `drive`, `update`. Administration stays in the web console.

A closure gate holds this: a phone build links no server crate, no window toolkit, and no slice it did not name.

## 5a. What is shared with the desktop harness

Everything where bugs and security issues live; nothing that decides how a screen looks.

| Layer | Desktop | iOS | Android |
|---|---|---|---|
| Sign-in, sealing, key trust, sync | `systemprompt-device` | same crate | same crate |
| Typed calls and streams | `systemprompt-client` | same crate | same crate |
| Client state machines per feature (stream reduction and resume, the approval queue, notification routing) | the `client` feature slices | same code, through the bindings | same code, through the bindings |
| What a screen shows (grouping, formatting, field choice) | its own | its own | its own |
| Brand and theme | one declaration | same declaration | same declaration |
| Instance control over features | member policy | same | same |
| Scenarios and the scripted instance | one suite | same suite | same suite |
| Screen-and-state list | one list | same list | same list |
| Drawing and system integration | Rust window toolkit | SwiftUI | Jetpack Compose |

Every client renders a session from the same server-published stream, so a session, an approval and the inbox are the same everywhere. The web console consumes the same stream with a small script; it shares the protocol and the scenario suite, not the Rust code.

The shared layer is deliberately state machines, not screen-shaped models: a phone's compact approval card and a desktop's full diff review need different data, and a fork must be able to redraw a screen without a core change.

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
