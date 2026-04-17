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

# systemprompt-content

<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/domain-content.svg">
    <source media="(prefers-color-scheme: light)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/light/domain-content.svg">
    <img alt="systemprompt-content terminal demo" src="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/domain-content.svg" width="100%">
  </picture>
</div>

[![Crates.io](https://img.shields.io/crates/v/systemprompt-content.svg?style=flat-square)](https://crates.io/crates/systemprompt-content)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-content?style=flat-square)](https://docs.rs/systemprompt-content)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

Markdown content management, sources, and event tracking for systemprompt.io AI governance dashboards. Governed publishing pipeline for the MCP governance platform with content ingestion, full-text search, link tracking, and UTM analytics.

**Layer**: Domain — business-logic modules that implement systemprompt.io features. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Overview

**Integrations** · [Skill Marketplace](https://systemprompt.io/features/skill-marketplace)

This crate handles all content-related functionality:

- **Content Management**: CRUD operations for markdown content with frontmatter metadata
- **Content Ingestion**: Parse and ingest markdown files from configured directories
- **Search**: Full-text search across content with category filtering
- **Link Tracking**: Campaign links with UTM parameters and click analytics
- **Configuration**: Validated content source configuration with routing

## Usage

```toml
[dependencies]
systemprompt-content = "0.2.1"
```

```rust
use systemprompt_content::{
    // Models
    Content, ContentMetadata, IngestionOptions, IngestionReport,
    IngestionSource, SearchFilters, SearchRequest, SearchResponse,
    SearchResult, UpdateContentParams,

    // Repositories
    ContentRepository, LinkAnalyticsRepository, SearchRepository,

    // Services
    DefaultContentProvider, IngestionService, LinkAnalyticsService, SearchService,

    // Jobs
    ContentIngestionJob,

    // Config
    ContentConfigValidated, ContentReady, ContentSourceConfigValidated,
    LoadStats, ParsedContent, ValidationResult,

    // API
    router, get_content_handler, list_content_by_source_handler, query_handler,

    // Error
    ContentError,

    // Validation
    validate_content_metadata, validate_paper_metadata,
    validate_paper_section_ids_unique,
};
```

## File Structure

```
src/
├── api/
│   ├── mod.rs                      # Router exports
│   └── routes/
│       ├── mod.rs                  # Route composition
│       ├── blog.rs                 # Content handlers (list, get by slug)
│       ├── query.rs                # Search handler
│       └── links/
│           ├── mod.rs              # Link route exports
│           ├── handlers.rs         # Link CRUD + redirect handlers
│           └── types.rs            # Request/response types
├── config/
│   ├── mod.rs                      # Config exports
│   ├── validated.rs                # ContentConfigValidated (validation logic)
│   └── ready.rs                    # ContentReady (loaded content cache)
├── jobs/
│   ├── mod.rs                      # Job exports
│   └── content_ingestion.rs        # ContentIngestionJob (implements Job trait)
├── models/
│   ├── mod.rs                      # Model exports
│   ├── builders/
│   │   ├── mod.rs                  # Builder exports
│   │   ├── content.rs              # CreateContentParams, UpdateContentParams
│   │   └── link.rs                 # CreateLinkParams, RecordClickParams, TrackClickParams
│   ├── content.rs                  # Content, ContentMetadata, IngestionReport
│   ├── content_error.rs            # ContentError (validation errors)
│   ├── link.rs                     # CampaignLink, LinkClick, LinkPerformance
│   ├── paper.rs                    # PaperMetadata, PaperSection
│   └── search.rs                   # SearchRequest, SearchResponse, SearchResult
├── repository/
│   ├── mod.rs                      # Repository exports
│   ├── content/
│   │   ├── mod.rs                  # ContentRepository
│   │   ├── queries.rs              # Read operations (get, list)
│   │   └── mutations.rs            # Write operations (create, update, delete)
│   ├── link/
│   │   ├── mod.rs                  # LinkRepository
│   │   └── analytics.rs            # LinkAnalyticsRepository
│   └── search/
│       └── mod.rs                  # SearchRepository
├── services/
│   ├── mod.rs                      # Service exports
│   ├── content.rs                  # ContentService
│   ├── content_provider.rs         # DefaultContentProvider (implements ContentProvider)
│   ├── ingestion/
│   │   ├── mod.rs                  # IngestionService
│   │   ├── parser.rs               # Paper chapter loading, frontmatter validation
│   │   └── scanner.rs              # Directory scanning, file validation
│   ├── link/
│   │   ├── mod.rs                  # Link service exports
│   │   ├── analytics.rs            # LinkAnalyticsService
│   │   └── generation.rs           # LinkGenerationService
│   ├── search/
│   │   └── mod.rs                  # SearchService
│   └── validation/
│       └── mod.rs                  # Content and paper metadata validation
├── error.rs                        # ContentError enum (thiserror)
└── lib.rs                          # Crate root with public exports
```

## Modules

### api/routes/

HTTP route handlers for content retrieval, search queries, and link management. Routes delegate to services, never directly to repositories.

### config/

Content source configuration validation and caching. `ContentConfigValidated` validates YAML configuration, `ContentReady` loads and caches parsed content for fast access.

### jobs/

Background jobs for content processing. `ContentIngestionJob` scans configured directories and syncs markdown content to the database.

### models/

Domain types for content, links, and search. Builder pattern used for complex parameter types (`CreateContentParams`, `TrackClickParams`).

### repository/

Database access layer using SQLX macros for compile-time SQL verification. Repositories handle data persistence with no business logic. Split into `queries.rs` (reads) and `mutations.rs` (writes) for clarity.

### services/

Business logic layer. Services coordinate repositories and implement domain operations:

- `ContentService`: Content retrieval by source and slug
- `IngestionService`: Directory scanning and content parsing
- `SearchService`: Category and keyword search
- `LinkGenerationService`: Campaign link creation with UTM parameters
- `LinkAnalyticsService`: Click tracking and performance metrics

## Dependencies

**Internal (shared/):**
- `systemprompt-models` - Cross-crate model types
- `systemprompt-identifiers` - Typed IDs (ContentId, LinkId, etc.)
- `systemprompt-traits` - ContentProvider, Job traits
- `systemprompt-provider-contracts` - Job registration macros

**Internal (infra/):**
- `systemprompt-database` - Database pool abstraction
- `systemprompt-logging` - Logging infrastructure
- `systemprompt-config` - Configuration management

## Traits Implemented

- `ContentProvider` (systemprompt-traits) - `DefaultContentProvider`
- `Job` (systemprompt-traits) - `ContentIngestionJob`
- `ContentRouting` (systemprompt-models) - `ContentConfigValidated`, `ContentReady`

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-content)** · **[docs.rs](https://docs.rs/systemprompt-content)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Domain layer · Own how your organization uses AI.</sub>

</div>
