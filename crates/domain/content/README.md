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
[![codecov](https://img.shields.io/codecov/c/github/systempromptio/systemprompt-core/main?style=flat-square&logo=codecov)](https://codecov.io/gh/systempromptio/systemprompt-core)

Your content, ingested and served from your own binary. Markdown management with frontmatter, full-text search, campaign links, and UTM click analytics, all held in your PostgreSQL rather than a hosted CMS.

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
systemprompt-content = "0.21"
```

```rust
use systemprompt_content::{
    // Models
    Content, ContentMetadata, IngestionOptions, IngestionReport,
    IngestionSource, SearchFilters, SearchRequest, SearchResponse,
    SearchResult, UpdateContentParams, LinkType, TrackClickParams, UtmParams,

    // Repositories
    ContentRepository, LinkAnalyticsRepository, SearchRepository,

    // Services
    DefaultContentProvider, IngestionService, LinkAnalyticsService,
    LinkGenerationService, SearchService, GenerateLinkParams,

    // Jobs
    execute_content_ingestion,

    // Extension
    ContentExtension,

    // Default providers
    DefaultBrandingProvider, DefaultHomepagePrerenderer,
    DefaultListBrandingProvider, ListItemsCardRenderer,

    // Config
    ContentConfigValidated, ContentReady, ContentSourceConfigValidated,
    LoadStats, ParsedContent, ValidationResult,

    // Error
    ContentError, ContentResult,

    // Validation
    validate_content_metadata,
};
```

## Module Layout

| Module | Purpose |
|--------|---------|
| `models/` | `Content`, `ContentMetadata`, search request/response types, and campaign-link/UTM types, with builders. |
| `repository/` | Compile-time-verified persistence for `content/`, `link/` (plus analytics), and full-text `search/`. |
| `services/` | `IngestionService` (directory scan and ingest), `SearchService`, `LinkAnalyticsService`, `LinkGenerationService`, and `DefaultContentProvider`. |
| `config/` | Validated content-source configuration (`ContentConfigValidated`, `ContentReady`). |
| `jobs/` | `execute_content_ingestion` scheduled ingestion entrypoint. |
| (crate root) | Branding, homepage prerender, and list-rendering default providers. |

## Modules

### config/

Content source configuration validation and caching. `ContentConfigValidated` validates YAML configuration, `ContentReady` loads and caches parsed content for fast access.

### jobs/

Background jobs for content processing. `execute_content_ingestion` scans configured directories and syncs markdown content to the database, registered via `ContentExtension`.

### models/

Domain types for content, links, and search. Builder pattern used for complex parameter types (`CreateContentParams`, `TrackClickParams`).

### repository/

Database access layer using SQLX macros for compile-time SQL verification. Repositories handle data persistence with no business logic. Split into `queries.rs` (reads) and `mutations.rs` (writes) for clarity.

### services/

Business logic layer. Services coordinate repositories and implement domain operations:

- `DefaultContentProvider`: Implements the `ContentProvider` trait for downstream consumers
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
- `systemprompt-cloud` - Cloud integration types
- `systemprompt-extension` - Extension framework

## Traits Implemented

- `ContentProvider` (systemprompt-traits) - `DefaultContentProvider`
- `Extension` (systemprompt-extension) - `ContentExtension` (registers schema + ingestion job)
- `ContentRouting` (systemprompt-models) - `ContentConfigValidated`, `ContentReady`

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-content)** · **[docs.rs](https://docs.rs/systemprompt-content)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Domain layer · Own how your organization uses AI.</sub>

</div>
