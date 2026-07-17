<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://systemprompt.io/files/images/logo.svg">
  <source media="(prefers-color-scheme: light)" srcset="https://systemprompt.io/files/images/logo-dark.svg">
  <img src="https://systemprompt.io/files/images/logo.svg" alt="systemprompt.io" width="180">
</picture>

### Config-as-code templates for AI governance you own

[**Website**](https://systemprompt.io) · [**Documentation**](https://systemprompt.io/documentation/) · [**Guides**](https://systemprompt.io/guides) · [**Core**](https://github.com/systempromptio/systemprompt-core) · [**Template**](https://github.com/systempromptio/systemprompt-template) · [**Discord**](https://discord.gg/wkAbSuPWpr)

</div>

---

# systemprompt-template-provider

<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/00-overview.svg">
    <source media="(prefers-color-scheme: light)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/light/00-overview.svg">
    <img alt="systemprompt-template-provider — systemprompt-core workspace" src="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/00-overview.svg" width="100%">
  </picture>
</div>

[![Crates.io](https://img.shields.io/crates/v/systemprompt-template-provider.svg?style=flat-square)](https://crates.io/crates/systemprompt-template-provider)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-template-provider?style=flat-square)](https://docs.rs/systemprompt-template-provider)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)
[![codecov](https://img.shields.io/codecov/c/github/systempromptio/systemprompt-core/main?style=flat-square&logo=codecov)](https://codecov.io/gh/systempromptio/systemprompt-core)

Your templates are config, versioned in your own repository, not entries in someone else's dashboard. This crate defines the traits that load them. Templates compile into the binary or resolve from the filesystem at runtime, and every provider passes through one typed contract.

**Layer**: Shared. Foundational types and traits with no dependencies on other layers. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Overview

The rendering pipeline talks to templates through traits, never through a concrete registry. An embedded loader serves compile-time templates. A filesystem loader discovers them at runtime, sandboxed to a base path. Provider contracts for template, component, and page data are re-exported so downstream crates depend on one surface.

## Architecture

| Type | Description |
|------|-------------|
| `TemplateProvider` | Top-level provider trait for the rendering pipeline |
| `TemplateLoader` | Trait for loading templates by name |
| `TemplateDataExtender` | Trait for augmenting template render context |
| `ComponentRenderer` | Trait for rendering reusable components |
| `PageDataProvider` | Trait for supplying page-level data |
| `PagePrerenderer` | Trait for static page prerendering (re-exported from `systemprompt-provider-contracts`) |
| `EmbeddedLoader` | Unit struct loader for compile-time embedded templates |
| `FileSystemLoader` | Async `tokio::fs`-backed loader with base-path sandboxing (requires `tokio` feature) |
| `TemplateLoaderError` / `TemplateLoaderResult` | Error type and result alias for loader operations |
| `DynTemplateProvider` | `Arc<dyn TemplateProvider>` type alias |
| `DynTemplateLoader` | `Arc<dyn TemplateLoader>` type alias |
| `DynTemplateDataExtender` | `Arc<dyn TemplateDataExtender>` type alias |
| `DynComponentRenderer` | `Arc<dyn ComponentRenderer>` type alias |
| `DynPageDataProvider` | `Arc<dyn PageDataProvider>` type alias |
| `DynPagePrerenderer` | `Arc<dyn PagePrerenderer>` type alias |

## Usage

```toml
[dependencies]
systemprompt-template-provider = "0.21"
```

```rust
use systemprompt_template_provider::{
    TemplateLoader, TemplateLoaderResult, EmbeddedLoader,
    DynTemplateProvider, DynTemplateLoader,
};
use async_trait::async_trait;

let loader: DynTemplateLoader = std::sync::Arc::new(EmbeddedLoader);
```

```rust
use std::sync::Arc;
use systemprompt_template_provider::{DynTemplateProvider, TemplateProvider};

fn register(provider: Arc<dyn TemplateProvider>) -> DynTemplateProvider {
    // Hand the provider to the runtime as a type-erased Arc.
    provider
}
```

## Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `tokio` | No | Enables `FileSystemLoader` for async file-based template loading |

## Dependencies

### Internal

| Crate | Purpose |
|-------|---------|
| `systemprompt-provider-contracts` | Provider trait definitions |

### External

| Crate | Purpose |
|-------|---------|
| `async-trait` | Async trait support |
| `thiserror` | Derive macros for loader error types |
| `tokio` | Async filesystem operations (optional, `fs` + `sync`) |

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-template-provider)** · **[docs.rs](https://docs.rs/systemprompt-template-provider)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Shared layer · Own how your organization governs AI.</sub>

</div>
