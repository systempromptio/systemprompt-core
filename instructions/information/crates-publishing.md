# Crates.io Publishing Guide

Complete guide for publishing SystemPrompt crates to crates.io.

---

## Overview

The SystemPrompt workspace contains **27 internal crates** organized in 5 layers. Publishing requires strict topological ordering due to inter-crate dependencies.

### Layer Hierarchy

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

---

## Prerequisites

### 1. crates.io Account Setup

```bash
# Login to crates.io
cargo login <your-api-token>

# Verify login
cargo owner --list
```

### 2. External Dependencies Verification

All external dependencies must be available on crates.io:

| Dependency | crates.io | Notes |
|------------|-----------|-------|
| `rmcp` | :white_check_mark: | MCP protocol implementation |
| `sqlx` | :white_check_mark: | Database |
| `tokio` | :white_check_mark: | Async runtime |
| `axum` | :white_check_mark: | HTTP framework |
| All others | :white_check_mark: | Standard ecosystem crates |

### 3. Workspace Cargo.toml Preparation

Before publishing, update the root `Cargo.toml` to include:

```toml
[workspace.package]
version = "0.1.0"
authors = ["SystemPrompt <team@systemprompt.io>"]
license = "MIT OR Apache-2.0"
repository = "https://github.com/systemprompt/systemprompt-core"
homepage = "https://systemprompt.io"
keywords = ["ai", "mcp", "agent", "llm"]
categories = ["development-tools", "web-programming"]
```

---

## Publishing Order

### Phase 1: Shared Layer (No Internal Dependencies First)

**Step 1.1: systemprompt-identifiers** (no internal deps)
```bash
cd crates/shared/identifiers
cargo publish --dry-run
cargo publish
```

**Step 1.2: systemprompt-provider-contracts** (depends on: identifiers)
```toml
# Update Cargo.toml
systemprompt-identifiers = "0.1.0"  # was path = "../identifiers"
```
```bash
cd crates/shared/provider-contracts
cargo publish
```

**Step 1.3: systemprompt-traits** (depends on: identifiers, provider-contracts)

```toml
# Update Cargo.toml
systemprompt-identifiers = "0.1.0"
systemprompt-provider-contracts = "0.1.0"
```
```bash
cd crates/shared/traits
cargo publish
```

**Step 1.4: systemprompt-extension** (depends on: provider-contracts, traits)
```toml
systemprompt-provider-contracts = "0.1.0"
systemprompt-traits = "0.1.0"
```

**Step 1.5: systemprompt-models** (depends on: traits, identifiers, extension)
```toml
systemprompt-traits = "0.1.0"
systemprompt-identifiers = "0.1.0"
systemprompt-extension = "0.1.0"
```

**Step 1.6: systemprompt-client** (depends on: models, identifiers)
```toml
systemprompt-models = "0.1.0"
systemprompt-identifiers = "0.1.0"
```

**Step 1.7: systemprompt-template-provider** (depends on: provider-contracts)
```toml
systemprompt-provider-contracts = "0.1.0"
```

---

### Phase 2: Infrastructure Layer

**Step 2.1: systemprompt-database** (depends on: traits, identifiers, models, extension)
```toml
systemprompt-traits = "0.1.0"
systemprompt-identifiers = "0.1.0"
systemprompt-models = "0.1.0"
systemprompt-extension = "0.1.0"
```

**Step 2.2: systemprompt-logging** (depends on: database, traits, identifiers)
```toml
systemprompt-database = "0.1.0"
systemprompt-traits = "0.1.0"
systemprompt-identifiers = "0.1.0"
```

**Step 2.3: systemprompt-events** (depends on: models, identifiers)
```toml
systemprompt-models = "0.1.0"
systemprompt-identifiers = "0.1.0"
```

**Step 2.4: systemprompt-security** (depends on: models, identifiers)
```toml
systemprompt-models = "0.1.0"
systemprompt-identifiers = "0.1.0"
```

**Step 2.5: systemprompt-loader** (depends on: models)
```toml
systemprompt-models = "0.1.0"
```

**Step 2.6: systemprompt-config** (depends on: logging, models)
```toml
systemprompt-logging = "0.1.0"
systemprompt-models = "0.1.0"
```

**Step 2.7: systemprompt-cloud** (depends on: identifiers, models, client, logging)
```toml
systemprompt-identifiers = "0.1.0"
systemprompt-models = "0.1.0"
systemprompt-client = "0.1.0"
systemprompt-logging = "0.1.0"
```

---

### Phase 3: Domain Layer

**Step 3.1: systemprompt-analytics** (depends on: models, identifiers, traits, database)
```toml
systemprompt-models = "0.1.0"
systemprompt-identifiers = "0.1.0"
systemprompt-traits = "0.1.0"
systemprompt-database = "0.1.0"
```

**Step 3.2: systemprompt-users** (depends on: database, identifiers, models, provider-contracts, traits)
```toml
systemprompt-database = "0.1.0"
systemprompt-identifiers = "0.1.0"
systemprompt-models = "0.1.0"
systemprompt-provider-contracts = "0.1.0"
systemprompt-traits = "0.1.0"
```

**Step 3.3: systemprompt-files** (depends on: cloud, database, logging, models, identifiers, traits, provider-contracts)
```toml
systemprompt-cloud = "0.1.0"
systemprompt-database = "0.1.0"
systemprompt-logging = "0.1.0"
systemprompt-models = "0.1.0"
systemprompt-identifiers = "0.1.0"
systemprompt-traits = "0.1.0"
systemprompt-provider-contracts = "0.1.0"
```

**Step 3.4: systemprompt-templates** (depends on: template-provider)
```toml
systemprompt-template-provider = "0.1.0"
```

**Step 3.5: systemprompt-content** (depends on: database, logging, models, identifiers, traits, provider-contracts, config)
```toml
systemprompt-database = "0.1.0"
systemprompt-logging = "0.1.0"
systemprompt-models = "0.1.0"
systemprompt-identifiers = "0.1.0"
systemprompt-traits = "0.1.0"
systemprompt-provider-contracts = "0.1.0"
systemprompt-config = "0.1.0"
```

**Step 3.6: systemprompt-ai** (depends on: models, database, loader, logging, files, analytics, traits, identifiers)
```toml
systemprompt-models = "0.1.0"
systemprompt-database = "0.1.0"
systemprompt-loader = "0.1.0"
systemprompt-logging = "0.1.0"
systemprompt-files = "0.1.0"
systemprompt-analytics = "0.1.0"
systemprompt-traits = "0.1.0"
systemprompt-identifiers = "0.1.0"
```

---

### Phase 4: Application Layer

**Step 4.1: systemprompt-runtime** (depends on: models, traits, extension, identifiers, database, logging, config, loader, analytics, files)
```toml
systemprompt-models = "0.1.0"
systemprompt-traits = "0.1.0"
systemprompt-extension = "0.1.0"
systemprompt-identifiers = "0.1.0"
systemprompt-database = "0.1.0"
systemprompt-logging = "0.1.0"
systemprompt-config = "0.1.0"
systemprompt-loader = "0.1.0"
systemprompt-analytics = "0.1.0"
systemprompt-files = "0.1.0"
```

**Step 4.2: systemprompt-scheduler** (depends on: runtime, database, logging, analytics, users, traits, provider-contracts, identifiers, models)
```toml
systemprompt-runtime = "0.1.0"
systemprompt-database = "0.1.0"
systemprompt-logging = "0.1.0"
systemprompt-analytics = "0.1.0"
systemprompt-users = "0.1.0"
systemprompt-traits = "0.1.0"
systemprompt-provider-contracts = "0.1.0"
systemprompt-identifiers = "0.1.0"
systemprompt-models = "0.1.0"
```

**Step 4.3: systemprompt-oauth** (depends on: models, runtime, users, logging, database, analytics, security, traits, identifiers)
```toml
systemprompt-models = "0.1.0"
systemprompt-runtime = "0.1.0"
systemprompt-users = "0.1.0"
systemprompt-logging = "0.1.0"
systemprompt-database = "0.1.0"
systemprompt-analytics = "0.1.0"
systemprompt-security = "0.1.0"
systemprompt-traits = "0.1.0"
systemprompt-identifiers = "0.1.0"
```

**Step 4.4: systemprompt-mcp** (depends on: models, identifiers, runtime, oauth, logging, config, database, scheduler, traits, loader)
```toml
systemprompt-models = "0.1.0"
systemprompt-identifiers = "0.1.0"
systemprompt-runtime = "0.1.0"
systemprompt-oauth = "0.1.0"
systemprompt-logging = "0.1.0"
systemprompt-config = "0.1.0"
systemprompt-database = "0.1.0"
systemprompt-scheduler = "0.1.0"
systemprompt-traits = "0.1.0"
systemprompt-loader = "0.1.0"
```

**Step 4.5: systemprompt-agent** (depends on: many - publish last in domain)
```toml
systemprompt-models = "0.1.0"
systemprompt-traits = "0.1.0"
systemprompt-identifiers = "0.1.0"
systemprompt-runtime = "0.1.0"
systemprompt-loader = "0.1.0"
systemprompt-events = "0.1.0"
systemprompt-oauth = "0.1.0"
systemprompt-users = "0.1.0"
systemprompt-logging = "0.1.0"
systemprompt-config = "0.1.0"
systemprompt-database = "0.1.0"
systemprompt-analytics = "0.1.0"
systemprompt-security = "0.1.0"
systemprompt-scheduler = "0.1.0"
systemprompt-mcp = "0.1.0"
systemprompt-ai = "0.1.0"
systemprompt-files = "0.1.0"
```

**Step 4.6: systemprompt-generator** (depends on: models, traits, provider-contracts, identifiers, database, logging, config, cloud, content, files, templates, template-provider, extension)
```toml
# Update all path dependencies to version dependencies
```

**Step 4.7: systemprompt-sync** (depends on: agent, content, database, logging, identifiers)
```toml
systemprompt-agent = "0.1.0"
systemprompt-content = "0.1.0"
systemprompt-database = "0.1.0"
systemprompt-logging = "0.1.0"
systemprompt-identifiers = "0.1.0"
```

---

### Phase 5: Entry Layer

**Step 5.1: systemprompt-api** (depends on: many domain and app crates)
```toml
# Update all path dependencies to version dependencies
```

**Step 5.2: systemprompt-cli** (depends on: nearly everything)
```toml
# Update all path dependencies to version dependencies
```

---

## Complete Publishing Order (27 crates)

| Order | Crate | Layer | Dependencies |
|-------|-------|-------|--------------|
| 1 | `systemprompt-identifiers` | Shared | None |
| 2 | `systemprompt-provider-contracts` | Shared | identifiers |
| 3 | `systemprompt-traits` | Shared | identifiers, provider-contracts |
| 4 | `systemprompt-extension` | Shared | provider-contracts, traits |
| 5 | `systemprompt-models` | Shared | traits, identifiers, extension |
| 6 | `systemprompt-client` | Shared | models, identifiers |
| 7 | `systemprompt-template-provider` | Shared | provider-contracts |
| 8 | `systemprompt-database` | Infra | traits, identifiers, models, extension |
| 9 | `systemprompt-logging` | Infra | database, traits, identifiers |
| 10 | `systemprompt-events` | Infra | models, identifiers |
| 11 | `systemprompt-security` | Infra | models, identifiers |
| 12 | `systemprompt-loader` | Infra | models |
| 13 | `systemprompt-config` | Infra | logging, models |
| 14 | `systemprompt-cloud` | Infra | identifiers, models, client, logging |
| 15 | `systemprompt-analytics` | Domain | models, identifiers, traits, database |
| 16 | `systemprompt-users` | Domain | database, identifiers, models, provider-contracts, traits |
| 17 | `systemprompt-files` | Domain | cloud, database, logging, models, identifiers, traits, provider-contracts |
| 18 | `systemprompt-templates` | Domain | template-provider |
| 19 | `systemprompt-content` | Domain | database, logging, models, identifiers, traits, provider-contracts, config |
| 20 | `systemprompt-ai` | Domain | models, database, loader, logging, files, analytics, traits, identifiers |
| 21 | `systemprompt-runtime` | App | models, traits, extension, identifiers, database, logging, config, loader, analytics, files |
| 22 | `systemprompt-scheduler` | App | runtime, database, logging, analytics, users, traits, provider-contracts, identifiers, models |
| 23 | `systemprompt-oauth` | Domain | models, runtime, users, logging, database, analytics, security, traits, identifiers |
| 24 | `systemprompt-mcp` | Domain | models, identifiers, runtime, oauth, logging, config, database, scheduler, traits, loader |
| 25 | `systemprompt-agent` | Domain | (many) |
| 26 | `systemprompt-generator` | App | (many) |
| 27 | `systemprompt-sync` | App | agent, content, database, logging, identifiers |
| 28 | `systemprompt-api` | Entry | (many) |
| 29 | `systemprompt-cli` | Entry | (nearly all) |

---

## Known Issues

### 1. ~~Circular Dependency: traits ↔ database~~ (RESOLVED)

**Status**: Fixed. The `systemprompt-database` dev-dependency in `systemprompt-traits` was orphaned and has been removed.

**Original Problem**: `systemprompt-traits` had a dev-dependency on `systemprompt-database`, but `systemprompt-database` depends on `systemprompt-traits`.

**Resolution Applied**: Removed the unused `systemprompt-database` dev-dependency from `crates/shared/traits/Cargo.toml`. Analysis confirmed no source code or tests actually used this dependency.

**Current State**:
```
systemprompt-database (Infra) ──[depends on]──> systemprompt-traits (Shared) ✓
systemprompt-traits (Shared) ──[no infra deps]──> (shared crates only)       ✓
```

### 2. Cross-Layer Dependencies

Several domain crates depend on app layer (`systemprompt-runtime`):
- `systemprompt-oauth`
- `systemprompt-mcp`
- `systemprompt-agent`

**Resolution**: These were architectural violations. The `systemprompt-ai` crate was fixed to remove runtime dependency. Similar fixes needed for others.

---

## Automation Script

```bash
#!/bin/bash
# publish-all.sh - Publish all crates in order

set -e

CRATES=(
    "crates/shared/identifiers"
    "crates/shared/provider-contracts"
    "crates/shared/traits"
    "crates/shared/extension"
    "crates/shared/models"
    "crates/shared/client"
    "crates/shared/template-provider"
    "crates/infra/database"
    "crates/infra/logging"
    "crates/infra/events"
    "crates/infra/security"
    "crates/infra/loader"
    "crates/infra/config"
    "crates/infra/cloud"
    "crates/domain/analytics"
    "crates/domain/users"
    "crates/domain/files"
    "crates/domain/templates"
    "crates/domain/content"
    "crates/domain/ai"
    "crates/app/runtime"
    "crates/app/scheduler"
    "crates/domain/oauth"
    "crates/domain/mcp"
    "crates/domain/agent"
    "crates/app/generator"
    "crates/app/sync"
    "crates/entry/api"
    "crates/entry/cli"
)

for crate in "${CRATES[@]}"; do
    echo "Publishing $crate..."
    cd "$crate"
    cargo publish
    cd -
    sleep 30  # Wait for crates.io to index
done

echo "All crates published!"
```

---

## Pre-Publication Checklist

For each crate:

- [ ] Update `Cargo.toml` path deps to version deps
- [ ] Ensure `version`, `license`, `repository` are set
- [ ] Run `cargo publish --dry-run`
- [ ] Verify no `path = ` dependencies remain
- [ ] Check README.md exists and is accurate
- [ ] Verify all tests pass: `cargo test`
- [ ] Verify clippy passes: `cargo clippy -- -D warnings`
- [ ] Verify formatting: `cargo fmt -- --check`

---

## Version Strategy

### Initial Release: 0.1.0

All crates start at `0.1.0` for initial crates.io publication.

### Subsequent Releases

Use workspace-level versioning:
```toml
# Root Cargo.toml
[workspace.package]
version = "0.2.0"
```

When updating:
1. Update workspace version
2. Update all inter-crate dependencies to new version
3. Publish in topological order
4. Tag release: `git tag v0.2.0`

---

## Selective Publishing

To publish only specific crates (e.g., just the AI module):

```bash
# Minimum set for systemprompt-ai
REQUIRED=(
    "systemprompt-identifiers"
    "systemprompt-provider-contracts"
    "systemprompt-traits"
    "systemprompt-extension"
    "systemprompt-models"
    "systemprompt-database"
    "systemprompt-logging"
    "systemprompt-loader"
    "systemprompt-cloud"
    "systemprompt-client"
    "systemprompt-analytics"
    "systemprompt-files"
    "systemprompt-ai"
)
```

This requires **13 crates** to publish `systemprompt-ai` independently.
