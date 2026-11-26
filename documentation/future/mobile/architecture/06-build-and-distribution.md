# 06. Build and distribution

## 1. Can the apps be installed without the stores?

| Platform | Answer |
|---|---|
| Android | **Yes.** The instance serves a signed APK and the user installs it |
| iOS | **No.** Every route to an iPhone passes through Apple. The chosen route is private: the app is offered to named organisations and is never a public listing |

Facts below marked *confirmed* were read on the vendors' own pages on 2026-11-26; *unconfirmed* were not found there and must be checked in M0 before they are relied on.

## 2. Android

### 2.1 Routes

| Route | How it works | Use |
|---|---|---|
| **Self-hosted APK** (chosen) | The install's release publishes a signed APK; the instance's download route serves it with its checksum; the discovery document names it. The user allows installs from the browser once | Default for every install |
| Managed private app | An organisation publishes the app privately to its own managed store; the store account is created for the organisation at no fee (*confirmed*). Installed and updated by device management | Organisations that manage devices |
| Public store | Not planned | — |

### 2.2 Developer verification

Google now requires developers of apps installed on certified devices to be verified.

- In force since 30 September 2026 for participating stores in Brazil, Indonesia, Singapore and Thailand, on Android 8 and later (*confirmed*).
- Directly sideloaded apps are not yet covered; the vendor states the requirement "won't apply to your app yet" and that verification expands to all apps on certified devices worldwide in 2027 (*confirmed*).
- A path exists for developers who distribute only outside the public store: an account in the vendor's developer console, a one-off fee, and registration of each package name with its signing keys (*confirmed*).
- Users can still install from an unverified developer through an advanced flow with a one-day wait, and developers through the debug bridge (*confirmed*). Neither is acceptable for ordinary users.
- Enterprise apps in a managed store are exempt (*confirmed*).

**Decision:** the reference app's publisher and every install register as verified developers and register their package names and signing keys before the worldwide expansion. Self-hosting then keeps working unchanged.

### 2.3 Updates

The app reads the discovery document's minimum and current versions. When a newer APK exists it offers the download; below the minimum it says so and blocks sign-in. The platform installer performs the update; the app never replaces itself silently.

## 3. iOS

### 3.1 Routes

| Route | Who can install | Apple's involvement | Use |
|---|---|---|---|
| **Custom apps** (chosen) | Organisations the publisher names, through Apple Business Manager or Apple School Manager. The organisation distributes by device management or redemption codes (*confirmed*) | App Review (*confirmed*). Offered only to the named organisations | Production |
| **TestFlight** (chosen) | Up to 100 internal and 10,000 external testers (*confirmed*). Build lifetime: commonly 90 days (*unconfirmed*) | External builds are reviewed (*confirmed*) | Pilots and pre-release |
| Enterprise in-house | The organisation's own employees only. Requires 100 or more employees, a legal entity, a verification interview; 299 USD a year (*confirmed*). The only route in which the instance could serve the app itself | Apple approves the organisation, not each build | An install whose customer already holds this membership |
| Ad hoc | Registered devices only; an annual cap per device type (limit *unconfirmed*) | None per build | Development |
| Web distribution in the EU | Users in the EU only; the publisher must be authorised and meet financial or scale criteria; builds are notarised; a 5% commission on transactions (*confirmed*) | Authorisation and notarisation | Not planned |
| Public store | Anyone | App Review | Not planned |

### 3.2 What the chosen route asks of each party

| Party | Needs |
|---|---|
| The publisher (the template's reference app, or an install for its brand) | An Apple developer membership; the app in the publisher's account; each customer organisation's identifier |
| The customer organisation | An Apple Business Manager account; device management or redemption codes to hand the app to its people |
| The user | Nothing beyond installing from the organisation's catalogue |

A customer with no Business Manager account cannot use the chosen route. The fallbacks are TestFlight for a bounded group, or that customer's own enterprise membership. This is stated to installs up front.

### 3.3 Review

Custom apps and external TestFlight builds are reviewed. The app's purpose (a client for an organisation's own instance, signing in to that organisation's server) must be demonstrable to a reviewer: the reference instance provides a review account and sample data for that.

## 4. The build graph

```
one push to the template (or an install)
   │
   ├─ Rust core ──► cargo build per phone target
   │                 ├─ iOS: static libraries ──► XCFramework ─┐
   │                 └─ Android: shared libraries ──► AAR ─────┤
   │                 └─ binding generator ──► Swift module, Kotlin package
   │
   ├─ brand and theme declarations ──► generated assets and identifiers
   │
   ├─ iOS app:     Xcode build ──► archive ──► signed build ──► TestFlight / custom apps
   └─ Android app: Gradle build ──► signed APK ──► the install's release ──► served by the instance
```

| Step | Runner | Tooling |
|---|---|---|
| Rust for Android | Linux | The Android native toolchain; cross-compilation |
| Rust for iOS | macOS | The Apple toolchain |
| Android app | Linux | Gradle |
| iOS app | macOS | Xcode's command-line build and export |
| Signing and upload | The same jobs, on release only | The platform's signing tools; a store-connect key for upload |

Push runs build, lint and test unsigned on both runners and store nothing. Release runs sign and publish. This is the same split the desktop workflow uses.

## 5. Signing

| Secret | Holder | Used by |
|---|---|---|
| Android signing key | Each publisher, for its own package name | The release job; registered for developer verification |
| Apple distribution certificate and provisioning | Each publisher, for its own bundle identifier | The release job |
| Store-connect key | Each publisher | Upload to TestFlight and custom apps |
| Push credentials | Each **instance** (`07-push-and-approvals.md`) | The server, not the build |

The template signs the reference app with the maintainers' identities; each install signs its own brand with its own. Core never signs anything.

## 6. Managed configuration

Both platforms let device management hand configuration to an app. The apps read: the default instance address, the pinned manifest key, and whether other instances may be added. With these set, a managed phone opens directly at the organisation's sign-in and cannot be pointed elsewhere: the phone's equivalent of the desktop's administrator-installed form.

## 7. In the release cycle

1. Core releases and publishes crates, `systemprompt-mobile` included. No binary.
2. Template lane: bump the core pin; build every flavour including `mobile`; sign and publish the reference apps.
3. Install lanes: bump the pin; build the server image, the desktop artefacts and the phone apps in one release; publish the APK to the instance's download source and the iOS build to its channels.

The apps report the core version they were built from. A phone app on an older core minor than its instance keeps working within the instance's version window.
