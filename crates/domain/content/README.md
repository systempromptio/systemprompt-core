# systemprompt-content

Content management domain module for SystemPrompt. Provides content ingestion, search, link tracking, analytics, and configuration validation.

## Overview

This crate handles all content-related functionality:

- **Content Management**: CRUD operations for markdown content with frontmatter metadata
- **Content Ingestion**: Parse and ingest markdown files from configured directories
- **Search**: Full-text search across content with category filtering
- **Link Tracking**: Campaign links with UTM parameters and click analytics
- **Configuration**: Validated content source configuration with routing

## File Structure

```
src/
├── analytics/
│   ├── mod.rs                  # Module exports
│   ├── repository.rs           # LinkAnalyticsRepository
│   └── service.rs              # LinkAnalyticsService
├── api/
│   ├── mod.rs                  # Router exports
│   └── routes/
│       ├── mod.rs              # Route composition
│       ├── blog.rs             # Content handlers (list, get by slug)
│       ├── query.rs            # Search handler
│       └── links/
│           ├── mod.rs          # Link route exports
│           ├── handlers.rs     # Link CRUD + redirect handlers
│           └── types.rs        # Request/response types
├── config/
│   ├── mod.rs                  # Config exports
│   ├── validated.rs            # ContentConfigValidated (validation logic)
│   └── ready.rs                # ContentReady (loaded content cache)
├── jobs/
│   ├── mod.rs                  # Job exports
│   └── content_ingestion.rs    # ContentIngestionJob (implements Job trait)
├── models/
│   ├── mod.rs                  # Model exports
│   ├── builders/
│   │   ├── mod.rs              # Builder exports
│   │   ├── content.rs          # CreateContentParams, UpdateContentParams
│   │   └── link.rs             # CreateLinkParams, RecordClickParams, TrackClickParams
│   ├── content.rs              # Content, ContentMetadata, IngestionReport
│   ├── content_error.rs        # ContentError (validation errors)
│   ├── link.rs                 # CampaignLink, LinkClick, LinkPerformance
│   ├── paper.rs                # PaperMetadata, PaperSection
│   └── search.rs               # SearchRequest, SearchResponse, SearchResult
├── repository/
│   ├── mod.rs                  # Repository exports
│   ├── content/
│   │   └── mod.rs              # ContentRepository
│   ├── link/
│   │   ├── mod.rs              # LinkRepository
│   │   └── analytics.rs        # LinkAnalyticsRepository
│   └── search/
│       └── mod.rs              # SearchRepository
├── services/
│   ├── mod.rs                  # Service exports
│   ├── content.rs              # ContentService
│   ├── content_provider.rs     # DefaultContentProvider (implements ContentProvider)
│   ├── ingestion/
│   │   ├── mod.rs              # IngestionService
│   │   └── parser.rs           # Paper chapter loading, frontmatter validation
│   ├── link/
│   │   ├── mod.rs              # Link service exports
│   │   ├── analytics.rs        # LinkAnalyticsService
│   │   └── generation.rs       # LinkGenerationService
│   ├── search/
│   │   └── mod.rs              # SearchService
│   └── validation/
│       └── mod.rs              # Content and paper metadata validation
├── error.rs                    # ContentError enum (database, validation, parse errors)
└── lib.rs                      # Crate root with public exports
```

## Modules

### analytics/

Link click tracking and campaign performance analytics. Tracks unique clicks per session, conversion events, and content journey mapping.

### api/routes/

HTTP route handlers for content retrieval, search queries, and link management. Routes delegate to services, never directly to repositories.

### config/

Content source configuration validation and caching. `ContentConfigValidated` validates YAML configuration, `ContentReady` loads and caches parsed content for fast access.

### jobs/

Background jobs for content processing. `ContentIngestionJob` scans configured directories and syncs markdown content to the database.

### models/

Domain types for content, links, and search. Builder pattern used for complex parameter types (`CreateContentParams`, `TrackClickParams`).

### repository/

Database access layer using SQLX macros for compile-time SQL verification. Repositories handle data persistence with no business logic.

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
    ContentRepository, SearchRepository,

    // Services
    DefaultContentProvider, IngestionService, SearchService,

    // Analytics
    LinkAnalyticsRepository, LinkAnalyticsService,

    // Jobs
    ContentIngestionJob,

    // Config
    ContentConfigValidated, ContentReady, LoadStats, ParsedContent,
    ValidationResult,

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
