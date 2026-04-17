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

# systemprompt-templates

<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/domain-templates.svg">
    <source media="(prefers-color-scheme: light)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/light/domain-templates.svg">
    <img alt="systemprompt-templates terminal demo" src="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/domain-templates.svg" width="100%">
  </picture>
</div>

[![Crates.io](https://img.shields.io/crates/v/systemprompt-templates.svg?style=flat-square)](https://crates.io/crates/systemprompt-templates)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-templates?style=flat-square)](https://docs.rs/systemprompt-templates)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

Template registry, loading, and rendering for systemprompt.io config-as-code AI governance deployments. Handlebars-powered template engine for the MCP governance pipeline with plugin architecture, priority resolution, and filesystem discovery.

**Layer**: Domain — business-logic modules that implement systemprompt.io features. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Overview

**Integrations** · [Extensible Architecture](https://systemprompt.io/features/extensible-architecture)

This crate provides the core template system for discovering, loading, and rendering HTML templates using Handlebars. It supports a plugin architecture with providers, loaders, extenders, and component renderers.

## Usage

```toml
[dependencies]
systemprompt-templates = "0.2.1"
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

## Priority System

Templates are resolved by priority where lower values take precedence:

| Constant | Value | Use Case |
|----------|-------|----------|
| `EXTENSION_PRIORITY` | 500 | Override default templates |
| `DEFAULT_PRIORITY` | 1000 | Standard templates |

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-templates)** · **[docs.rs](https://docs.rs/systemprompt-templates)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Domain layer · Own how your organization uses AI.</sub>

</div>
