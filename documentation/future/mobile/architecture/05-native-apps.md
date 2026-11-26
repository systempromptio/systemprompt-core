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

Each feature is an opt-in slice (`02-target-architecture.md` section 5): a screen model in core and one feature module in each app. A build includes only the features its composition names.

| Feature | Screens | Opt-in |
|---|---|---|
| Sign-in | Choose or enter an instance; sign in (browser sheet, passkey); scan a pairing code; key-trust prompt; sync state | Always present |
| Chat | Agents; a conversation with streamed text, tool activity and attachments | Yes |
| Runs | Run list with filters; a run's live events; outputs; start; cancel | Yes |
| Approvals | A request with the tool and a redacted summary; allow or deny with feedback; recent decisions | Yes |
| Inbox | The notification feed; push | Yes |
| Documents | List; a document with comments | Yes |
| Preferences | Appearance; notification preferences; sign out; diagnostics | Yes |

Home is not a feature: it is assembled from whichever features are present (active runs, pending approvals, recent chats) and is empty of anything not opted into.

There is no administration on the phone: no management of users, devices, policy, the marketplace or the instance. That stays in the web console.

Each feature is a lane's worth of work per platform (`../stages/m3-native-apps.md`), and less than it would otherwise be, because the screen model is shared: a feature module draws a snapshot and forwards actions.

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
| Managed configuration | Reads the default instance and pinned key from device-management app configuration | The same, from managed restrictions |
| Biometric gate | Before unsealing, when policy asks | Same |
| Accessibility | Dynamic type, VoiceOver labels | Font scale, TalkBack labels |

An approval can be decided from the notification itself, after the device's own unlock.

## 7. Licence

`extensions/mobile/` is MIT, as the template is. It links core's crates through the generated library, as the web and desktop extensions link them.
