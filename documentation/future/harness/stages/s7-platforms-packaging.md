# S7. Platforms, packaging, signing

**Goal.** One platform contract with three real implementations; Linux headless; packaged and signed artefacts for Windows, macOS and Linux from one template push.

**Entry.** S1 closed for waves A and B (they work on core's device crates). S6 closed for wave C (it packages the bridge composition).

**Specification.** `../architecture/08-platforms-and-packaging.md`.

Windows and macOS already build, test and (on Windows) package from S1. This stage replaces stubs with real implementations and adds what a release needs.

## Wave A — platform modules, four lanes, parallel

One lane per concern group; each owns its `platform` module in one crate.

| Lane | Concerns | Owner crate |
|---|---|---|
| **S7-A1** | Secret sealing and keyrings; owner-only files; base directories and the single data-root layout | `systemprompt-device` |
| **S7-A2** | Shell selection; output decoding; child-tree kill | `systemprompt-engine` |
| **S7-A3** | Single instance; scheduler entries; elevation; machine policy locations | `systemprompt-device`, `systemprompt-machine` |
| **S7-A4** | The confinement gate: target-system conditions only inside `platform` modules | Gate scripts |

After S6, the bridge's implementations are the base for A1 and A3; before S6, A1 and A3 implement macOS and Linux fresh and S6-E1 reconciles them.

**Wave gate (L1).** Offline check on Linux; the confinement gate.

## Wave B — headless Linux and shells, three lanes, parallel

| Lane | Repository | Work |
|---|---|---|
| **S7-B1 Linux headless** | core | `run` and `serve` on Linux; the golden scenarios join the Linux job |
| **S7-B2 macOS shell** | template | Menu-bar item, deep-link registration, application bundle |
| **S7-B3 Linux shell** | template | Required. Verify the pinned window toolkit on Wayland and X11 first. If it does not hold, the lane's work becomes raising or patching the toolkit pin, with every view re-checked on the screenshot tour on all three systems |

**Wave gate (L1).** Offline check.

## Wave C — packaging and signing, three lanes, parallel

| Lane | Repository | Owned paths | Work |
|---|---|---|---|
| **S7-C1 Packaging** | template | `packaging/harness/{macos,linux}/**` | Disk image and installer package for macOS; static headless binary, portable image and distribution packages for Linux; all reading `package --describe` |
| **S7-C2 Release workflow** | template | The release workflow | Moved from core: bills of materials, provenance attestation, keyless signing, release publication, generated manifests for the package channels |
| **S7-C3 Update slice** | core, then template | `update` | The bridge's updater as a slice with its fail-closed policy; feed and identity from the brand declaration |

## Stage gate (L2)

- Golden scenarios on Windows, macOS and Linux.
- Packaged artefacts for three systems from one push; each installs and reports its version.
- The screenshot tour from the packaged build on Windows, macOS and Linux.
- A sealed sign-in survives an application restart on each system (asserted by the tour: sign in, quit, relaunch, capture the signed-in state).
- Signing verified by the release workflow's own verification step.

## Exit criteria

1. Stage gate green.
2. No stub implementation remains; a missing platform implementation is a compile error.
3. The manual walk has been done on real machines and its result recorded.

## Decisions reserved to people

- Moving the bridge's existing signing identities and their secrets to the template's release workflow. Installs sign their own brand with their own identity.
- App icon artwork.
- Any release.
