# 08. Quality and gates

Three languages, one standard: behaviour lives in Rust under core's existing gates, and the native code is held to equivalent rules in its own tools.

## 1. Rust

Everything in core for this work, `systemprompt-mobile` included, is ordinary core code. Core's coding standards and every core gate apply without exception: typed identifiers, typed errors with sources, no stringly errors, owned background work, structured logging, no inline WHAT comments, no legacy paths, one compiled shape, no environment readers outside the allowlist, fail-closed policy, tests in the separate test workspace.

Additions specific to this work:

| Gate | Enforces |
|---|---|
| Phone closure | The phone composition links no server crate, no window toolkit, no engine, proxy, host or machine code |
| Phone targets compile | The shared crates and `systemprompt-mobile` check for the iOS and Android targets on every push |
| Binding surface | Only `systemprompt-mobile` exports across the boundary; no secret-bearing type is exported |
| Platform confinement | Target-system conditions only inside platform modules (the harness's gate, extended to phone targets) |

## 2. Swift and Kotlin

| Concern | iOS | Android |
|---|---|---|
| Formatting | The standard formatter, checked | The standard formatter, checked |
| Linting | The standard linter, warnings as errors | The standard static analysers, warnings as errors |
| Structure | One feature per directory; features do not import each other; design components do not import features | The same, as Gradle modules |
| No behaviour in the app | A lint rule fails on direct networking, token handling or cursor arithmetic in app code: those belong to the core | The same |
| Strings | Every user-visible string from the localisation catalogue | The same, from string resources |
| Secrets | No credential, key or token in source or resources; a scan runs on every push | The same |
| Foreign names | Names of other projects absent from source, strings and file names | The same |
| Dependencies | A short allowlist; a new package is a reviewed change | The same |

## 3. Binding drift

A change in core's exported surface can break the apps. One gate rebuilds the bindings from the pinned core and compiles both apps against them on every push to the template. A core change that alters the exported surface is called out in its changelog entry, and the semver report covers `systemprompt-mobile`.

## 4. Parity between the two apps

The apps are written twice, so they can diverge. Two guards:

- A shared list of screens and states in the extension, checked against each app's navigation table.
- The screenshot tour (`../testing/03-evidence.md`) captures the same states on both, side by side.

## 5. Security review

Independent, read-only review of: token registration; the approval step; the association files; the binding surface (that no secret can cross it); the managed-configuration lock.

## 6. Validation levels

As in the harness operating model. A lane checks only what it edited. A wave gate is one offline check per repository. Builds for phone targets, both apps, their tests and the scenario suites run once per stage on hosted runners; the evidence run follows on a real machine.
