# M4. Build and distribution

**Goal.** A person outside the build team can install the app: on Android from the instance, on iOS from the beta channel and as a custom app offered to their organisation.

**Entry.** M3 closed. The accounts from M0's reserved decisions exist.

**Specification.** `../architecture/06-build-and-distribution.md`.

## Wave A — three lanes, parallel

| Lane | Repository | Owned paths | Work |
|---|---|---|---|
| **M4-A1 Android release** | template | The Android signing configuration; the release job for Android in the `mobile` workflow; `packaging/mobile/android/**` | Signed release APK from a release run. Published to the install's release source with its checksum, where the instance's download route (M1-B4) finds it. The update prompt driven by the discovery document |
| **M4-A2 iOS release** | template | The iOS signing configuration and export options; the release job for iOS; `packaging/mobile/ios/**` | Archive, sign and upload from a release run to the beta channel. The record needed to offer the build as a custom app to named organisations. Review notes and the review account on the reference instance |
| **M4-A3 Managed configuration** | template, core | `ios/App/ManagedConfig/**`, `android/app/**/managedconfig/**`; the `Instances` object's lock in `systemprompt-mobile` | Read the default instance, pinned manifest key and "other instances allowed" from device-management configuration; the core enforces the lock, the apps only pass the values in |

**Wave gate (L1).** The workflow files parse; an unsigned dry run of each release job completes up to the signing step.

## Wave B — people, then one lane

1. A person provides the signing secrets to the template's release environment.
2. A person registers the Android package name and signing key for developer verification.
3. **M4-B1 First release run**: the integrator triggers one release; reads both jobs.

## Stage gate (L2)

- A release run produces a signed APK that the reference instance serves, and that installs on an emulator from the instance's download address.
- The same run uploads an iOS build that appears on the beta channel.
- The discovery document of the reference instance lists both apps with their versions.
- An evidence run (`../testing/03-evidence.md`) installs the released Android APK on an emulator and the released iOS build on a simulator and reaches the sign-in screen.

## Exit criteria

1. Stage gate green.
2. A person has installed the Android app on a real phone from the instance, and the iOS app on a real phone from the beta channel.
3. The steps an install follows to ship its own branded apps are written in the template's flavour page.

## Decisions reserved to people

- Every signing identity and secret.
- Submitting a build for review; offering the custom app to an organisation.
- Any release.
