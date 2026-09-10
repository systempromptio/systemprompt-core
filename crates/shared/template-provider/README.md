# systemprompt-template-provider

[![Crates.io](https://img.shields.io/crates/v/systemprompt-template-provider.svg?style=flat-square)](https://crates.io/crates/systemprompt-template-provider)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-template-provider?style=flat-square)](https://docs.rs/systemprompt-template-provider)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)
[![codecov](https://img.shields.io/codecov/c/github/systempromptio/systemprompt-core/main?style=flat-square&logo=codecov)](https://codecov.io/gh/systempromptio/systemprompt-core)

Defines template-provider contracts for compiled and filesystem-backed template sources.

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
systemprompt-template-provider = "0.50"
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
