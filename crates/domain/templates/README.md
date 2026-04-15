<div align="center">
  <a href="https://systemprompt.io">
    <img src="https://systemprompt.io/logo.svg" alt="systemprompt.io" width="150" />
  </a>
  <p><strong>Production infrastructure for AI agents</strong></p>
  <p><a href="https://systemprompt.io">systemprompt.io</a> • <a href="https://systemprompt.io/documentation">Documentation</a> • <a href="https://github.com/systempromptio/systemprompt-core">Core</a> • <a href="https://github.com/systempromptio/systemprompt-template">Template</a></p>
</div>

---


# systemprompt-templates

<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="../../../assets/readme/terminals/dark/domain-templates.svg">
    <source media="(prefers-color-scheme: light)" srcset="../../../assets/readme/terminals/light/domain-templates.svg">
    <img alt="systemprompt-templates terminal demo" src="../../../assets/readme/terminals/dark/domain-templates.svg" width="100%">
  </picture>
</div>

Template registry and management for systemprompt.io.

[![Crates.io](https://img.shields.io/crates/v/systemprompt-templates.svg)](https://crates.io/crates/systemprompt-templates)
[![Documentation](https://docs.rs/systemprompt-templates/badge.svg)](https://docs.rs/systemprompt-templates)
[![License: BUSL-1.1](https://img.shields.io/badge/License-BUSL--1.1-blue.svg)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

## Overview

**Part of the Domain layer in the systemprompt.io architecture.**

This crate provides the core template system for discovering, loading, and rendering HTML templates using Handlebars. It supports a plugin architecture with providers, loaders, extenders, and component renderers.

## File Structure

```
src/
├── lib.rs              # Public exports and re-exports from template-provider
├── builder.rs          # TemplateRegistryBuilder for fluent construction
├── core_provider.rs    # CoreTemplateProvider for filesystem template discovery
├── error.rs            # TemplateError enum for error handling
└── registry.rs         # TemplateRegistry for managing templates and rendering

tests/
├── core_provider_tests.rs  # Tests for CoreTemplateProvider
└── registry_tests.rs       # Tests for TemplateRegistry
```

## Modules

### `builder`
Provides `TemplateRegistryBuilder` for fluent construction of `TemplateRegistry` instances. Supports chaining `with_provider()`, `with_loader()`, `with_extender()`, `with_component()`, and `with_page_provider()` methods.

### `core_provider`
Implements `CoreTemplateProvider` which discovers HTML templates from a filesystem directory. Reads optional `templates.yaml` manifests for metadata and infers content types from template name suffixes (`-post`, `-list`).

### `error`
Defines `TemplateError` with variants for common failure modes: `NotFound`, `LoadError`, `CompileError`, `RenderError`, `NoLoader`, and `NotInitialized`.

### `registry`
Core `TemplateRegistry` struct that coordinates template providers, loaders, extenders, and component renderers. Uses Handlebars for template compilation and rendering. Resolves template conflicts by priority (lower values win).

## Usage

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

## Priority System

Templates are resolved by priority where lower values take precedence:

| Constant | Value | Use Case |
|----------|-------|----------|
| `EXTENSION_PRIORITY` | 500 | Override default templates |
| `DEFAULT_PRIORITY` | 1000 | Standard templates |

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
systemprompt-templates = "0.0.1"
```

## License

Business Source License 1.1 - See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE) for details.
