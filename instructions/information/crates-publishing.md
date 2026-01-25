# Crates.io Publishing Guide

Complete guide for publishing and maintaining systemprompt.io crates on crates.io.

---

## Published Status

**Current Version:** `0.0.7` (Rotate Credentials Dual URLs - January 23, 2026)

All **30 crates** have been published to crates.io.

### Crate Links

#### Shared Layer
| Crate | crates.io | docs.rs |
|-------|-----------|---------|
| `systemprompt-identifiers` | [crates.io](https://crates.io/crates/systemprompt-identifiers) | [docs.rs](https://docs.rs/systemprompt-identifiers) |
| `systemprompt-provider-contracts` | [crates.io](https://crates.io/crates/systemprompt-provider-contracts) | [docs.rs](https://docs.rs/systemprompt-provider-contracts) |
| `systemprompt-traits` | [crates.io](https://crates.io/crates/systemprompt-traits) | [docs.rs](https://docs.rs/systemprompt-traits) |
| `systemprompt-extension` | [crates.io](https://crates.io/crates/systemprompt-extension) | [docs.rs](https://docs.rs/systemprompt-extension) |
| `systemprompt-models` | [crates.io](https://crates.io/crates/systemprompt-models) | [docs.rs](https://docs.rs/systemprompt-models) |
| `systemprompt-client` | [crates.io](https://crates.io/crates/systemprompt-client) | [docs.rs](https://docs.rs/systemprompt-client) |
| `systemprompt-template-provider` | [crates.io](https://crates.io/crates/systemprompt-template-provider) | [docs.rs](https://docs.rs/systemprompt-template-provider) |

#### Infrastructure Layer
| Crate | crates.io | docs.rs |
|-------|-----------|---------|
| `systemprompt-database` | [crates.io](https://crates.io/crates/systemprompt-database) | [docs.rs](https://docs.rs/systemprompt-database) |
| `systemprompt-logging` | [crates.io](https://crates.io/crates/systemprompt-logging) | [docs.rs](https://docs.rs/systemprompt-logging) |
| `systemprompt-events` | [crates.io](https://crates.io/crates/systemprompt-events) | [docs.rs](https://docs.rs/systemprompt-events) |
| `systemprompt-security` | [crates.io](https://crates.io/crates/systemprompt-security) | [docs.rs](https://docs.rs/systemprompt-security) |
| `systemprompt-loader` | [crates.io](https://crates.io/crates/systemprompt-loader) | [docs.rs](https://docs.rs/systemprompt-loader) |
| `systemprompt-config` | [crates.io](https://crates.io/crates/systemprompt-config) | [docs.rs](https://docs.rs/systemprompt-config) |
| `systemprompt-cloud` | [crates.io](https://crates.io/crates/systemprompt-cloud) | [docs.rs](https://docs.rs/systemprompt-cloud) |

#### Domain Layer
| Crate | crates.io | docs.rs |
|-------|-----------|---------|
| `systemprompt-analytics` | [crates.io](https://crates.io/crates/systemprompt-analytics) | [docs.rs](https://docs.rs/systemprompt-analytics) |
| `systemprompt-users` | [crates.io](https://crates.io/crates/systemprompt-users) | [docs.rs](https://docs.rs/systemprompt-users) |
| `systemprompt-files` | [crates.io](https://crates.io/crates/systemprompt-files) | [docs.rs](https://docs.rs/systemprompt-files) |
| `systemprompt-templates` | [crates.io](https://crates.io/crates/systemprompt-templates) | [docs.rs](https://docs.rs/systemprompt-templates) |
| `systemprompt-content` | [crates.io](https://crates.io/crates/systemprompt-content) | [docs.rs](https://docs.rs/systemprompt-content) |
| `systemprompt-ai` | [crates.io](https://crates.io/crates/systemprompt-ai) | [docs.rs](https://docs.rs/systemprompt-ai) |
| `systemprompt-oauth` | [crates.io](https://crates.io/crates/systemprompt-oauth) | [docs.rs](https://docs.rs/systemprompt-oauth) |
| `systemprompt-mcp` | [crates.io](https://crates.io/crates/systemprompt-mcp) | [docs.rs](https://docs.rs/systemprompt-mcp) |
| `systemprompt-agent` | [crates.io](https://crates.io/crates/systemprompt-agent) | [docs.rs](https://docs.rs/systemprompt-agent) |

#### Application Layer
| Crate | crates.io | docs.rs |
|-------|-----------|---------|
| `systemprompt-runtime` | [crates.io](https://crates.io/crates/systemprompt-runtime) | [docs.rs](https://docs.rs/systemprompt-runtime) |
| `systemprompt-scheduler` | [crates.io](https://crates.io/crates/systemprompt-scheduler) | [docs.rs](https://docs.rs/systemprompt-scheduler) |
| `systemprompt-generator` | [crates.io](https://crates.io/crates/systemprompt-generator) | [docs.rs](https://docs.rs/systemprompt-generator) |
| `systemprompt-sync` | [crates.io](https://crates.io/crates/systemprompt-sync) | [docs.rs](https://docs.rs/systemprompt-sync) |

#### Entry Layer
| Crate | crates.io | docs.rs |
|-------|-----------|---------|
| `systemprompt-api` | [crates.io](https://crates.io/crates/systemprompt-api) | [docs.rs](https://docs.rs/systemprompt-api) |
| `systemprompt-cli` | [crates.io](https://crates.io/crates/systemprompt-cli) | [docs.rs](https://docs.rs/systemprompt-cli) |

#### Facade
| Crate | crates.io | docs.rs |
|-------|-----------|---------|
| `systemprompt` | [crates.io](https://crates.io/crates/systemprompt) | [docs.rs](https://docs.rs/systemprompt) |

---

## Pre-Publish Checklist (MANDATORY)

Before publishing any crate, ALL checks must pass:

### 1. Verify Package Compiles as Standalone

```bash
# Test that each crate compiles when packaged (simulates crates.io download)
cargo package -p <crate-name> --allow-dirty

# For all crates:
for crate in systemprompt-{identifiers,provider-contracts,traits,extension,models,client,template-provider,database,logging,events,security,loader,config,cloud,analytics,users,files,templates,content,ai,runtime,scheduler,oauth,mcp,agent,generator,sync,api,cli} systemprompt; do
  echo "Verifying $crate..."
  cargo package -p "$crate" --allow-dirty 2>&1 | tail -1
done
```

This catches issues like:
- `include_str!` paths pointing outside the crate
- Missing files not included in package
- Dependencies that only work with path references

### 2. Run Full Test Suite

```bash
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check
```

### 3. Update CHANGELOG.md

**REQUIRED**: Every crate must have a `CHANGELOG.md` in its root directory. Update it before every publish.

Location: `crates/<layer>/<crate>/CHANGELOG.md`

Format:
```markdown
# Changelog

## [0.0.2] - 2026-01-22

### Added
- New feature X

### Changed
- Modified behavior Y

### Fixed
- Bug fix Z

## [0.0.1] - 2026-01-21

- Initial release
```

### 4. Verify Dry Run

```bash
cargo publish -p <crate-name> --dry-run --allow-dirty
```

---

## Publishing with cargo-workspaces (Recommended)

We use `cargo-workspaces` for lock-step versioning and publishing all crates together.

### Install

```bash
cargo install cargo-workspaces
```

### Publish Workflow

```bash
# 1. Make code changes and update CHANGELOG.md in affected crates

# 2. Commit code changes first
git add -A
git commit -m "feat: description of changes"

# 3. Bump version and publish all crates
cargo ws version patch --no-git-push --yes  # or minor/major
cargo ws publish --no-verify --publish-as-is --yes

# 4. Push version bump commit and tags
git push && git push --tags
```

### Version Types

- `patch`: 0.0.7 → 0.0.8 (bug fixes, small changes)
- `minor`: 0.0.7 → 0.1.0 (new features, backward compatible)
- `major`: 0.0.7 → 1.0.0 (breaking changes)

### What cargo-ws Does

1. Bumps version in root `Cargo.toml`
2. Updates all inter-crate dependency versions automatically
3. Creates a git commit with version bump
4. Creates git tags for each crate
5. Publishes all crates in correct dependency order
6. Handles crates.io rate limits with retries

---

## Manual Publishing (Fallback)

If cargo-workspaces fails, use manual approach:

### Prerequisites

```bash
cargo login <your-api-token>
cargo owner --list systemprompt
```

### Publish Single Crate

```bash
cargo publish -p systemprompt-cli --no-verify --allow-dirty
```

### Rate Limits

crates.io has rate limits for new crate publishers:
- ~1 new crate per 10 minutes for new accounts
- Higher limits for established accounts

If you hit rate limits:
```
error: 429 Too Many Requests
Please try again after <timestamp>
```

Wait until the specified time and retry.

---

## Yanking Versions

To remove a broken version (does not delete, just hides from new installs):

```bash
# Yank a version
cargo yank --version 0.0.1 systemprompt-loader

# Un-yank if needed
cargo yank --version 0.0.1 systemprompt-loader --undo
```

---

## Verifying Published Crates

```bash
# Search crates.io
cargo search systemprompt

# Check specific crate info
cargo info systemprompt-agent

# Test installation in new project
mkdir /tmp/test-sp && cd /tmp/test-sp
cargo init
echo 'systemprompt = "0.0.1"' >> Cargo.toml
cargo build
```

---

## Layer Hierarchy

```
Entry (api, cli)
    ↓
App (runtime, scheduler, generator, sync)
    ↓
Domain (agent, ai, analytics, content, files, mcp, oauth, templates, users)
    ↓
Infra (cloud, config, database, events, loader, logging, security)
    ↓
Shared (identifiers, provider-contracts, traits, extension, models, client, template-provider)
```

Dependencies flow downward only. No circular dependencies.

---

## Complete Publishing Order

| # | Crate | Layer |
|---|-------|-------|
| 1 | `systemprompt-identifiers` | Shared |
| 2 | `systemprompt-provider-contracts` | Shared |
| 3 | `systemprompt-traits` | Shared |
| 4 | `systemprompt-extension` | Shared |
| 5 | `systemprompt-models` | Shared |
| 6 | `systemprompt-client` | Shared |
| 7 | `systemprompt-template-provider` | Shared |
| 8 | `systemprompt-database` | Infra |
| 9 | `systemprompt-logging` | Infra |
| 10 | `systemprompt-events` | Infra |
| 11 | `systemprompt-security` | Infra |
| 12 | `systemprompt-loader` | Infra |
| 13 | `systemprompt-config` | Infra |
| 14 | `systemprompt-cloud` | Infra |
| 15 | `systemprompt-analytics` | Domain |
| 16 | `systemprompt-users` | Domain |
| 17 | `systemprompt-files` | Domain |
| 18 | `systemprompt-templates` | Domain |
| 19 | `systemprompt-content` | Domain |
| 20 | `systemprompt-ai` | Domain |
| 21 | `systemprompt-runtime` | App |
| 22 | `systemprompt-scheduler` | App |
| 23 | `systemprompt-oauth` | Domain |
| 24 | `systemprompt-mcp` | Domain |
| 25 | `systemprompt-agent` | Domain |
| 26 | `systemprompt-generator` | App |
| 27 | `systemprompt-sync` | App |
| 28 | `systemprompt-api` | Entry |
| 29 | `systemprompt-cli` | Entry |
| 30 | `systemprompt` | Facade |

---

## Usage Examples

### Basic Usage

```toml
[dependencies]
systemprompt = "0.0.1"
```

### With Features

```toml
[dependencies]
systemprompt = { version = "0.0.1", features = ["full"] }
```

### Specific Crates Only

```toml
[dependencies]
systemprompt-models = "0.0.1"
systemprompt-extension = "0.0.1"
systemprompt-identifiers = "0.0.1"
```

