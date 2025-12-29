# systemprompt-core-files

File management module for SystemPrompt.

## Directories

| Directory | Purpose |
|-----------|---------|
| `schema/` | SQL table definitions |
| `src/jobs/` | Background job implementations |
| `src/models/` | Data structures and enums |
| `src/repository/` | Database access layer |
| `src/services/` | Business logic layer |

## Key Files

| File | Description |
|------|-------------|
| `src/lib.rs` | Public API exports |
| `src/jobs/file_ingestion.rs` | Scans storage for image files |
| `src/models/file.rs` | File entity with typed identifiers |
| `src/models/content_file.rs` | Junction record, FileRole enum |
| `src/models/metadata.rs` | FileMetadata, type-specific variants |
| `src/repository/file/mod.rs` | Core CRUD operations |
| `src/repository/content/mod.rs` | Content-file linking |
| `src/repository/ai/mod.rs` | AI image queries |
| `src/services/storage.rs` | LocalFileStorage (FileStorage trait) |
| `src/services/file/mod.rs` | FileService wrapper |
| `src/services/content/mod.rs` | ContentService wrapper |
| `src/services/ai/mod.rs` | AiService wrapper |
| `schema/files.sql` | Files table schema |
| `schema/content_files.sql` | Junction table schema |

## Public Types

| Type | Description |
|------|-------------|
| `File` | File entity with path, URL, metadata |
| `ContentFile` | Links file to content with role |
| `FileRole` | Featured, Attachment, Inline, OgImage, Thumbnail |
| `FileMetadata` | Container for checksums and type metadata |
| `ImageMetadata` | Dimensions, alt text, generation info |
| `InsertFileRequest` | Builder for file insertion |

## Services

| Service | Methods |
|---------|---------|
| `FileService` | insert, find_by_id, find_by_path, list_by_user, list_all, soft_delete, update_metadata |
| `ContentService` | link_to_content, unlink_from_content, list_files_by_content, find_featured_image, set_featured |
| `AiService` | list_ai_images, list_ai_images_by_user, count_ai_images_by_user |
| `LocalFileStorage` | store, retrieve, delete, metadata, exists (implements FileStorage trait) |

## Jobs

| Job | Schedule | Description |
|-----|----------|-------------|
| `FileIngestionJob` | Every 30 min | Scans storage directory for images, creates DB entries |

## Dependencies

| Crate | Purpose |
|-------|---------|
| systemprompt-core-database | DbPool |
| systemprompt-core-logging | Logging infrastructure |
| systemprompt-models | PathConfig |
| systemprompt-identifiers | FileId, UserId, ContentId, SessionId, TraceId |
| systemprompt-traits | Job, FileStorage traits |
