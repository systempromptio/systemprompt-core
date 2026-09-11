# systemprompt-templates

[![Crates.io](https://img.shields.io/crates/v/systemprompt-templates.svg?style=flat-square)](https://crates.io/crates/systemprompt-templates)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-templates?style=flat-square)](https://docs.rs/systemprompt-templates)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)
[![codecov](https://img.shields.io/codecov/c/github/systempromptio/systemprompt-core/main?style=flat-square&logo=codecov)](https://codecov.io/gh/systempromptio/systemprompt-core)

Resolves Handlebars templates from registered providers and filesystem sources using configured priorities.

**Layer**: Domain — business-logic modules that implement systemprompt.io features. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Overview

**Integrations** · [Extensible Architecture](https://systemprompt.io/features/extensible-architecture)

This crate provides the core template system for discovering, loading, and rendering HTML templates using Handlebars. It supports a plugin architecture with providers, loaders, extenders, and component renderers.

## Usage

```toml
[dependencies]
systemprompt-templates = "0.51"
```

```rust
use std::sync::Arc;
use systemprompt_templates::{
    CoreTemplateProvider, FileSystemLoader, TemplateRegistryBuilder,
};

async fn setup_templates() -> Result<(), Box<dyn std::error::Error>> {
    let provider = CoreTemplateProvider::discover_from("./templates").await?;
    let loader = FileSystemLoader::new(vec!["./templates".into()]);

    let registry = TemplateRegistryBuilder::new()
        .with_provider(Arc::new(provider))
        .with_loader(Arc::new(loader))
        .build_and_init()
        .await?;

    let html = registry.render("page", &serde_json::json!({
        "title": "Hello"
    }))?;

    Ok(())
}
```

## Module Layout

| Module | Purpose |
|--------|---------|
| `builder` | `TemplateRegistryBuilder` for fluent registry construction. |
| `core_provider` | `CoreTemplateProvider` for filesystem template discovery. |
| `embedded_defaults` | `EmbeddedDefaultsProvider` bundling the in-tree `defaults/templates/`. |
| `registry/` | `TemplateRegistry` and Handlebars wiring: lifecycle, render/lookup queries, and stats. |

## Modules

### `builder`
Provides `TemplateRegistryBuilder` for fluent construction of `TemplateRegistry` instances. Supports chaining `with_provider()`, `with_loader()`, `with_extender()`, `with_component()`, and `with_page_provider()` methods.

### `core_provider`
Implements `CoreTemplateProvider` which discovers HTML templates from a filesystem directory. Reads optional `templates.yaml` manifests for metadata and infers content types from template name suffixes (`-post`, `-list`).

### `embedded_defaults`
`EmbeddedDefaultsProvider` exposes the in-tree `defaults/templates/` bundle so consumers get a working engine without filesystem access.

### `error`
Defines `TemplateError` with variants for common failure modes (`NotFound`, `LoadError`, `CompileError`, `RenderError`, `NoLoader`, `NotInitialized`) plus the `TemplateResult` alias.

### `registry`
Core `TemplateRegistry` that coordinates providers, loaders, extenders, and component renderers. Uses Handlebars for compilation and rendering, registers a `json` helper for JSON-LD safe serialisation, and resolves template conflicts by priority (lower values win). Split into `lifecycle`, `queries`, and `stats` submodules.

## Priority System

Templates are resolved by priority where lower values take precedence:

| Constant | Value | Use Case |
|----------|-------|----------|
| `EXTENSION_PRIORITY` | 500 | Override default templates |
| `DEFAULT_PRIORITY` | 1000 | Standard templates |

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---
