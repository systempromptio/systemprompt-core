<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://systemprompt.io/files/images/logo.svg">
  <source media="(prefers-color-scheme: light)" srcset="https://systemprompt.io/files/images/logo-dark.svg">
  <img src="https://systemprompt.io/files/images/logo.svg" alt="systemprompt.io" width="180">
</picture>

### Production infrastructure for AI agents

[**Website**](https://systemprompt.io) · [**Documentation**](https://systemprompt.io/documentation/) · [**Guides**](https://systemprompt.io/guides) · [**Core**](https://github.com/systempromptio/systemprompt-core) · [**Template**](https://github.com/systempromptio/systemprompt-template) · [**Discord**](https://discord.gg/wkAbSuPWpr)

</div>

---

# systemprompt-extension

<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/00-overview.svg">
    <source media="(prefers-color-scheme: light)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/light/00-overview.svg">
    <img alt="systemprompt-extension — systemprompt-core workspace" src="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/00-overview.svg" width="100%">
  </picture>
</div>

[![Crates.io](https://img.shields.io/crates/v/systemprompt-extension.svg?style=flat-square)](https://crates.io/crates/systemprompt-extension)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-extension?style=flat-square)](https://docs.rs/systemprompt-extension)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

Compile-time extension framework for systemprompt.io AI governance infrastructure. Built on the `inventory` crate — registers schemas, API routes, jobs, and providers in the MCP governance pipeline. Extensions can add new routes, services, and capabilities to the platform.

**Layer**: Shared — foundational types/traits with no dependencies on other layers. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Overview

Provides the infrastructure for building and loading systemprompt.io extensions. Extensions can add new routes, services, and capabilities to the platform.

## Architecture

- `ExtensionContext` — Runtime context for extensions
- `ExtensionError` — Error types for extension operations
- `ExtensionLoader` — Registration and loading system

## Usage

```toml
[dependencies]
systemprompt-extension = "0.2.1"
```

```rust
use systemprompt_extension::prelude::*;

struct MyExtension;

impl Extension for MyExtension {
    fn id(&self) -> &str { "my-extension" }
    fn name(&self) -> &str { "My Extension" }
    fn version(&self) -> &str { "1.0.0" }
}

register_extension!(MyExtension);
```

```rust
use systemprompt_extension::{Extension, ExtensionMetadata, ExtensionRole};

pub struct MyExtension;

impl Extension for MyExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            name: "my-extension".into(),
            version: "0.1.0".into(),
            role: ExtensionRole::Domain,
            ..Default::default()
        }
    }
}
```

## Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `web` | Yes | HTTP API routes via Axum |
| `plugin-discovery` | No | Dynamic plugin loading |

## Dependencies

- `async-trait` — Async trait support
- `axum` — Router types (optional, with `web` feature)
- `inventory` — Compile-time extension registration
- `reqwest` — HTTP client (optional, with `web` feature)

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-extension)** · **[docs.rs](https://docs.rs/systemprompt-extension)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Shared layer · Own how your organization uses AI.</sub>

</div>
