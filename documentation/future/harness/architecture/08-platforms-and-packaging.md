# 08. Platforms and packaging

Target: Windows, macOS and Linux, each with a desktop window. Linux comes first, headless, because the engine and the proxy need no window and a Linux `run` and `serve` are what servers and the `agent-runs` adapter need.

## 1. Starting points

| | Bridge | Harness |
|---|---|---|
| Windows | Ships; window; policy hive; DACLs | Builds and packs; never launched |
| macOS | Ships; window; managed preferences; Keychain | Type-checks and tests only |
| Linux | Ships headless (`proxy`); no window | Not built |
| Keyrings | Windows native, Apple Keychain, Secret Service and keyutils | Windows only |
| Platform code style | Real implementations per system | `cfg(windows)` with a stub beside it (50 sites) |

The bridge is ahead on every row except the window toolkit. Its platform code is the base; the harness's Windows-only code is checked against it and deleted where it duplicates.

## 2. One platform contract

Stubs are replaced by a `platform` module per concern with one trait and three real implementations, so a missing implementation is a compile error on that target. `cfg(target_os)` appears only inside `platform` modules (a gate enforces it).

| Concern | Windows | macOS | Linux | Owner |
|---|---|---|---|---|
| Secret sealing | DPAPI, Credential Manager | Keychain | Secret Service, keyutils; with no keyring, sign-in is memory only | `device` |
| Owner-only files | DACLs | `0600`, `O_NOFOLLOW` | Same | `device` |
| Base directories | `%LOCALAPPDATA%`, `%ProgramData%` | `~/Library/Application Support` | XDG | `device` |
| Machine policy | `HKLM\SOFTWARE\Policies\…`, SYSTEM policy-writer task | Managed preferences profile (user withdraws it by hand) | Files under `/etc` | `machine` |
| Scheduler | Task Scheduler | launchd | systemd user units | `machine` |
| Elevation | UAC | Authorization prompt | `pkexec` or documented manual step | `machine` |
| Single instance | Named pipe | Unix socket in the data root | Same | `device` |
| Deep links | Registry scheme | `Info.plist` URL types | `.desktop` `x-scheme-handler` | extension `shell` |
| Shell for the tool | PowerShell 7, Git Bash, Windows PowerShell, cmd | `zsh`, `bash`, `sh` | `bash`, `sh` | `engine` |
| Output decoding | Console code pages | UTF-8 | Locale charset | `engine` |
| Child-tree kill | Job objects | Process groups | Process groups, `PR_SET_PDEATHSIG` | `engine` |
| Window, tray, notifications | Toolkit; tray | Toolkit; menu-bar item | Toolkit on Wayland and X11, required (**to verify**; if the pinned version does not support it, the pin is raised or patched); tray where the desktop has one | extension `shell` |

Each product must carry its own identity across all of these: app id, URL scheme, data root, task names. They come from `BrandDecl`, so two brands and several channels install side by side.

## 3. Packaging

Owned by the extension. Core produces no artefact.

| Target | Artefact | Signing | Channels |
|---|---|---|---|
| Windows x64 (arm64 later) | Per-user NSIS installer; an administrator MSI or `install --apply` path for MDM | Authenticode | Scoop, direct download, MDM |
| macOS universal | `.app` in a `.dmg`; `.pkg` for MDM | Developer ID, notarization | Homebrew tap, direct download, MDM |
| Linux x64, arm64 | Static headless binary; AppImage, `.deb`, `.rpm` for desktop | Detached signature | Existing APT and RPM repositories |

- `release-sign.yml` (CycloneDX SBOMs, provenance attestation, cosign keyless, GitHub release) moves from core to the template. The reference builds are signed with the identities the bridge release uses today; installs copy the workflow and sign their own brand with their own identity. Core never signs a fork.
- Packaging scripts read identity from `<binary> package --describe` (JSON from `BrandDecl`). The brand table exists once; today the harness writes it twice.
- Homebrew and Scoop manifests are generated in the release job, ending the manual drift.
- Nothing ships unsigned once an external user exists.

## 4. Update

The bridge's `update/` moves to core's `update` slice with its fail-closed policy: when the last-sync sentinel is unreadable, corrupt or from another gateway, automatic staging is withheld and a start-up fault is recorded. The feed and signing identity belong to the product, declared in `BrandDecl`. This brings auto-update into scope for the harness earlier than its own plan assumed, because the bridge's fleet already relies on it.

## 5. Build matrix after the change

| Where | Runner | Runs |
|---|---|---|
| Core push run | `ubuntu-latest` | All gates; device crate tests; headless compositions built and booted against the mock instance |
| Core push run, scoped | `windows-latest`, `macos-latest` | `platform` modules and native device tests, only when a device crate changed |
| Template push run | all three | Check, test, release build, package, smoke test to the sign-in gate |
| Template and install release | all three | Sign and publish |

Core's native work shrinks from today's full `bridge-native` matrix on every push to a scoped check. The window toolkit is compiled only in the template and in installs.

## 6. Manual verification that no CI replaces

Before the fleet handover, on real machines: Windows 10 and 11 (standard user and administrator, with and without an MDM-pinned key), macOS (Apple silicon and Intel), one Linux desktop on Wayland and one on X11. The walk is the harness handover's first-run list plus the bridge's: sign-in by loopback and by paste code; sealed sign-in surviving a restart; sync; a governed call on each protocol; a denied tool; each shell with a non-UTF-8 code page; background job killed with its tree; snapshot revert; Drive sync; each managed host configured, repaired after drift and removed; the policy writer applying a change without a prompt; uninstall leaving only foreign entries.
