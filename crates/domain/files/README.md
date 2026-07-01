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
systemprompt-files = "0.18.0"
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
├── lib.rs                    Public API exports and crate docs
├── error.rs                  FilesError, FilesResult
├── extension.rs              FilesExtension with schema registration
│
├── config/
│   ├── mod.rs                FilesConfig surface and YAML loading
│   ├── types.rs              FileUploadConfig, AllowedFileTypes, FilePersistenceMode
│   └── validator.rs          FilesConfigValidator for profile-driven settings
│
├── jobs/
│   ├── mod.rs                Job registration entry point
│   └── file_ingestion.rs     FileIngestionJob scanning storage for images
│
├── models/
│   ├── mod.rs                Re-exports for File, metadata, content_file
│   ├── file.rs               File entity with typed identifiers
│   ├── content_file.rs       Junction model and FileRole enum
│   ├── metadata.rs           FileMetadata with type-specific variants
│   └── image_metadata.rs     ImageMetadata, ImageGenerationInfo
│
├── repository/
│   ├── mod.rs                Repository re-exports
│   ├── file/
│   │   ├── mod.rs            FileRepository CRUD operations
│   │   ├── request.rs        InsertFileRequest builder
│   │   └── stats.rs          FileStats aggregation queries
│   ├── content/mod.rs        Content linking: link, unlink, featured
│   └── ai/mod.rs             AI image queries by user and tenant
│
└── services/
    ├── mod.rs                Service re-exports
    ├── ai_provider.rs        FilesAiPersistenceProvider implementation
    ├── providers.rs          Provider trait wiring
    └── upload/
        ├── mod.rs            Upload module entry point
        ├── service.rs        FileUploadService with storage and persistence
        ├── request.rs        FileUploadRequest, FileUploadRequestBuilder, UploadedFile
        ├── validator.rs      FileValidator, FileCategory, MIME enforcement
        └── error.rs          FileUploadError, FileValidationError
```

## Modules

| Module | Purpose |
|--------|---------|
| `config` | Profile-driven configuration loading, validation, and persistence modes |
| `error` | `FilesError` and `FilesResult` shared across the crate |
| `extension` | `FilesExtension` registering schemas and jobs via the extension framework |
| `jobs` | Background file ingestion with inventory registration |
| `models` | Data structures: `File`, `ContentFile`, `FileRole`, metadata variants |
| `repository` | Database access layer using compile-time verified `sqlx` macros |
| `services` | Upload, validation, and AI-persistence service wrappers |

## Public Types

| Type | Description |
|------|-------------|
| `File` | File entity with path, URL, metadata, and typed identifiers |
| `ContentFile` | Links a file to content with role and display order |
| `FileRole` | Featured, Attachment, Inline, OgImage, Thumbnail |
| `FileMetadata` | Container for checksums and type-specific metadata |
| `TypeSpecificMetadata` | Enum variant carrying image, audio, document, or video metadata |
| `ImageMetadata` / `ImageGenerationInfo` | Image dimensions, alt text, and AI generation provenance |
| `AudioMetadata` / `DocumentMetadata` / `VideoMetadata` | Type-specific metadata records |
| `FileChecksums` | SHA-256 / content checksum container |
| `FileUploadRequest` / `FileUploadRequestBuilder` | Builder for upload operations |
| `UploadedFile` | Result returned by `FileUploadService` |
| `FileCategory` / `FileValidator` | Upload validation surface |
| `FilesConfig` / `FilesConfigYaml` / `FilesConfigValidator` | Configuration types and validation |
| `FileUploadConfig` / `AllowedFileTypes` / `FilePersistenceMode` | Upload policy configuration |
| `InsertFileRequest` / `FileRepository` / `FileStats` | Repository surface |
| `FilesAiPersistenceProvider` | Provider implementation for AI-generated image persistence |
| `FilesExtension` | Extension entry point registered via `inventory` |
| `FileIngestionJob` | Scheduled job that reconciles storage with database rows |
| `FilesError` / `FilesResult` | Crate-level error type and result alias |

## Jobs

| Job | Description |
|-----|-------------|
| `FileIngestionJob` | Scans the configured storage root and reconciles on-disk image files with database rows |

## Schemas

| File | Purpose |
|------|---------|
| `schema/files.sql` | Core `files` table definition |
| `schema/content_files.sql` | Junction table linking files to content |
| `schema/ai_image_analytics.sql` | View/table for AI-generated image analytics |

## Dependencies

| Crate | Purpose |
|-------|---------|
| `systemprompt-database` | `DbPool` and `sqlx` query macros |
| `systemprompt-identifiers` | `FileId`, `UserId`, `ContentId`, `SessionId`, `TraceId`, `ContextId` |
| `systemprompt-traits` | `Job` trait for background jobs |
| `systemprompt-models` | `AppPaths`, `ProfileBootstrap`, shared models |
| `systemprompt-cloud` | Storage path constants and cloud integration |
| `systemprompt-config` | Profile and YAML configuration loading |
| `systemprompt-extension` | Extension trait and registration macros |
| `systemprompt-provider-contracts` | Provider trait contracts implemented by this crate |
| `systemprompt-logging` | Structured `tracing` integration |

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-files)** · **[docs.rs](https://docs.rs/systemprompt-files)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Domain layer · Own how your organization uses AI.</sub>

</div>
