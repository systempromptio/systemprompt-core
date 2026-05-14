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

# systemprompt-generator

<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/app-generator.svg">
    <source media="(prefers-color-scheme: light)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/light/app-generator.svg">
    <img alt="systemprompt-generator terminal demo" src="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/app-generator.svg" width="100%">
  </picture>
</div>

[![Crates.io](https://img.shields.io/crates/v/systemprompt-generator.svg?style=flat-square)](https://crates.io/crates/systemprompt-generator)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-generator?style=flat-square)](https://docs.rs/systemprompt-generator)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

Static site generation, theme rendering, and asset bundling for systemprompt.io AI governance dashboards. Coordinates domain services to generate prerendered HTML pages, sitemaps, RSS feeds, and optimized assets via a Handlebars and Markdown pipeline.

**Layer**: App — orchestrates domain modules. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Overview

Part of the App layer in the systemprompt.io architecture.
**Integrations** · [Extensible Architecture](https://systemprompt.io/features/extensible-architecture)

This application-layer crate orchestrates the full content publishing pipeline:

1. **Content Ingestion** - Fetches and processes content from sources
2. **Prerendering** - Generates static HTML for all content pages
3. **Asset Organization** - Copies and organizes CSS, JS, fonts, and images
4. **Feed Generation** - Creates sitemaps and RSS feeds for SEO

## Architecture

```
src/
├── lib.rs                    # Public API exports and crate-level docs
├── api.rs                    # HTTP API content fetching
├── assets.rs                 # Dist asset organisation (organize_dist_assets)
│
├── build/                    # Web build orchestration
│   ├── mod.rs               # Module exports
│   ├── orchestrator.rs      # BuildOrchestrator, BuildMode, BuildError
│   ├── steps.rs             # CSS build steps
│   └── validation.rs        # Sitemap URL validation
│
├── content/                  # Content processing
│   ├── mod.rs               # Module exports
│   ├── markdown.rs          # Markdown rendering, frontmatter extraction
│   └── toc.rs               # Table of contents extraction and heading IDs
│
├── error/                    # Typed errors
│   ├── mod.rs               # PublishError, GeneratorResult
│   └── suggestions.rs       # Human-readable error-suggestion strings
│
├── jobs/                     # Scheduled job definitions
│   ├── mod.rs               # Module exports
│   ├── copy_assets.rs       # execute_copy_extension_assets entry point
│   ├── content_prerender.rs # ContentPrerenderJob
│   └── page_prerender.rs    # PagePrerenderJob
│
├── prerender/                # Static page generation
│   ├── mod.rs               # Module exports
│   ├── engine.rs            # prerender_content, prerender_pages entry points
│   ├── context.rs           # PrerenderContext
│   ├── content.rs           # Source processing, item rendering
│   ├── fetch.rs             # Database content fetching with retries
│   ├── list.rs              # Listing / index page rendering
│   ├── render.rs            # Per-item render orchestration
│   └── utils.rs             # Shared prerender helpers
│
├── rss/                      # RSS feed generation
│   ├── mod.rs               # Module exports
│   ├── generator.rs         # generate_feed / generate_feed_with_providers
│   ├── default_provider.rs  # DefaultRssFeedProvider
│   └── xml.rs               # RssChannel, RssItem, XML building
│
├── sitemap/                  # Sitemap generation
│   ├── mod.rs               # Module exports
│   ├── generator.rs         # generate_sitemap entry point
│   ├── default_provider.rs  # DefaultSitemapProvider
│   └── xml.rs               # SitemapUrl, build_sitemap_xml, build_sitemap_index
│
└── templates/                # Template configuration loading
    ├── mod.rs               # Module exports
    └── engine.rs            # load_web_config, get_templates_path
```

### Module Descriptions

| Module | Purpose |
|--------|---------|
| `build` | Orchestrates the web build with progress reporting and CSS organisation |
| `content` | Markdown rendering, frontmatter extraction, and TOC generation |
| `error` | Typed `PublishError` and `GeneratorResult` returned across the public API |
| `jobs` | Scheduled jobs registered with the systemprompt scheduler via `inventory` |
| `prerender` | Static HTML generation for content sources and registered page prerenderers |
| `rss` | RSS 2.0 feed generation with a default feed provider |
| `sitemap` | XML sitemap generation with chunking and a default sitemap provider |
| `templates` | Template-path resolution and `WebConfig` loading |

### Key Types

| Type | Description |
|------|-------------|
| `BuildOrchestrator` | Coordinates CSS organisation and sitemap validation with progress reporting |
| `BuildMode` | Build-configuration variants |
| `BuildError` | Typed errors emitted by the build orchestrator |
| `PublishError` / `GeneratorResult` | Public error and result alias for every entry point |
| `PagePrerenderResult` | Outcome of a single page-prerenderer invocation |
| `ContentPrerenderJob` / `PagePrerenderJob` | Scheduler jobs for content and page prerendering |
| `SitemapUrl` / `DefaultSitemapProvider` | Sitemap entries and default provider |
| `RssChannel` / `RssItem` / `GeneratedFeed` / `DefaultRssFeedProvider` | RSS feed types and default provider |

## Usage

```toml
[dependencies]
systemprompt-generator = "0.9.0"
```

### Public Exports

```rust
pub use assets::organize_dist_assets;
pub use build::{BuildError, BuildMode, BuildOrchestrator};
pub use content::{extract_frontmatter, render_markdown};
pub use error::{GeneratorResult, PublishError};
pub use prerender::{PagePrerenderResult, prerender_content, prerender_pages};
pub use rss::{
    DefaultRssFeedProvider, GeneratedFeed, RssChannel, RssItem, build_rss_xml,
    generate_feed, generate_feed_with_providers,
};
pub use sitemap::{
    DefaultSitemapProvider, SitemapUrl, build_sitemap_index, build_sitemap_xml,
    escape_xml, generate_sitemap,
};
pub use templates::load_web_config;
pub use jobs::{ContentPrerenderJob, PagePrerenderJob, execute_copy_extension_assets};
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| `systemprompt-database` | Database connection pool |
| `systemprompt-content` | Content repository and models |
| `systemprompt-templates` | Template registry and rendering |
| `systemprompt-models` | Configuration and domain types |
| `systemprompt-identifiers` | Typed identifiers (SourceId, ContentId) |
| `systemprompt-traits` | Job trait interface |
| `systemprompt-extension` | Extension discovery and assets |
| `systemprompt-files` | File storage configuration |
| `handlebars` | Template engine |
| `comrak` | Markdown to HTML |

## Architecture Notes

This crate follows the application layer pattern:

- **Orchestration only** - No business logic; delegates to domain services
- **Read-only domain access** - Uses `ContentRepository` for data fetching
- **Job interface** - Jobs implement `systemprompt_traits::Job` for scheduling
- **Template delegation** - Uses `TemplateRegistry` from domain layer
- **Config via models** - Uses `Config::get()` instead of direct `env::var()`

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-generator)** · **[docs.rs](https://docs.rs/systemprompt-generator)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>App layer · Own how your organization uses AI.</sub>

</div>
