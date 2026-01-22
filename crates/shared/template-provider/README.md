# systemprompt-template-provider

Template provider traits and abstractions for systemprompt.io.

[![Crates.io](https://img.shields.io/crates/v/systemprompt-template-provider.svg)](https://crates.io/crates/systemprompt-template-provider)
[![Documentation](https://docs.rs/systemprompt-template-provider/badge.svg)](https://docs.rs/systemprompt-template-provider)
[![License: FSL-1.1-ALv2](https://img.shields.io/badge/License-FSL--1.1--ALv2-blue.svg)](LICENSE)

## Overview

Provides template loading abstractions and dynamic type aliases for template-related providers. Includes an embedded loader for compile-time templates and a filesystem loader for runtime template discovery. Re-exports provider contracts for template, component, and page data handling.

**Part of the Shared layer in the systemprompt.io architecture.**

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
systemprompt-template-provider = "0.0.1"
```

## Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `tokio` | No | Enables `FileSystemLoader` for async file-based template loading |

## Quick Example

```rust
use systemprompt_template_provider::{
    TemplateLoader, TemplateLoaderResult, EmbeddedLoader,
    DynTemplateProvider, DynTemplateLoader,
};
use async_trait::async_trait;

let loader: DynTemplateLoader = std::sync::Arc::new(EmbeddedLoader::new());
```

## Core Types

| Type | Description |
|------|-------------|
| `TemplateLoader` | Trait for loading templates by name |
| `EmbeddedLoader` | Loader for compile-time embedded templates |
| `FileSystemLoader` | Async filesystem template loader (requires `tokio` feature) |
| `DynTemplateProvider` | `Arc<dyn TemplateProvider>` type alias |
| `DynTemplateLoader` | `Arc<dyn TemplateLoader>` type alias |
| `DynComponentRenderer` | `Arc<dyn ComponentRenderer>` type alias |

## Dependencies

### Internal

| Crate | Purpose |
|-------|---------|
| `systemprompt-provider-contracts` | Provider trait definitions |

### External

| Crate | Purpose |
|-------|---------|
| `async-trait` | Async trait support |
| `tokio` | Async filesystem operations (optional) |

## License

FSL-1.1-ALv2 - See [LICENSE](../../LICENSE) for details.
