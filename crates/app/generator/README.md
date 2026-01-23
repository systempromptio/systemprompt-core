<div align="center">
  <a href="https://systemprompt.io">
    <img src="https://systemprompt.io/logo.svg" alt="systemprompt.io" width="150" />
  </a>
  <p><strong>Production infrastructure for AI agents</strong></p>
  <p><a href="https://systemprompt.io">systemprompt.io</a> • <a href="https://github.com/systempromptio/systemprompt">GitHub</a> • <a href="https://docs.systemprompt.io">Documentation</a></p>
</div>

---


# systemprompt-generator

Static site generation and content publishing orchestrator for systemprompt.io. Coordinates domain services to generate prerendered HTML pages, sitemaps, RSS feeds, and optimized assets.

## Overview

This application-layer crate orchestrates the full content publishing pipeline:

1. **Content Ingestion** - Fetches and processes content from sources
2. **Prerendering** - Generates static HTML for all content pages
3. **Asset Organization** - Copies and organizes CSS, JS, fonts, and images
4. **Feed Generation** - Creates sitemaps and RSS feeds for SEO

## File Structure

```
src/
├── lib.rs                    # Public API exports
├── api.rs                    # HTTP API content fetching
├── assets.rs                 # Asset copying and organization
│
├── build/                    # Web build orchestration
│   ├── mod.rs               # Module exports
│   ├── orchestrator.rs      # BuildOrchestrator, BuildMode, BuildError
│   ├── steps.rs             # Theme, TypeScript, Vite, CSS build steps
│   └── validation.rs        # Sitemap URL validation
│
├── content/                  # Content processing
│   ├── mod.rs               # Module exports
│   ├── cards.rs             # Card HTML generation, image URL normalization
│   └── markdown.rs          # Markdown rendering, frontmatter extraction
│
├── jobs/                     # Scheduled job definitions
│   ├── mod.rs               # Module exports
│   ├── copy_assets.rs       # CopyExtensionAssetsJob
│   └── publish_content.rs   # PublishContentJob (main pipeline)
│
├── prerender/                # Static page generation
│   ├── mod.rs               # Module exports
│   ├── engine.rs            # prerender_content, prerender_homepage entry points
│   ├── context.rs           # PrerenderContext, HomepageBranding
│   ├── content.rs           # Source processing, item rendering
│   ├── fetch.rs             # Database content fetching with retries
│   ├── homepage.rs          # Homepage-specific rendering
│   ├── index.rs             # GenerateParentIndexParams, parent index generation
│   └── parent.rs            # RenderParentParams, parent route rendering
│
├── rss/                      # RSS feed generation
│   ├── mod.rs               # Module exports
│   ├── generator.rs         # generate_feed entry point
│   └── xml.rs               # RssChannel, RssItem, XML building
│
├── sitemap/                  # Sitemap generation
│   ├── mod.rs               # Module exports
│   ├── generator.rs         # generate_sitemap entry point
│   └── xml.rs               # SitemapUrl, XML building
│
└── templates/                # Template data preparation
    ├── mod.rs               # Module exports
    ├── engine.rs            # load_web_config, get_templates_path
    ├── html.rs              # Related content, CTA links, references HTML
    ├── items.rs             # find_latest_items, find_popular_items
    ├── navigation.rs        # Footer, social action bar HTML generation
    ├── paper.rs             # Paper content type: TOC, sections, read time
    └── data/                # Template data preparation (split for maintainability)
        ├── mod.rs           # prepare_template_data entry point
        ├── types.rs         # TemplateDataParams, DateData, ImageData, etc.
        ├── extractors.rs    # Config and field extraction functions
        └── builders.rs      # JSON template building
```

## Module Descriptions

| Module | Purpose |
|--------|---------|
| `build` | Orchestrates web frontend build: theme generation, TypeScript, Vite, CSS |
| `content` | Markdown rendering and content card HTML generation |
| `jobs` | Scheduled jobs implementing the `Job` trait for the scheduler |
| `prerender` | Static HTML generation for content pages and homepage |
| `rss` | RSS 2.0 feed generation with Atom namespace support |
| `sitemap` | XML sitemap generation with chunking for large sites |
| `templates` | Template data preparation and HTML fragment generation |

## Key Types

| Type | Description |
|------|-------------|
| `BuildOrchestrator` | Coordinates theme, TypeScript, Vite, and CSS build steps |
| `BuildMode` | `Development`, `Production`, or `Docker` build configuration |
| `PrerenderContext` | Shared context for prerendering: db pool, config, templates |
| `PublishContentJob` | Main job that runs the full publishing pipeline |
| `SitemapUrl` | URL entry for sitemap XML generation |
| `RssChannel` / `RssItem` | RSS feed data structures |
| `TemplateDataParams` | Parameters for preparing template data |

## Public Exports

```rust
pub use assets::{copy_implementation_assets, organize_css_files, organize_js_files};
pub use build::{BuildError, BuildMode, BuildOrchestrator};
pub use content::{extract_frontmatter, render_markdown};
pub use prerender::{prerender_content, prerender_homepage};
pub use rss::{build_rss_xml, generate_feed, RssChannel, RssItem};
pub use sitemap::{build_sitemap_index, build_sitemap_xml, generate_sitemap, SitemapUrl};
pub use templates::{generate_footer_html, load_web_config, prepare_template_data};
pub use jobs::{CopyExtensionAssetsJob, PublishContentJob};
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

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
systemprompt-generator = "0.0.1"
```

## License

FSL-1.1-ALv2 - See [LICENSE](https://github.com/systempromptio/systemprompt/blob/main/LICENSE) for details.
