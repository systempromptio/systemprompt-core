# Files CLI Domain Plan

## Overview

Add comprehensive file management CLI commands exposing the existing `FileService`, `FileUploadService`, `ContentService` (file-content linking), and `AiService` (AI-generated images) functionality.

## Proposed Structure

```
files
├── list [--limit N] [--offset N] [--user USER_ID] [--mime MIME_TYPE]
├── show <FILE_ID|PATH>
├── upload <FILE_PATH> --context CONTEXT_ID [--user USER_ID] [--ai]
├── delete <FILE_ID> --yes
├── validate <FILE_PATH>
├── config
│
├── content
│   ├── link <FILE_ID> --content CONTENT_ID --role ROLE [--order N]
│   ├── unlink <FILE_ID> --content CONTENT_ID
│   ├── list <CONTENT_ID>
│   └── featured <CONTENT_ID> [--set FILE_ID]
│
└── ai
    ├── list [--limit N] [--offset N] [--user USER_ID]
    └── count [--user USER_ID]
```

## File Structure

```
crates/entry/cli/src/commands/files/
├── mod.rs              # FilesCommands enum + dispatch
├── types.rs            # Output types (FileListOutput, FileDetailOutput, etc.)
├── list.rs             # files list
├── show.rs             # files show
├── upload.rs           # files upload
├── delete.rs           # files delete
├── validate.rs         # files validate
├── config.rs           # files config
├── content/
│   ├── mod.rs          # ContentCommands enum + dispatch
│   ├── link.rs         # files content link
│   ├── unlink.rs       # files content unlink
│   ├── list.rs         # files content list
│   └── featured.rs     # files content featured
└── ai/
    ├── mod.rs          # AiCommands enum + dispatch
    ├── list.rs         # files ai list
    └── count.rs        # files ai count
```

## Command Details

### `files list`

List files with pagination and filtering.

```bash
files list                              # List first 20 files
files list --limit 50 --offset 100      # Paginate
files list --user user_abc123           # Filter by user
files list --mime "image/*"             # Filter by MIME type
files list --json                       # JSON output
```

**Args:**
```rust
#[derive(Args)]
pub struct ListArgs {
    #[arg(long, default_value = "20")]
    pub limit: i64,

    #[arg(long, default_value = "0")]
    pub offset: i64,

    #[arg(long, help = "Filter by user ID")]
    pub user: Option<String>,

    #[arg(long, help = "Filter by MIME type pattern")]
    pub mime: Option<String>,
}
```

**Output Type:**
```rust
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct FileListOutput {
    pub files: Vec<FileSummary>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct FileSummary {
    pub id: FileId,
    pub path: String,
    pub public_url: String,
    pub mime_type: String,
    pub size_bytes: Option<i64>,
    pub ai_content: bool,
    pub created_at: DateTime<Utc>,
}
```

**Service Call:** `FileService::list_all()` or `FileService::list_by_user()`

### `files show`

Show detailed file information.

```bash
files show file_abc123                  # By ID
files show /uploads/image.png           # By path
```

**Args:**
```rust
#[derive(Args)]
pub struct ShowArgs {
    #[arg(help = "File ID or path")]
    pub identifier: String,
}
```

**Output Type:**
```rust
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct FileDetailOutput {
    pub id: FileId,
    pub path: String,
    pub public_url: String,
    pub mime_type: String,
    pub size_bytes: Option<i64>,
    pub ai_content: bool,
    pub user_id: Option<UserId>,
    pub session_id: Option<SessionId>,
    pub trace_id: Option<TraceId>,
    pub context_id: Option<ContextId>,
    pub metadata: FileMetadataOutput,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct FileMetadataOutput {
    pub checksums: Option<ChecksumsOutput>,
    pub image: Option<ImageMetadataOutput>,
    pub document: Option<DocumentMetadataOutput>,
    pub audio: Option<AudioMetadataOutput>,
    pub video: Option<VideoMetadataOutput>,
}
```

**Service Call:** `FileService::find_by_id()` or `FileService::find_by_path()`

### `files upload`

Upload a file from the local filesystem.

```bash
files upload ./image.png --context ctx_abc123
files upload ./document.pdf --context ctx_abc123 --user user_abc123
files upload ./generated.png --context ctx_abc123 --ai
```

**Args:**
```rust
#[derive(Args)]
pub struct UploadArgs {
    #[arg(help = "Path to file to upload")]
    pub file_path: PathBuf,

    #[arg(long, help = "Context ID (required)")]
    pub context: Option<String>,

    #[arg(long, help = "User ID")]
    pub user: Option<String>,

    #[arg(long, help = "Session ID")]
    pub session: Option<String>,

    #[arg(long, help = "Mark as AI-generated content")]
    pub ai: bool,
}
```

**Output Type:**
```rust
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct FileUploadOutput {
    pub file_id: FileId,
    pub path: String,
    pub public_url: String,
    pub size_bytes: i64,
    pub mime_type: String,
    pub checksum_sha256: String,
}
```

**Service Call:** `FileUploadService::upload_file()`

**Implementation Notes:**
- Read file from filesystem
- Base64 encode content
- Detect MIME type from extension or content
- Call upload service
- Display result with public URL

### `files delete`

Delete a file permanently.

```bash
files delete file_abc123 --yes
```

**Args:**
```rust
#[derive(Args)]
pub struct DeleteArgs {
    #[arg(help = "File ID")]
    pub file_id: String,

    #[arg(short = 'y', long, help = "Skip confirmation")]
    pub yes: bool,
}
```

**Service Call:** `FileService::delete()`

### `files validate`

Validate a file before upload.

```bash
files validate ./image.png              # Check if file is valid for upload
files validate ./script.exe             # Should fail - blocked type
```

**Args:**
```rust
#[derive(Args)]
pub struct ValidateArgs {
    #[arg(help = "Path to file to validate")]
    pub file_path: PathBuf,
}
```

**Output Type:**
```rust
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct FileValidationOutput {
    pub valid: bool,
    pub mime_type: String,
    pub category: String,
    pub size_bytes: u64,
    pub max_size_bytes: u64,
    pub errors: Vec<String>,
}
```

**Service Call:** `FileValidator::validate()`

### `files config`

Show file upload configuration.

```bash
files config                            # Show current config
files config --json
```

**Output Type:**
```rust
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct FileConfigOutput {
    pub uploads_enabled: bool,
    pub max_file_size_bytes: u64,
    pub persistence_mode: String,
    pub storage_root: String,
    pub url_prefix: String,
    pub allowed_types: AllowedTypesOutput,
    pub storage_paths: StoragePathsOutput,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct AllowedTypesOutput {
    pub images: bool,
    pub documents: bool,
    pub audio: bool,
    pub video: bool,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct StoragePathsOutput {
    pub uploads: String,
    pub images: String,
    pub documents: String,
    pub audio: String,
    pub video: String,
}
```

**Service Call:** `FilesConfig::get()`

### `files content link`

Link a file to content with a specific role.

```bash
files content link file_abc123 --content content_xyz --role featured
files content link file_abc123 --content content_xyz --role attachment --order 1
```

**Args:**
```rust
#[derive(Args)]
pub struct LinkArgs {
    #[arg(help = "File ID")]
    pub file_id: String,

    #[arg(long, help = "Content ID")]
    pub content: Option<String>,

    #[arg(long, value_enum, help = "File role")]
    pub role: Option<FileRole>,

    #[arg(long, default_value = "0", help = "Display order")]
    pub order: i32,
}
```

**Service Call:** `ContentService::link_to_content()`

### `files content unlink`

Unlink a file from content.

```bash
files content unlink file_abc123 --content content_xyz
```

**Service Call:** `ContentService::unlink_from_content()`

### `files content list`

List files attached to content.

```bash
files content list content_xyz
```

**Output Type:**
```rust
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ContentFilesOutput {
    pub content_id: ContentId,
    pub files: Vec<ContentFileRow>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ContentFileRow {
    pub file_id: FileId,
    pub path: String,
    pub mime_type: String,
    pub role: String,
    pub display_order: i32,
}
```

**Service Call:** `ContentService::list_files_by_content()`

### `files content featured`

Get or set the featured image for content.

```bash
files content featured content_xyz              # Get featured image
files content featured content_xyz --set file_abc123  # Set featured image
```

**Args:**
```rust
#[derive(Args)]
pub struct FeaturedArgs {
    #[arg(help = "Content ID")]
    pub content_id: String,

    #[arg(long, help = "Set featured image")]
    pub set: Option<String>,
}
```

**Service Calls:**
- `ContentService::find_featured_image()` - Get
- `ContentService::set_featured()` - Set

### `files ai list`

List AI-generated images.

```bash
files ai list                           # All AI images
files ai list --user user_abc123        # User's AI images
files ai list --limit 50 --offset 100   # Paginate
```

**Args:**
```rust
#[derive(Args)]
pub struct AiListArgs {
    #[arg(long, default_value = "20")]
    pub limit: i64,

    #[arg(long, default_value = "0")]
    pub offset: i64,

    #[arg(long, help = "Filter by user ID")]
    pub user: Option<String>,
}
```

**Service Calls:**
- `AiService::list_ai_images()`
- `AiService::list_ai_images_by_user()`

### `files ai count`

Count AI-generated images.

```bash
files ai count                          # Total count
files ai count --user user_abc123       # User's count
```

**Service Call:** `AiService::count_ai_images_by_user()`

## Dependencies

Add to `crates/entry/cli/Cargo.toml`:
```toml
systemprompt_core_files = { path = "../../domain/files" }
```

## Implementation Checklist

- [ ] Create `commands/files/mod.rs` with `FilesCommands` enum
- [ ] Create `commands/files/types.rs` with output types
- [ ] Implement `files list`
- [ ] Implement `files show`
- [ ] Implement `files upload`
- [ ] Implement `files delete`
- [ ] Implement `files validate`
- [ ] Implement `files config`
- [ ] Create `commands/files/content/mod.rs`
- [ ] Implement `files content link`
- [ ] Implement `files content unlink`
- [ ] Implement `files content list`
- [ ] Implement `files content featured`
- [ ] Create `commands/files/ai/mod.rs`
- [ ] Implement `files ai list`
- [ ] Implement `files ai count`
- [ ] Add `Files` variant to main `Commands` enum in `lib.rs`
- [ ] Update CLI README with files commands

## Verification

```bash
# List files
systemprompt files list
systemprompt files list --user user_123 --json

# Show file details
systemprompt files show file_abc123

# Validate file
systemprompt files validate ./test-image.png

# Upload file
systemprompt files upload ./test-image.png --context ctx_123

# Delete file
systemprompt files delete file_abc123 --yes

# Show config
systemprompt files config

# Content file operations
systemprompt files content list content_xyz
systemprompt files content link file_abc123 --content content_xyz --role featured
systemprompt files content featured content_xyz
systemprompt files content unlink file_abc123 --content content_xyz

# AI images
systemprompt files ai list
systemprompt files ai count --user user_123
```
