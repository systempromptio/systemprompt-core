# systemprompt.io README Standards

Guidelines for writing crate and project READMEs. All READMEs must be consistent, informative, and suitable for crates.io publication.

---

## 1. Crate README Structure

Every crate README follows this structure in order.

### Required Sections

#### 1.1 Header Block

```markdown
# {crate-name}

{One-line description - must match Cargo.toml description exactly}

[![Crates.io](https://img.shields.io/crates/v/{crate-name}.svg)](https://crates.io/crates/{crate-name})
[![Documentation](https://docs.rs/{crate-name}/badge.svg)](https://docs.rs/{crate-name})
[![License: FSL-1.1-ALv2](https://img.shields.io/badge/License-FSL--1.1--ALv2-blue.svg)](LICENSE)
```

**Rules:**
- Crate name is the Cargo package name (e.g., `systemprompt-agent`)
- One-liner must be identical to `Cargo.toml` `description` field
- All three badges are mandatory

#### 1.2 Overview

2-4 sentences describing:
- What the crate does
- Primary use case
- Role in the systemprompt.io architecture

Include the layer indicator:

```markdown
**Part of the [Shared|Infra|Domain|App|Entry] layer in the systemprompt.io architecture.**
```

#### 1.3 Installation

```markdown
## Installation

Add to your `Cargo.toml`:

\`\`\`toml
[dependencies]
{crate-name} = "0.0.1"
\`\`\`
```

Use the current version from `Cargo.toml`.

#### 1.4 Quick Example

```markdown
## Quick Example

\`\`\`rust
use {crate_name}::prelude::*;

// 5-15 lines of working code
// Must compile and demonstrate primary use case
\`\`\`
```

**Rules:**
- Example must be self-contained and compilable
- Import via prelude when available
- Show the most common usage pattern
- Keep under 15 lines

#### 1.5 License

```markdown
## License

FSL-1.1-ALv2 - See [LICENSE](../../LICENSE) for details.
```

---

### Optional Sections

Include when applicable:

#### Feature Flags

Only include if the crate has optional features:

```markdown
## Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `web` | Yes | HTTP API routes |
| `cli` | No | CLI support |
```

#### Directory Structure

Only for crates with complex structure (>10 source files):

```markdown
## Structure

\`\`\`
src/
├── lib.rs
├── error.rs
├── models/
│   └── ...
└── services/
    └── ...
\`\`\`
```

**Rules:**
- Use ASCII tree format
- Show top 2 levels
- Truncate deeply nested items with `...`

#### Core Types / Public API

For crates with 3+ primary exports:

```markdown
## Core Types

| Type | Description |
|------|-------------|
| `TypeName` | Brief description |
```

#### Architecture Diagram

For complex crates with multi-component architecture:

```markdown
## Architecture

\`\`\`
┌─────────────┐     ┌─────────────┐
│  Component  │────►│  Component  │
└─────────────┘     └─────────────┘
\`\`\`
```

**Rules:**
- Use ASCII box drawing characters
- Keep diagram under 40 lines
- Label all arrows

#### Dependencies

```markdown
## Dependencies

### Internal

| Crate | Purpose |
|-------|---------|
| `systemprompt-models` | Shared data types |

### External

| Crate | Purpose |
|-------|---------|
| `tokio` | Async runtime |
```

---

## 2. Anti-Patterns

DO NOT include:

| Anti-Pattern | Reason |
|--------------|--------|
| Version badges pointing to specific version | Use crates.io badge (auto-updates) |
| "Work in progress" disclaimers | All published crates should be functional |
| API documentation (rustdoc content) | Belongs in doc comments, not README |
| Changelog | Use CHANGELOG.md or GitHub releases |
| Build status badges | Not applicable to library crates |
| Inline comments in examples | Code should be self-explanatory |

---

## 3. Root README Structure

The workspace root README has additional requirements.

### Required Sections

1. **Title + tagline + badges**
2. **Table of Contents** (linked)
3. **Why systemprompt.io?** (differentiation)
4. **Quick Start** (5-minute working example)
5. **Installation** (feature flags table)
6. **Architecture** (layer diagram)
7. **Available Crates** (by layer tables)
8. **Configuration** (profile setup)
9. **CLI Reference** (key commands)
10. **Troubleshooting** (common issues)
11. **Contributing** (link to CONTRIBUTING.md)
12. **License**

### Quick Start Requirements

The Quick Start section must:
- Work on a fresh clone with only `rust` and `docker` installed
- Complete in under 5 minutes
- Result in a running server
- Include database setup
- Use copy-paste commands

---

## 4. Layer Indicators

Use the correct layer for each crate:

| Layer | Path | Crates |
|-------|------|--------|
| Shared | `crates/shared/` | traits, models, identifiers, extension, client, provider-contracts, template-provider |
| Infra | `crates/infra/` | database, events, security, config, logging, cloud, loader |
| Domain | `crates/domain/` | users, oauth, files, analytics, content, ai, mcp, agent, templates |
| App | `crates/app/` | runtime, scheduler, generator, sync |
| Entry | `crates/entry/` | api, cli |

---

## 5. Validation Checklist

Before submitting README changes:

- [ ] Title matches `Cargo.toml` package name
- [ ] Description matches `Cargo.toml` description
- [ ] All three badges are present and correct
- [ ] Quick example compiles
- [ ] Layer indicator is correct
- [ ] License section references correct LICENSE file
- [ ] No inline TODO/FIXME/WIP markers
- [ ] No dead links
- [ ] Markdown renders correctly (preview first)

---

## 6. Examples

### Minimal Crate README

For simple crates like `systemprompt-identifiers`:

```markdown
# systemprompt-identifiers

Core identifier types for systemprompt.io.

[![Crates.io](https://img.shields.io/crates/v/systemprompt-identifiers.svg)](https://crates.io/crates/systemprompt-identifiers)
[![Documentation](https://docs.rs/systemprompt-identifiers/badge.svg)](https://docs.rs/systemprompt-identifiers)
[![License: FSL-1.1-ALv2](https://img.shields.io/badge/License-FSL--1.1--ALv2-blue.svg)](LICENSE)

## Overview

Type-safe identifier wrappers for all systemprompt.io entities. Provides compile-time safety and consistent ID generation across the platform.

**Part of the Shared layer in the systemprompt.io architecture.**

## Installation

Add to your `Cargo.toml`:

\`\`\`toml
[dependencies]
systemprompt-identifiers = "0.0.1"
\`\`\`

## Quick Example

\`\`\`rust
use systemprompt_identifiers::{UserId, TaskId};

let user_id = UserId::new();
let task_id = TaskId::new();

println!("User: {}, Task: {}", user_id, task_id);
\`\`\`

## License

FSL-1.1-ALv2 - See [LICENSE](../../LICENSE) for details.
```

### Standard Crate README

For typical crates, include:
- Header with badges
- Overview with layer indicator
- Installation
- Quick Example
- Core Types table (if 3+ exports)
- Dependencies section
- License

### Complex Crate README

For large crates like `systemprompt-agent`, include all optional sections:
- Architecture diagram
- Directory structure
- Core Types table
- Feature Flags table
- Dependencies (internal + external)

---

## 7. Badge URLs

Standard badge format:

```markdown
[![Crates.io](https://img.shields.io/crates/v/{crate-name}.svg)](https://crates.io/crates/{crate-name})
[![Documentation](https://docs.rs/{crate-name}/badge.svg)](https://docs.rs/{crate-name})
[![License: FSL-1.1-ALv2](https://img.shields.io/badge/License-FSL--1.1--ALv2-blue.svg)](LICENSE)
```

Replace `{crate-name}` with the actual package name from `Cargo.toml`.
