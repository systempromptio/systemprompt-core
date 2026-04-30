# Architecture: three repos, three targets

## Repo responsibilities

| Repo | Visibility | Owns | Tag patterns | Publishes to |
|---|---|---|---|---|
| `systemprompt-core` | public | Rust workspace (30 `systemprompt-*` crates), `bin/bridge/` binary source, signed cowork binary releases | `cowork-v*` (build+sign), `cowork-mac-v*` (macOS x86_64 separate track), `<crate>@<ver>` (cargo-ws cruft, never pushed) | crates.io, `systempromptio/systemprompt-core` GH Releases |
| `systemprompt-deploy` | **private** | Gateway CI/CD: cross-compile, deb/rpm, Docker image, Helm chart, Homebrew tap, Scoop bucket | `gateway-v*` | GHCR, `systempromptio/systemprompt-template` GH Releases (via `RELEASE_UPLOAD_TOKEN`), `charts.systemprompt.io` |
| `systemprompt-template` | public | Fork-and-compile evaluation template; consumer-facing GH Releases for both products | `cowork-v*` (re-bundles only); receives `gateway-v*` as an upload from deploy | `systempromptio/systemprompt-template` GH Releases |

> Bare `v*` is still accepted by both core and deploy for one transition cycle but is deprecated. See [versioning.md](versioning.md).

## Why three repos

- **core** stays focused on library and binary source. Downstream consumers depend on it via the `systemprompt` crates.io facade. No CI secrets for production registries live here.
- **deploy** hides production CI secrets, signing identities, and registry tokens that should not be visible in a public template. It also owns Helm charts and OS packages that are not part of the user-facing fork.
- **template** is what end users fork. It carries the application skeleton (`src/`, `extensions/`, `migrations/`, `services/`, `web/`, demos, install docs) and acts as the public Release surface for both gateway and cowork. Users read its README and follow `docs/install/*` and `docs/cowork/*`.

The pattern: source in core, machinery in deploy, surface in template. There is observed pollution where deploy-style machinery (Dockerfile, install scripts, docs-internal) has leaked into template — see `cleanup.md` in the template repo root for the migration list.

## Cowork capability matrix

| Capability | Linux x86_64 | macOS aarch64 | macOS x86_64 | Windows x86_64 |
|---|---|---|---|---|
| CLI (credential helper, sync, install) | ✅ | ✅ | ✅ | ✅ |
| Tray icon + native GUI | ❌ | ✅ (WKWebView) | ✅ | ✅ (WebView2) |
| MDM snippet generation | ❌ (`install/mdm.rs` errors) | ✅ (`.mobileconfig` profile) | ✅ | ✅ (registry write) |
| Managed-prefs read | ❌ | ✅ (`/Library/Managed Preferences/$USER/<domain>.plist`) | ✅ | ✅ (`HKLM\SOFTWARE\Policies\Claude` + HKCU) |
| Process discovery | `ps` | `/bin/ps` | `/bin/ps` | `tasklist` + `GetConsoleProcessList` (`winproc.rs`) |
| `.app` bundle | n/a | `bin/bridge/scripts/make-mac-app.sh` | same | n/a |
| Icon embedded in binary | n/a | n/a | n/a | `winresource` via `bin/bridge/build.rs` (ICO from `assets/app-icon.ico`) |
| Release matrix | main 3-OS (`v*`) | main 3-OS (`v*`) | **separate** `cowork-mac-v*` tag | main 3-OS (`v*`) |

## Critical files

| File | Role |
|---|---|
| `bin/bridge/Cargo.toml` | Binary crate, **not** part of the workspace; versioned independently. |
| `bin/bridge/build.rs` | Windows-only: embeds `assets/app-icon.ico` and PE metadata via `winresource`. |
| `justfile` (root, recipes `build-cowork`, `build-cowork-all`, `bundle-cowork-mac`) | Authoritative local-build commands. CI mirrors these. |
| `bin/bridge/src/integration/claude_desktop/{macos,windows,shared}.rs` | Per-OS integration with Claude Desktop (managed prefs, MDM, profile generation). |
| `bin/bridge/src/install/{bootstrap,mdm}.rs` | Install bootstrap (`chown` reset on Unix), MDM dispatch (`apply_mdm()` per OS). |
| `bin/bridge/src/winproc.rs` | Windows-only console attach + process listing. |
| `bin/bridge/scripts/make-mac-app.sh` + `make-icons.swift` | macOS `.app` bundling and `.icns` generation. |
| `.github/workflows/release-sign.yml` (this repo) | 3-OS matrix build + cosign keyless sign on `v*`. |
| `../systemprompt-deploy/.github/workflows/release.yml` | Gateway build + sign on `v*`. |
| `../systemprompt-template/.github/workflows/release.yml` | Cowork re-bundle on `cowork-v*`. |
