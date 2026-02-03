# Page Prerenderer Implementation Guide

This document describes the extension-driven page prerendering architecture in systemprompt-core.

## Overview

Page prerendering is fully extension-driven. The core framework provides:
- `PagePrerenderer` trait for extensions to implement
- `PagePrepareContext` with access to configuration and data
- Generic engine that discovers and executes prerenderers

Extensions own:
- Page type definitions
- Base data construction
- Template selection
- Output path decisions

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│  CORE (provider-contracts, generator)                           │
│                                                                 │
│  trait PagePrerenderer                                          │
│  trait PageDataProvider                                         │
│  trait ComponentRenderer                                        │
│                                                                 │
│  fn prerender_pages(ctx) {                                      │
│      for p in ctx.registry.page_prerenderers() {                │
│          let spec = p.prepare(ctx)?;                            │
│          // collect providers, components                       │
│          // render template                                     │
│          // write to output_path                                │
│      }                                                          │
│  }                                                              │
└─────────────────────────────────────────────────────────────────┘
                              ↑
              Extensions implement traits
                              ↑
┌─────────────────────────────────────────────────────────────────┐
│  EXTENSIONS                                                     │
│                                                                 │
│  impl PagePrerenderer for MyPagePrerenderer {                   │
│      fn page_type(&self) -> &str { "my-page" }                 │
│      async fn prepare(&self, ctx) -> Result<Option<Spec>>      │
│  }                                                              │
└─────────────────────────────────────────────────────────────────┘
```

## Key Types

### PagePrerenderer Trait

Location: `crates/shared/provider-contracts/src/page_prerenderer.rs`

```rust
#[async_trait]
pub trait PagePrerenderer: Send + Sync {
    fn page_type(&self) -> &str;

    fn priority(&self) -> u32 {
        100
    }

    async fn prepare(&self, ctx: &PagePrepareContext<'_>) -> Result<Option<PageRenderSpec>>;
}
```

### PagePrepareContext

Provides access to configuration without exposing internal types:

```rust
pub struct PagePrepareContext<'a> {
    pub web_config: &'a WebConfig,
    content_config: &'a (dyn Any + Send + Sync),
    db_pool: &'a (dyn Any + Send + Sync),
    dist_dir: &'a Path,
}

impl<'a> PagePrepareContext<'a> {
    pub fn content_config<T: 'static>(&self) -> Option<&T>
    pub fn db_pool<T: 'static>(&self) -> Option<&T>
    pub fn dist_dir(&self) -> &Path
}
```

### PageRenderSpec

Returned by `prepare()` to specify what to render:

```rust
pub struct PageRenderSpec {
    pub template_name: String,
    pub base_data: serde_json::Value,
    pub output_path: PathBuf,
}
```

## Implementing a Page Prerenderer

### Step 1: Create the Prerenderer

```rust
use std::path::PathBuf;
use anyhow::Result;
use async_trait::async_trait;
use systemprompt_models::ContentConfigRaw;
use systemprompt_provider_contracts::{
    PagePrepareContext, PagePrerenderer, PageRenderSpec,
};

const PAGE_TYPE: &str = "docs-index";
const TEMPLATE_NAME: &str = "docs-index";
const OUTPUT_FILE: &str = "docs/index.html";

#[derive(Debug, Clone, Copy, Default)]
pub struct DocsIndexPrerenderer;

#[async_trait]
impl PagePrerenderer for DocsIndexPrerenderer {
    fn page_type(&self) -> &str {
        PAGE_TYPE
    }

    fn priority(&self) -> u32 {
        100
    }

    async fn prepare(&self, ctx: &PagePrepareContext<'_>) -> Result<Option<PageRenderSpec>> {
        let base_data = serde_json::json!({
            "site": ctx.web_config,
            "page_title": "Documentation"
        });

        Ok(Some(PageRenderSpec::new(
            TEMPLATE_NAME,
            base_data,
            PathBuf::from(OUTPUT_FILE),
        )))
    }
}
```

### Step 2: Register with Extension

```rust
use std::sync::Arc;
use systemprompt_extension::prelude::*;

impl Extension for MyExtension {
    fn page_prerenderers(&self) -> Vec<Arc<dyn PagePrerenderer>> {
        vec![Arc::new(DocsIndexPrerenderer)]
    }
}
```

## Data Flow

1. **Discovery**: `ExtensionRegistry::discover()` finds all extensions
2. **Collection**: Extensions return prerenderers via `page_prerenderers()`
3. **Registration**: Prerenderers added to `TemplateRegistry`
4. **Execution**: Engine iterates prerenderers in priority order
5. **Preparation**: Each prerenderer's `prepare()` builds the render spec
6. **Enhancement**: Engine collects `PageDataProvider`s and `ComponentRenderer`s for the page type
7. **Merge**: Provider data merged with base data from spec
8. **Render**: Template rendered with merged data
9. **Output**: HTML written to spec's output path

## Page Data Providers

Extensions can also provide data for pages without owning the prerender:

```rust
impl PageDataProvider for MyDataProvider {
    fn provider_id(&self) -> &str {
        "my-data"
    }

    fn applies_to_pages(&self) -> Vec<String> {
        vec!["homepage".to_string(), "docs-index".to_string()]
    }

    async fn provide_page_data(&self, ctx: &PageContext<'_>) -> Result<Value> {
        Ok(serde_json::json!({
            "extra_field": "value"
        }))
    }
}
```

## Component Renderers

Components render HTML fragments for pages:

```rust
impl ComponentRenderer for MyComponent {
    fn component_id(&self) -> &str {
        "hero-section"
    }

    fn variable_name(&self) -> &str {
        "HERO_HTML"
    }

    fn applies_to(&self) -> Vec<String> {
        vec!["homepage".to_string()]
    }

    async fn render(&self, ctx: &ComponentContext<'_>) -> Result<RenderedComponent> {
        Ok(RenderedComponent::new(
            "HERO_HTML",
            "<section class=\"hero\">...</section>",
        ))
    }
}
```

## Default Homepage Prerenderer

The `ContentExtension` provides `DefaultHomepagePrerenderer`:

Location: `crates/domain/content/src/homepage_prerenderer.rs`

Features:
- Extracts branding from `ContentConfigRaw` and `WebConfig`
- Generates footer navigation HTML
- Provides standard template variables

## CLI Usage

```bash
systemprompt core content publish --step pages
```

Or run all steps:

```bash
systemprompt core content publish
```

## Extension Discovery

Extensions are discovered via the `inventory` crate at compile time. Injected extensions (via `set_injected_extensions()`) are automatically included in discovery.

```rust
register_extension!(MyExtension);
```

## Priority

Lower priority values indicate higher importance and execute first. When multiple prerenderers share the same `page_type`, only the first one (lowest priority) runs - others are skipped. This allows extensions to override core defaults.

| Priority | Use Case |
|----------|----------|
| 0-49 | Critical - overrides defaults |
| 50-99 | Core application pages |
| 100 | Default (fallback, easily overridden) |
| 101+ | Low priority/optional |

**Example**: Extension with `HomepagePrerenderer` (priority 10) overrides core's `DefaultHomepagePrerenderer` (priority 100).

## Error Handling

- Return `Ok(None)` to skip rendering (template not found, feature disabled)
- Return `Err(...)` for actual failures
- Engine logs warnings for missing templates but continues with other pages

## Migration from Hardcoded Homepage

The old `prerender_homepage()` function is deprecated. Migrate to:

1. Remove direct calls to `prerender_homepage()`
2. Ensure `ContentExtension` is registered (provides `DefaultHomepagePrerenderer`)
3. Use `prerender_pages()` or the `--step pages` CLI option

## Files Reference

| File | Purpose |
|------|---------|
| `crates/shared/provider-contracts/src/page_prerenderer.rs` | Trait definitions |
| `crates/shared/extension/src/lib.rs` | Extension trait with `page_prerenderers()` |
| `crates/domain/templates/src/registry.rs` | Registry for prerenderers |
| `crates/app/generator/src/prerender/engine.rs` | Execution engine |
| `crates/domain/content/src/homepage_prerenderer.rs` | Default homepage implementation |
