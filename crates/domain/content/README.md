# Blog Module

Content management with link tracking, analytics, static content generation, and scheduled jobs.

## Directories

| Directory | Purpose |
|-----------|---------|
| `src/analytics/` | Link click tracking and analytics (repository + service) |
| `src/api/routes/` | HTTP route handlers for content and links |
| `src/generator/` | Static content generation (sitemap, prerender, templates) |
| `src/jobs/` | Scheduled jobs for content ingestion and publishing |
| `src/models/` | Domain types and builders |
| `src/repository/` | Database access for content, links, images, search |
| `src/services/` | Business logic for ingestion, links, search, validation |
| `schema/` | SQL schema files |

## Key Files

| File | Purpose |
|------|---------|
| `src/lib.rs` | Crate root with public exports |
| `src/error.rs` | ContentError enum using thiserror |
| `src/analytics/repository.rs` | LinkAnalyticsRepository |
| `src/analytics/service.rs` | LinkAnalyticsService |
| `src/services/content.rs` | ContentService (used by routes) |
| `src/services/content_provider.rs` | DefaultContentProvider (implements ContentProvider trait) |
| `src/jobs/content_ingestion.rs` | ContentIngestionJob (implements Job trait) |
| `src/jobs/publish_content.rs` | PublishContentJob (implements Job trait) |
| `src/repository/content/mod.rs` | ContentRepository |
| `src/repository/search/mod.rs` | SearchRepository |
| `src/services/ingestion/mod.rs` | IngestionService |
| `src/services/search/mod.rs` | SearchService |

## Structure

```
src/
├── analytics/
│   ├── mod.rs
│   ├── repository.rs
│   └── service.rs
├── api/
│   ├── mod.rs
│   └── routes/
│       ├── mod.rs
│       ├── blog.rs
│       ├── query.rs
│       └── links/
├── generator/
│   ├── mod.rs
│   ├── assets.rs
│   ├── cards.rs
│   ├── images.rs
│   ├── markdown.rs
│   ├── prerender.rs
│   ├── prerender_index.rs
│   ├── prerender_parent.rs
│   ├── sitemap.rs
│   ├── sitemap_xml.rs
│   ├── templates.rs
│   ├── templates_data.rs
│   ├── templates_html.rs
│   ├── templates_items.rs
│   ├── templates_navigation.rs
│   ├── templates_paper.rs
│   ├── web_build.rs
│   ├── web_build_steps.rs
│   └── web_build_validation.rs
├── jobs/
│   ├── mod.rs
│   ├── content_ingestion.rs
│   └── publish_content.rs
├── models/
│   ├── mod.rs
│   ├── builders/
│   ├── content.rs
│   ├── content_error.rs
│   ├── link.rs
│   ├── paper.rs
│   └── search.rs
├── repository/
│   ├── mod.rs
│   ├── content/
│   ├── images/
│   ├── link/
│   └── search/
├── services/
│   ├── mod.rs
│   ├── content.rs
│   ├── content_provider.rs
│   ├── ingestion/
│   ├── link/
│   ├── search/
│   └── validation/
├── error.rs
└── lib.rs
```

## Dependencies

Internal:
- systemprompt-core-database
- systemprompt-core-logging
- systemprompt-core-system
- systemprompt-core-files
- systemprompt-identifiers
- systemprompt-models
- systemprompt-traits

## Traits Implemented

- `ContentProvider` (systemprompt-traits) - DefaultContentProvider
- `Job` (systemprompt-traits) - ContentIngestionJob, PublishContentJob

## Exports

```rust
use systemprompt_core_content::{
    Content, ContentMetadata, IngestionOptions, IngestionReport,
    SearchFilters, SearchRequest, SearchResponse, SearchResult,
    ContentRepository, SearchRepository,
    DefaultContentProvider, IngestionService, SearchService,
    LinkAnalyticsRepository, LinkAnalyticsService,
    ContentIngestionJob, PublishContentJob,
    generate_sitemap, optimize_images, organize_css_files,
    prerender_content, BuildError, BuildMode, BuildOrchestrator,
    TemplateEngine, ContentError,
};
```
