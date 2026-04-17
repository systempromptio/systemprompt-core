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

# systemprompt-files

<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/domain-files.svg">
    <source media="(prefers-color-scheme: light)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/light/domain-files.svg">
    <img alt="systemprompt-files terminal demo" src="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/domain-files.svg" width="100%">
  </picture>
</div>

[![Crates.io](https://img.shields.io/crates/v/systemprompt-files.svg?style=flat-square)](https://crates.io/crates/systemprompt-files)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-files?style=flat-square)](https://docs.rs/systemprompt-files)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

File storage, metadata, and access control for systemprompt.io AI governance infrastructure. Governed file operations for the MCP governance pipeline with upload validation, AI-generated image tracking, and content-file associations.

**Layer**: Domain — business-logic modules that implement systemprompt.io features. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Overview

**Infrastructure** · [Self-Hosted Deployment](https://systemprompt.io/features/self-hosted-ai-platform)

This crate provides file storage, metadata management, and content-file linking capabilities. It handles file uploads with validation, AI-generated image tracking, and content associations.

## Usage

```toml
[dependencies]
systemprompt-files = "0.2.1"
```

Configured via `files.yaml`:

```yaml
files:
  urlPrefix: "/files"
  upload:
    enabled: true
    maxFileSizeBytes: 52428800
    persistenceMode: context_scoped
    allowedTypes:
      images: true
      documents: true
      audio: true
      video: false
```

## File Structure

```
src/
├── lib.rs                    Public API exports
├── config.rs                 FilesConfig, FileUploadConfig, persistence modes
│
├── jobs/
│   └── file_ingestion.rs     Background job scanning storage for images
│
├── models/
│   ├── file.rs               File entity with typed identifiers
│   ├── content_file.rs       Junction table model, FileRole enum
│   ├── metadata.rs           FileMetadata with type-specific variants
│   └── image_metadata.rs     ImageMetadata, ImageGenerationInfo
│
├── repository/
│   ├── file/mod.rs           Core CRUD: insert, find, list, delete, stats
│   ├── content/mod.rs        Content linking: link, unlink, featured
│   └── ai/mod.rs             AI image queries: list, count by user
│
└── services/
    ├── file/mod.rs           FileService wrapper over repository
    ├── content/mod.rs        ContentService for file-content relations
    ├── ai/mod.rs             AiService for AI-generated images
    └── upload/
        ├── mod.rs            FileUploadService with storage logic
        └── validator.rs      MIME type validation, extension extraction
```

## Modules

| Module | Purpose |
|--------|---------|
| `config` | Configuration loading from YAML, validation, path resolution |
| `jobs` | Background file ingestion job with inventory registration |
| `models` | Data structures: File, ContentFile, FileRole, metadata types |
| `repository` | Database access layer with SQLX queries |
| `services` | Business logic wrappers providing clean API |

## Public Types

| Type | Description |
|------|-------------|
| `File` | File entity with path, URL, metadata, identifiers |
| `ContentFile` | Links file to content with role and display order |
| `FileRole` | Featured, Attachment, Inline, OgImage, Thumbnail |
| `FileMetadata` | Container for checksums and type-specific metadata |
| `ImageMetadata` | Dimensions, alt text, generation info |
| `FileUploadRequest` | Builder for file upload operations |
| `FilesConfig` | Runtime configuration for storage paths and URLs |

## Services

| Service | Methods |
|---------|---------|
| `FileService` | insert, find_by_id, find_by_path, list_by_user, list_all, delete, update_metadata, get_stats |
| `ContentService` | link_to_content, unlink_from_content, list_files_by_content, find_featured_image, set_featured |
| `AiService` | list_ai_images, list_ai_images_by_user, count_ai_images_by_user, count_ai_images |
| `FileUploadService` | upload_file with validation, storage, and database insertion |

## Jobs

| Job | Schedule | Description |
|-----|----------|-------------|
| `FileIngestionJob` | Every 30 min | Scans storage directory for images, creates DB entries |

## Dependencies

| Crate | Purpose |
|-------|---------|
| systemprompt-database | DbPool for database connections |
| systemprompt-identifiers | FileId, UserId, ContentId, SessionId, TraceId, ContextId |
| systemprompt-traits | Job trait for background jobs |
| systemprompt-models | AppPaths, ProfileBootstrap |
| systemprompt-cloud | Storage path constants |

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-files)** · **[docs.rs](https://docs.rs/systemprompt-files)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Domain layer · Own how your organization uses AI.</sub>

</div>
