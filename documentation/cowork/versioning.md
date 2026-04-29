# Versioning strategy

Three repos do not version in lockstep. This document is the source of truth for what each tag means and which version of what gets published.

**Single rule**: the **tag prefix names the product**, regardless of which repo it lives in. Bare `v*` is legacy and being removed.

## Tag namespaces

| Tag pattern | Repo | Triggers | Product |
|---|---|---|---|
| `cowork-vX.Y.Z` | `systemprompt-core` | `release-sign.yml` | cowork binary, 3-OS, signed |
| `cowork-mac-vX.Y.Z` | `systemprompt-core` | dedicated workflow | cowork macOS x86_64 only (Intel) |
| `gateway-vX.Y.Z` | `systemprompt-deploy` | `release.yml` + Docker / Helm / Homebrew / Scoop fan-out | gateway server (cross-uploaded to template's Releases via `RELEASE_UPLOAD_TOKEN`) |
| `cowork-vX.Y.Z` | `systemprompt-template` | `release.yml` | cowork end-user bundle (re-attaches core's signed binaries to a branded template release) |
| `gateway-vX.Y.Z` | `systemprompt-template` | (passive — assets uploaded by deploy) | gateway end-user release surface |
| `<crate>@<ver>` | `systemprompt-core` | none | cargo-ws artifact — **never push** |

### Legacy `v*` (deprecated)

Both `systemprompt-core/release-sign.yml` and `systemprompt-deploy/release.yml` currently still listen on bare `v*` for one transition cycle. Do not push new bare `v*` tags. After the next release ships under the prefixed scheme, the `v*` triggers will be removed from both workflows.

Why it's deprecated: bare `v*` triggered cowork in core but gateway in deploy — same string, different product. Backporting or scripted release tooling could ship the wrong product by tagging the wrong repo. Prefixed tags eliminate the ambiguity.

## Workspace version (the 30 published crates)

The `systemprompt-*` crate workspace publishes to crates.io via `cargo workspaces`:

```bash
cargo ws version --all patch --no-git-push --yes
cargo ws publish --no-verify --publish-as-is --yes
```

Publish order (each layer must resolve before the next):

```
shared → infra → domain → app → entry → facade
```

The facade crate `systemprompt` re-exports everything with feature flags (`core`, `database`, `api`, `cli`, `full`). Downstream consumers depend on the facade; only the facade version matters for them.

Crate publish does not need a git tag — `cargo ws publish` pushes to crates.io directly. If you want a git marker for the publish event, use `crates-vX.Y.Z` (no workflow listens on it; it's a label only).

Known issues:

- `cargo ws version` sometimes skips crates it thinks are unchanged and leaves stale `version = "X.Y.Z"` pins in the root `[workspace.dependencies]`. This breaks `cargo update`. Grep + hand-fix before publishing.
- SQLx caches need regeneration before publishing: `just sqlx-prepare-publish`. Commit per-crate `.sqlx/` dirs.

## Independent version drift across repos

Snapshot at time of writing:

| Repo | Workspace version | `systemprompt` facade dep |
|---|---|---|
| `systemprompt-core` | 0.4.0 | n/a (owns the facade) |
| `bin/cowork/` (in core) | 0.4.0 (independent — not in workspace) | n/a |
| `systemprompt-deploy` | 0.4.0 | `= "0.4.0"` |
| `systemprompt-template` | 0.4.2 | `= "0.4.2"` + `[patch.crates-io]` overrides for dev |

Rules:
- **Cowork (`bin/cowork/Cargo.toml`)** versions independently. Its tag track is `cowork-v*` in core; its workspace membership is none. The crate version and the release tag don't have to match — the binary identifies itself by tag.
- **Gateway** versions independently of cowork. Tagged `gateway-v*` in deploy.
- **Template** is allowed to lag or lead core's facade by patch versions. Forks pin a fixed `systemprompt = "X.Y.Z"`; the `[patch.crates-io]` block exists for local dev against `../systemprompt-core/` and is stripped on fork.

## Release cadence

- Cowork can ship without a gateway release. It often does.
- Gateway can ship without a cowork release.
- A `cargo ws publish` of the workspace usually pairs with a deploy `gateway-v*` release, but doesn't have to. Crate-only releases (no binary, no Docker) are valid.
- macOS x86_64 cowork lags the main 3-OS matrix. Re-tag `cowork-mac-vX.Y.Z` only when an Intel-Mac-specific change merits it.

## Pre-release / RC

There is **no formal RC tag policy** wired up today. `cowork-vX.Y.Z-rc.N`, `gateway-vX.Y.Z-rc.N` etc. would not currently match any workflow trigger and are not published. If you need a pre-release, use `workflow_dispatch` with a non-monotonic tag and treat the resulting GitHub Release as draft.

## Consumer pinning

**Template `Cargo.toml`** pins:
```toml
systemprompt = { version = "0.4.2", default-features = false, features = ["core", "database"] }

[patch.crates-io]
systemprompt = { path = "../systemprompt-core/systemprompt" }
# … 25 more local path overrides …
```

The `[patch.crates-io]` block is dev-only. Forkers must comment it out or delete it; otherwise `cargo build` fails on a missing sibling directory.

**`bin/cowork/Cargo.toml`** is `workspace = false` and versioned independently from the facade. It does not depend on the `systemprompt` facade — it has its own `[dependencies]` block.

## Tag hygiene

| Do | Don't |
|---|---|
| `git push origin cowork-vX.Y.Z` | `git push --tags` (drags `<crate>@<ver>` cruft) |
| `git push origin gateway-vX.Y.Z` | Push a bare `vX.Y.Z` tag (legacy; will silently stop triggering after the deprecation cycle) |
| `git tag -l '*@*' \| xargs -r git tag -d` to prune local clutter | Tag in the wrong repo (cowork in deploy, gateway in core) — though prefixes now make this a no-op rather than a footgun |

When in doubt, push the single tag explicitly by name. Never push tag globs.
