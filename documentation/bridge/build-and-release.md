# Build & release runbook

## Local builds

From the root of `systemprompt-core`:

```bash
just build-bridge                              # host triple
just build-bridge aarch64-apple-darwin         # specific target
just build-bridge-all                          # all four release targets
just bundle-bridge-mac aarch64-apple-darwin    # binary + .app bundle (Info.plist + AppIcon.icns)
```

`build-bridge-all` produces:
- `aarch64-apple-darwin`
- `x86_64-apple-darwin`
- `x86_64-pc-windows-msvc`
- `x86_64-unknown-linux-gnu`

Release profile (pinned in `bin/bridge/Cargo.toml`) matches CI: `opt-level = "z"`, `lto = "fat"`, `codegen-units = 1`, `strip = "symbols"`. Rebuilding with `--release` from scratch on a cold cache takes 3–5 minutes per target.

## Cutting a bridge release

Bridge ships in two coordinated pushes: one in `systemprompt-core` (build + sign), one in `systemprompt-template` (re-bundle for end users).

### 1. Build & sign in `systemprompt-core`

1. Bump `bin/bridge/Cargo.toml` `version` and update `bin/bridge/CHANGELOG.md`.
2. Commit.
3. Tag and push:
   ```bash
   git tag bridge-vX.Y.Z
   git push origin bridge-vX.Y.Z
   ```
   **Never `git push --tags`** — the working tree carries hundreds of stale `<crate>@<ver>` tags from past `cargo-ws` runs.

   `release-sign.yml` still listens on bare `v*` for one transition cycle, but the prefixed `bridge-v*` is the canonical trigger. New releases should use the prefix.
4. `.github/workflows/release-sign.yml` runs the 3-OS matrix (`x86_64-unknown-linux-gnu` / `aarch64-apple-darwin` / `x86_64-pc-windows-msvc`), generates `SHA256SUMS`, cosign-keyless-signs every artifact, and creates a GitHub Release on `systempromptio/systemprompt-core` (tag = `bridge-vX.Y.Z`) containing:
   - `systemprompt-bridge-<target>` binaries (+ `.exe` for Windows)
   - `<file>.sig` and `<file>.pem` cosign artifacts per binary
   - `SHA256SUMS`, `SHA256SUMS.sig`, `SHA256SUMS.pem`
5. Verify a binary locally:
   ```bash
   cosign verify-blob \
     --certificate-identity-regexp='https://github.com/systempromptio/systemprompt-core/' \
     --certificate-oidc-issuer='https://token.actions.githubusercontent.com' \
     --signature systemprompt-bridge-x86_64-unknown-linux-gnu.sig \
     --certificate systemprompt-bridge-x86_64-unknown-linux-gnu.pem \
     systemprompt-bridge-x86_64-unknown-linux-gnu
   ```

### 2. Re-bundle in `systemprompt-template`

End users download from template's Releases page, not core's. The template workflow re-attaches the signed binaries with branded release notes.

```bash
cd ../systemprompt-template
git tag bridge-vX.Y.Z
git push origin bridge-vX.Y.Z
```

This triggers `.github/workflows/release.yml` which:
1. Downloads bridge artifacts from `systempromptio/systemprompt-core` release tagged `vX.Y.Z` (workflow input `bridge_tag`, default `v0.3.3` — override via `workflow_dispatch` for backports).
2. Optionally downloads the macOS x86_64 zip from a separate `bridge-mac-vX.Y.Z` release (input `bridge_mac_x64_tag`, default `bridge-mac-v0.3.2`).

   Note: template's `release.yml` `bridge_tag` default is currently the legacy `v0.3.3`. After the next core release lands as `bridge-vX.Y.Z`, update that default to match.
3. Renames `SHA256SUMS*` → `SHA256SUMS.bridge*` so they don't collide with the gateway's `SHA256SUMS.gateway*`.
4. Creates / updates the template release `bridge-vX.Y.Z` and uploads everything in `dist/`.

To re-publish without retagging (e.g. if release notes need fixing or you want to bundle a different `bridge_tag`):

```bash
gh workflow run release.yml \
  -R systempromptio/systemprompt-template \
  -f tag=bridge-vX.Y.Z \
  -f bridge_tag=vX.Y.Z \
  -f bridge_mac_x64_tag=bridge-mac-vX.Y.Z
```

### 3. Scoop bucket fan-out (Windows)

After the core release is published, `.github/workflows/scoop-bridge.yml` fires on `release: published` and updates the manifest at `systempromptio/scoop-bucket` (`bucket/bridge.json`). Trigger filter: only releases whose tag starts with `bridge-v`. Other tag prefixes (gateway, mac, legacy `v*`) are ignored.

The workflow:
1. Resolves the version from the tag (`bridge-v0.4.0` → `0.4.0`).
2. Downloads `systemprompt-bridge-x86_64-pc-windows-msvc.exe` from the core release and computes its SHA256.
3. Writes `bucket/bridge.json` with the URL, hash, and a `#/systemprompt-bridge.exe` fragment so Scoop renames the binary on install (short, stable name on PATH).
4. Commits to `systempromptio/scoop-bucket` using `SCOOP_BUCKET_TOKEN` (a PAT with contents:write on the bucket repo — same secret pattern as the gateway's `scoop.yml` in deploy).

End-user install:

```powershell
scoop bucket add systemprompt https://github.com/systempromptio/scoop-bucket
scoop install systemprompt/bridge
```

Scoop downloads via PowerShell, so SmartScreen is bypassed even without Authenticode signing. `scoop update bridge` picks up new releases via the manifest's `checkver` regex (`bridge-v([\d.]+)`), which intentionally ignores bare `v*` and `bridge-mac-v*` tags.

To re-run without retagging:

```bash
gh workflow run scoop-bridge.yml \
  -R systempromptio/systemprompt-core \
  -f tag=bridge-vX.Y.Z
```

**Prerequisite secret**: `SCOOP_BUCKET_TOKEN` must exist on `systemprompt-core` repo secrets, scoped to `systempromptio/scoop-bucket` with contents:write.

### 4. macOS x86_64 special case

The main `release-sign.yml` matrix does **not** include `x86_64-apple-darwin` — `macos-13` runner queue times made it unreliable. Intel Mac builds are tagged separately in core as `bridge-mac-v*` and produced by their own workflow. Template's bundler picks the latest `bridge-mac-v*` via `bridge_mac_x64_tag` input. Only re-tag the Intel build when there's a meaningful change for that platform; it can lag the main matrix by patch versions.

## Package-manager distribution (bridge)

| Channel | Status | Workflow | Bucket/tap repo | Signing prereq |
|---------|--------|----------|-----------------|----------------|
| GitHub Releases | active | `release-sign.yml` | — | cosign keyless |
| Scoop (Windows) | active | `scoop-bridge.yml` | `systempromptio/scoop-bucket` | none |
| Homebrew Cask (macOS) | planned | `homebrew-cask-bridge.yml` (TBD) | `systempromptio/homebrew-tap` | Developer ID + notarization |
| winget (Windows) | planned | TBD | `microsoft/winget-pkgs` (PR-based) | Authenticode (Azure Trusted Signing) |
| `.deb` / `.rpm` | deferred | TBD | attached to GH Release | optional GPG repo signing |

Both bucket repos (`scoop-bucket`, `homebrew-tap`) already exist and are driven by the gateway today. Adding bridge is per-channel: a new manifest file + workflow per repo, no bootstrap.

## Cutting a gateway release (`systemprompt-deploy`)

Independent of bridge. Lives in the private deploy repo.

1. Bump workspace version (`cargo ws version --all patch --no-git-push --yes`).
2. Tag `gateway-vX.Y.Z` and push only that tag (`git push origin gateway-vX.Y.Z`). The bare `v*` trigger is still wired up for one transition cycle but is being removed.
3. `.github/workflows/release.yml` cross-compiles `linux-amd64`, `linux-arm64`, `darwin-arm64` (darwin x86_64 is removed; see workflow comment at line 44), packages `services/` + `migrations/` + MCP extension manifests + `web/`, generates `SHA256SUMS.gateway`, cosign-signs it, and uploads to the **template repo's** GitHub Releases via `RELEASE_UPLOAD_TOKEN`.
4. The `release: published` event on the template repo fans out to deploy's other workflows (`docker.yml`, `helm.yml`, `homebrew.yml`, `scoop.yml`); deferred: `apt.yml`, `rpm.yml`, `winget.yml`.

The two-product separation: bridge releases never trigger gateway workflows, and gateway releases never trigger bridge workflows. They coexist on the same template Releases page under different tag prefixes (`bridge-v*` vs `v*`).

## Troubleshooting

**Windows icon not embedded**
`build.rs` is Windows-only and depends on `winresource`. Confirm `bin/bridge/assets/app-icon.ico` exists and is a valid ICO. PE metadata (FileDescription, ProductName, CompanyName) is also set in `build.rs`.

**`cargo ws` skipped a crate's version bump**
`cargo ws version` sometimes leaves a stale `version = "X.Y.Z"` pin in root `[workspace.dependencies]` for crates it thinks are unchanged. This breaks `cargo update` for downstream consumers. Grep `Cargo.toml` for the prior version string and hand-fix before publishing.

**SQLx cache drift on publish**
Run `just sqlx-prepare-publish` to regenerate per-crate `.sqlx/` directories (requires a running database). Commit the result before tagging.

**`<crate>@<ver>` tag pollution**
`cargo-ws` writes per-crate tags locally on every run. Never `git push --tags`. To prune local clutter:
```bash
git tag -l '*@*' | xargs -r git tag -d
```

**Template `release.yml` can't find bridge assets**
Check that the upstream tag in core actually has assets uploaded. `release-sign.yml` deletes the release before re-creating it on workflow_dispatch retries; if it fails partway through, the release may exist with no assets. Re-run the core workflow first.

**Gateway upload fails with 403**
`RELEASE_UPLOAD_TOKEN` (deploy → template cross-repo upload) expired. Rotate in deploy's repo secrets.

**Tag namespace (legacy `v*`)**
Both repos still accept bare `v*` for one transition cycle. Use the prefixed forms (`bridge-v*`, `gateway-v*`) for any new release. The bare-`v*` triggers will be removed once a full release cycle has shipped under the prefixed scheme.
