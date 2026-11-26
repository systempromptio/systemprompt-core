# 05. The native apps in the extension

## 1. Layout

```
extensions/
  web/  mcp/  cli/  harness/            existing and from the harness set
  mobile/
    ios/                                 an Xcode project
      App/                               entry point, navigation, deep links, push registration
      Features/                          one directory per screen group
      Design/                            components over generated brand and theme assets
      Tests/  UITests/
    android/                             a Gradle project
      app/                               entry point, navigation, deep links, push registration
      feature/                           one module per screen group
      design/                            components over generated brand and theme resources
      src/test/  src/androidTest/
    shared/                              scripts that build the Rust library and generate bindings
flavours/
  mobile/                                flavour entry, setup, build, smoke
packaging/
  mobile/                                signing configuration templates, export options, store metadata
```

The two apps mirror each other's structure and names so a change is made in the same place twice.

## 2. Features and screens

Each feature is an opt-in slice (`02-target-architecture.md` section 5): a state machine in core and one feature module in each app. A build includes only the features its composition names.

| Feature | Screens | Opt-in |
|---|---|---|
| Sign-in | Choose or enter an instance; sign in (browser sheet, passkey); scan a pairing code; key-trust prompt; sync state | Always present |
| Chat | Agents; a conversation with streamed text and tool activity; a composer with dictation, camera, photo and file attachments | Yes |
| Runs | Run list with filters; a run's live events; outputs; start; cancel; schedules (create, pause, run now) | Yes |
| Approvals | A request with the tool and a redacted summary; allow or deny with feedback; recent decisions | Yes |
| Inbox | The notification feed; push | Yes |
| Documents | List; a document with comments | Yes |
| Preferences | Appearance; notification preferences; sign out; diagnostics | Yes |

Home is not a feature: it is assembled from whichever features are present (active runs, pending approvals, recent chats) and is empty of anything not opted into.

There is no administration on the phone: no management of users, devices, policy, the marketplace or the instance. That stays in the web console.

Each feature is a lane's worth of work per platform (`../stages/m3-native-apps.md`). Stream handling, ordering and races are in the shared state machine; a feature module shapes and draws its screens and forwards actions.

## 3. Brand and theme

One declaration per product serves the desktop window and both phone apps.

- `BrandDecl` (name, vendor, identifiers, default instance, icon, displayed version) and `ThemeDecl` (palettes, type scale) are data in the extension.
- A build step generates from them: an asset catalogue and colour set for iOS, colour and string resources for Android, app icons in each platform's sizes, and the bundle identifier and package name.
- At run time the same declarations reach the apps through the binding layer, so a theme pinned or changed by the instance applies without a rebuild.
- Contrast is checked when the declaration is built, as on the desktop.

## 4. What a fork owns

| A fork changes freely | A fork cannot change |
|---|---|
| Brand, icons, product name, default instance, identifiers | How credentials are sealed; that the native side holds no secret |
| Themes | Manifest trust and the key-trust prompt's existence |
| Any screen, navigation and layout | The approval rule: only the owner decides, a timeout denies |
| Which optional features are shown | Features the instance disables or requires |
| Its own Rust members for logic | The push payload carrying no content |

## 5. The limit of extensibility on phones

On the desktop a member can contribute a view at a mount point. On phones it cannot, and the reason is structural: the screens are Swift and Kotlin, compiled by Xcode and Gradle, not Rust linked into a composition.

| Extension mechanism | Desktop | Phone |
|---|---|---|
| Rust member for logic (settings, observers, projections, notification categories) | Yes | Yes |
| Theme and brand as data | Yes | Yes, the same data |
| Content by signed manifest (agents, commands, themes) | Yes | Yes |
| A contributed screen | Yes, at mount points | No. A fork edits the native code it forked |
| A server-defined screen | — | A small set of generic screens (a list, a detail, a form) driven by data from an instance capability, so a new server capability can appear on phones without an app release |

The last row is the phone's substitute for mount points and is deliberately modest: three generic layouts, not a layout language.

## 6. System integration each app provides

| Integration | iOS | Android |
|---|---|---|
| Sign-in sheet | The system authentication session | A custom tab |
| Passkeys | The system passkey sheet, enabled by the association file | The credential manager, enabled by the asset-links file |
| Deep links | Universal links and the brand's custom scheme | Verified app links and the brand's custom scheme |
| Push | Registration, categories with allow and deny actions, a service extension only if content must be fetched before display | A messaging service, a notification channel per category, actions |
| Dictation | The system's on-device speech recognition into the composer | The system's on-device speech recognition into the composer |
| Camera and photos | The system camera and photo picker; images attached to a message | The same |
| Files | The system document picker and the share sheet into the app | The system document picker and share targets |
| Managed configuration | Reads the default instance and pinned key from device-management app configuration | The same, from managed restrictions |
| Biometric gate | Before unsealing, when policy asks | Same |
| Accessibility | Dynamic type, VoiceOver labels | Font scale, TalkBack labels |

An approval can be decided from the notification itself, after the device's own unlock.

## 6a. Input: dictation, camera and attachments

| Input | Design |
|---|---|
| Dictation | Speech is turned into text on the device by the operating system and placed in the composer for the user to edit before sending. No audio leaves the phone and nothing is added to the instance. Where on-device recognition is unavailable for a language, dictation is not offered; it does not fall back to a network service |
| Camera and photos | A photo or picked image is attached to the message. It is uploaded through the instance's existing upload routes and referenced from the prompt |
| Files | A document from the picker, or one shared into the app from another app, is attached the same way |
| Limits | Size and type limits are the instance's, reported to the app, and enforced on the server. An attachment over the limit is refused before upload starts |
| Sessions with a workspace | In a `Remote` session with a workspace, an attachment is placed in the session's workspace as well as referenced in the prompt |
| Policy | An instance may disable attachments or dictation through member policy |

Dictation is input assistance, not a voice conversation: the agent does not speak, and there is no continuous listening.

## 7. Licence

`extensions/mobile/` is MIT, as the template is. It links core's crates through the generated library, as the web and desktop extensions link them.
