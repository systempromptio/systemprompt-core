# S6. Platforms, packaging, signing

**Goal.** One platform contract with three real implementations; Linux headless; packaged and signed artefacts for Windows, macOS and Linux from one template push.

**Entry.** S4 closed for waves A and B (they work on core's device crates). S5 closed for wave C (it packages the agents-only form).

**Specification.** `../architecture/08-platforms-and-packaging.md`.

Windows and macOS already build, test and (on Windows) package from S1. This stage replaces stubs with real implementations and adds what a release needs.

## Wave A — platform modules, four lanes, parallel

One lane per concern group; each owns its `platform` module in one crate.

| Lane | Concerns | Owner crate |
|---|---|---|
| **S6-A1** | Secret sealing and keyrings; owner-only files; base directories | `device` |
| **S6-A2** | Shell selection; output decoding; child-tree kill | `engine` |
| **S6-A3** | Single instance; scheduler entries; elevation; machine policy locations | `device`, `machine` |
| **S6-A4** | The confinement gate: target-system conditions only inside `platform` modules | Gate scripts |

After S5, the bridge's implementations are the base for A1 and A3; before S5, A1 and A3 implement macOS and Linux fresh and S5-E1 reconciles them.

**Wave gate (L1).** Offline check on Linux; the confinement gate.

## Wave B — headless Linux and shells, three lanes, parallel

| Lane | Repository | Work |
|---|---|---|
| **S6-B1 Linux headless** | core | `run` and `serve` on Linux; the golden scenarios join the Linux job |
| **S6-B2 macOS shell** | template | Menu-bar item, deep-link registration, application bundle |
| **S6-B3 Linux shell** | template | Gated on verifying the pinned window toolkit on Wayland and X11. If it does not hold, this lane reports and stops; headless remains the Linux product |

**Wave gate (L1).** Offline check.

## Wave C — packaging and signing, three lanes, parallel

| Lane | Repository | Owned paths | Work |
|---|---|---|---|
| **S6-C1 Packaging** | template | `packaging/harness/{macos,linux}/**` | Disk image and installer package for macOS; static headless binary, portable image and distribution packages for Linux; all reading `package --describe` |
| **S6-C2 Release workflow** | template | The release workflow | Moved from core: bills of materials, provenance attestation, keyless signing, release publication, generated manifests for the package channels |
| **S6-C3 Update slice** | core, then template | `update` | The bridge's updater as a slice with its fail-closed policy; feed and identity from the brand declaration |

## Stage gate (L2)

- Golden scenarios on Windows, macOS and Linux.
- Packaged artefacts for three systems from one push; each installs and reports its version.
- The screenshot tour on macOS from the packaged bundle; on Linux if S6-B3 held.
- A sealed sign-in survives an application restart on each system (asserted by the tour: sign in, quit, relaunch, capture the signed-in state).
- Signing verified by the release workflow's own verification step.

## Exit criteria

1. Stage gate green.
2. No stub implementation remains; a missing platform implementation is a compile error.
3. The manual walk has been done on real machines and its result recorded.

## Decisions reserved to people

- Signing identities and where their secrets live.
- Whether Linux desktop is a supported product or headless only.
- App icon artwork.
- Any release.
