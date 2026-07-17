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
[![codecov](https://img.shields.io/codecov/c/github/systempromptio/systemprompt-core/main?style=flat-square&logo=codecov)](https://codecov.io/gh/systempromptio/systemprompt-core)

The static site your governance dashboards ship from, built on the same PostgreSQL you own. This crate turns content records into prerendered HTML, sitemaps, RSS feeds, and organised assets, all from a Markdown and template pipeline that runs inside your binary.

**Layer**: App, orchestrates domain modules. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Overview

The content publishing pipeline runs in four stages:

1. **Content fetch** reads sources and content records from the database.
2. **Prerender** generates static HTML for every content page and registered page prerenderer.
3. **Asset organisation** copies and arranges CSS, JS, fonts, and images into the dist tree.
4. **Feed generation** writes sitemaps and RSS feeds for search discovery.

**Integrations** · [Extensible Architecture](https://systemprompt.io/features/extensible-architecture)

## Modules

| Module | Purpose |
|--------|---------|
| `build` | Orchestrates the web build with progress reporting and CSS organisation (`BuildOrchestrator`, `BuildMode`, `BuildError`) |
| `content` | Markdown rendering and frontmatter extraction |
| `prerender` | Static HTML generation for content sources and page prerenderers, plus table-of-contents extraction (`toc.rs`) and JSON data merging |
| `rss` | RSS 2.0 feed generation with a default feed provider |
| `sitemap` | XML sitemap generation with chunking and a default sitemap provider |
| `templates` | Template-path resolution and `WebConfig` loading |
| `jobs` | Scheduled jobs registered with the systemprompt scheduler via `inventory` |
| `error` | Typed `PublishError` and `GeneratorResult` returned across the public API |

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
systemprompt-generator = "0.21"
```

### Public Exports

```rust
pub use assets::organize_dist_assets;
pub use build::{BuildError, BuildMode, BuildOrchestrator};
pub use content::{extract_frontmatter, render_markdown};
pub use error::{GeneratorResult, PublishError};
pub use prerender::{
    PagePrerenderResult, TocResult, generate_toc, merge_json_data, prerender_content,
    prerender_pages,
};
pub use rss::{
    DefaultRssFeedProvider, GeneratedFeed, RssChannel, RssItem, build_rss_xml,
    generate_feed, generate_feed_with_providers,
};
pub use sitemap::{
    DefaultSitemapProvider, SitemapUrl, build_sitemap_index, build_sitemap_xml,
    escape_xml, generate_sitemap,
};
pub use templates::{get_templates_path, load_web_config};
pub use jobs::{ContentPrerenderJob, PagePrerenderJob, copy_asset, execute_copy_extension_assets};
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| `systemprompt-database` | Database connection pool |
| `systemprompt-content` | Content repository and models |
| `systemprompt-templates` | Template registry and rendering |
| `systemprompt-template-provider` | Template provider traits |
| `systemprompt-provider-contracts` | Provider-contract registration |
| `systemprompt-models` | Configuration and domain types |
| `systemprompt-identifiers` | Typed identifiers (SourceId, ContentId) |
| `systemprompt-traits` | Job trait interface |
| `systemprompt-extension` | Extension discovery and assets |
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
