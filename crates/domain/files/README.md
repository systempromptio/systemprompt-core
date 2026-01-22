# systemprompt-files

File management module for systemprompt.io.

[![Crates.io](https://img.shields.io/crates/v/systemprompt-files.svg)](https://crates.io/crates/systemprompt-files)
[![Documentation](https://docs.rs/systemprompt-files/badge.svg)](https://docs.rs/systemprompt-files)
[![License: FSL-1.1-ALv2](https://img.shields.io/badge/License-FSL--1.1--ALv2-blue.svg)](LICENSE)

## Overview

**Part of the Domain layer in the systemprompt.io architecture.**

This crate provides file storage, metadata management, and content-file linking capabilities. It handles file uploads with validation, AI-generated image tracking, and content associations.

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

## Configuration

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

## Dependencies

| Crate | Purpose |
|-------|---------|
| systemprompt-database | DbPool for database connections |
| systemprompt-identifiers | FileId, UserId, ContentId, SessionId, TraceId, ContextId |
| systemprompt-traits | Job trait for background jobs |
| systemprompt-models | AppPaths, ProfileBootstrap |
| systemprompt-cloud | Storage path constants |

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
systemprompt-files = "0.0.1"
```

## License

FSL-1.1-ALv2 - See [LICENSE](../../LICENSE) for details.
