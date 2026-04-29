# Cowork build, release & versioning

`systemprompt-cowork` is a standalone Rust binary shipped from `bin/cowork/`. It plays two roles in one process: a **credential helper** (emits Anthropic's `inferenceCredentialHelper` JSON to stdout) and a **plugin / MCP sync agent** (pulls signed manifests from a `systemprompt-gateway` into the user's `org-plugins/` directory). On macOS and Windows it also runs as a tray app with a native settings GUI; Linux is CLI-only.

It ships across three OS targets: `x86_64-unknown-linux-gnu`, `aarch64-apple-darwin`, and `x86_64-pc-windows-msvc`. macOS Intel (`x86_64-apple-darwin`) is built separately and tracked under `cowork-mac-v*` tags rather than the main 3-OS matrix (the `macos-13` runner queue made the merged matrix unreliable).

## The three repos

```
systemprompt-core (public, source)        systemprompt-deploy (private, CI)
        │                                          │
        │ tag cowork-v*                            │ tag gateway-v*
        ▼                                          ▼
.github/workflows/release-sign.yml        .github/workflows/release.yml
3-OS cowork build + cosign keyless sign   gateway 4-arch build + deb/rpm + cosign
        │                                          │
        ▼                                          ▼
GH Releases on systemprompt-core          GH Releases on systemprompt-template
(cowork binaries + SHA256SUMS)            (gateway tarballs + SHA256SUMS.gateway)
        │                                          ▲
        │                                          │
        │ ┌────────────────────────────────────────┘
        ▼ │
systemprompt-template (public, fork target)
        │ tag cowork-v*
        ▼
.github/workflows/release.yml
downloads cowork artifacts from systemprompt-core's GH Release,
renames SHA256SUMS → SHA256SUMS.cowork, re-attaches to a template release
```

**Single rule**: the **tag prefix names the product**, regardless of which repo it lives in.

| Tag prefix | Repo | Product |
|---|---|---|
| `cowork-v*` | core (build+sign), template (re-bundle) | cowork binary |
| `cowork-mac-v*` | core | cowork macOS Intel |
| `gateway-v*` | deploy (build+sign), template (passive surface) | gateway server |
| `v*` (bare) | core, deploy | **legacy / deprecated** — see [versioning.md](versioning.md) |

Cowork and gateway version independently. Template's `cowork-v*` tag exists to give end users a single, branded release to download from — the binaries themselves are produced by core.

## Index

| Doc | Purpose |
|---|---|
| [architecture.md](architecture.md) | Repo responsibilities, capability matrix per OS, critical files |
| [build-and-release.md](build-and-release.md) | Step-by-step runbook: local builds, cutting a release, troubleshooting |
| [versioning.md](versioning.md) | Tag namespaces, workspace versioning, release cadence, consumer pinning |
| [platform-notes.md](platform-notes.md) | Per-OS reference (Linux / macOS / Windows), how to add a new platform feature |
