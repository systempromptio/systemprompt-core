<div align="center">
  <a href="https://systemprompt.io">
    <img src="https://systemprompt.io/logo.svg" alt="systemprompt.io" width="150" />
  </a>
  <p><strong>Production infrastructure for AI agents</strong></p>
  <p><a href="https://systemprompt.io">systemprompt.io</a> • <a href="https://systemprompt.io/documentation">Documentation</a> • <a href="https://github.com/systempromptio/systemprompt-core">Core</a> • <a href="https://github.com/systempromptio/systemprompt-template">Template</a></p>
</div>

---


# systemprompt-content

<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="../../../assets/readme/terminals/dark/domain-content.svg">
    <source media="(prefers-color-scheme: light)" srcset="../../../assets/readme/terminals/light/domain-content.svg">
    <img alt="systemprompt-content terminal demo" src="../../../assets/readme/terminals/dark/domain-content.svg" width="100%">
  </picture>
</div>

Content module for systemprompt.io with content management, analytics, and event tracking.

[![Crates.io](https://img.shields.io/crates/v/systemprompt-content.svg)](https://crates.io/crates/systemprompt-content)
[![Documentation](https://docs.rs/systemprompt-content/badge.svg)](https://docs.rs/systemprompt-content)
[![License: BUSL-1.1](https://img.shields.io/badge/License-BUSL--1.1-blue.svg)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

## Overview

**Part of the Domain layer in the systemprompt.io architecture.**
**Integrations** · [Skill Marketplace](https://systemprompt.io/features/skill-marketplace)

This crate handles all content-related functionality:

- **Content Management**: CRUD operations for markdown content with frontmatter metadata
- **Content Ingestion**: Parse and ingest markdown files from configured directories
- **Search**: Full-text search across content with category filtering
- **Link Tracking**: Campaign links with UTM parameters and click analytics
- **Configuration**: Validated content source configuration with routing

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

## Public Exports

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

## Traits Implemented

- `ContentProvider` (systemprompt-traits) - `DefaultContentProvider`
- `Job` (systemprompt-traits) - `ContentIngestionJob`
- `ContentRouting` (systemprompt-models) - `ContentConfigValidated`, `ContentReady`

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
systemprompt-content = "0.0.1"
```

## License

Business Source License 1.1 - See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE) for details.
