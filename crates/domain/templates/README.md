# systemprompt-templates

Template registry and management for SystemPrompt.

## Overview

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
