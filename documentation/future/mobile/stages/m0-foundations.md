# M0. Foundations

**Goal.** The facts the design rests on are confirmed, and every path later stages own exists.

**Entry.** Harness S0 closed (core crate skeleton conventions and gates; the template's flavour and workflow conventions).

## Wave A — four lanes, parallel

| Lane | Repository | Owned paths | Work |
|---|---|---|---|
| **M0-A1 Source check** (read-only) | — | None. Updates `../risks-and-decisions.md` section 1 through the integrator | Re-read the vendors' own pages for every fact marked *unconfirmed* in `../architecture/06-build-and-distribution.md`: beta build lifetime; the ad hoc device cap; the current terms of custom apps; the developer-verification timeline and what it asks of a publisher outside the public store. Check that the binding generator and the Android native toolchain versions to be pinned exist and support the listed targets. Record what was confirmed, with the date |
| **M0-A2 Crate and gates** | core | The `systemprompt-mobile` crate directory (manifest, empty library with module documentation, library build types); the layer gate's table; the phone closure gate; a workflow job that checks the shared crates for the phone targets | Create the ninth device crate, empty. Add the closure gate (`../architecture/08-quality-and-gates.md` section 1). Add the target check to core's push run, Linux for Android targets and macOS for iOS targets |
| **M0-A3 Template scaffold** | template | `extensions/mobile/{ios,android,shared}/**`; `flavours/mobile/**`; `packaging/mobile/**` | An empty Xcode project and an empty Gradle project that build a blank app. The flavour entry with `artefact: mobile`. Build scripts that are stubs. Formatter and linter configuration for both languages |
| **M0-A4 Workflows** | template | A new `mobile` workflow | A Linux job that builds the blank Android app and a macOS job that builds the blank iOS app for the simulator. Unsigned; nothing stored |

**Wave gate (L1).** Core: offline check of the new crate; the gates. Template: both blank projects configure.

## Stage gate (L2)

- Core's push run green, including the phone-target check.
- The template's `mobile` workflow green on both runners.

## Exit criteria

1. Every fact in `06-build-and-distribution.md` is marked confirmed with a date, or the design has been changed to not depend on it.
2. `systemprompt-mobile` exists, empty, inside the layer and closure gates, and the shared crates check for all six phone targets.
3. The template builds a blank app for each platform on hosted runners.

## Decisions reserved to people

- Opening the developer accounts the chosen routes need (an Apple developer membership; a developer-console account for verification) and who holds them.
- The bundle identifier and package name of the reference app.
