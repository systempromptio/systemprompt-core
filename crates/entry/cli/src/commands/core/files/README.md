# Files CLI Commands

This document provides complete documentation for AI agents to use the files CLI commands. All commands support non-interactive mode for automation.

---

## Prerequisites

```bash
export SYSTEMPROMPT_PROFILE=/var/www/html/tyingshoelaces/.systemprompt/profiles/local/profile.yaml
cd /var/www/html/systemprompt-core
cargo build --package systemprompt-cli

alias sp="./target/debug/systemprompt --non-interactive"
```

---

## Command Reference

| Command | Description | Artifact Type | Requires Services |
|---------|-------------|---------------|-------------------|
| `files list` | List files with pagination | `Table` | No (DB only) |
| `files show <id>` | Show detailed file information | `Card` | No (DB only) |
| `files upload <path>` | Upload a file | `Card` | No (DB only) |
| `files delete <id>` | Delete a file | `Card` | No (DB only) |
| `files validate <path>` | Validate a file before upload | `Card` | No |
| `files config` | Show file upload configuration | `Card` | No |
| `files search <query>` | Search files by path pattern | `Table` | No (DB only) |
| `files stats` | Show file storage statistics | `Card` | No (DB only) |
| `files content list` | List content-file links | `Table` | No (DB only) |
| `files content link` | Link file to content | `Card` | No (DB only) |
| `files content unlink` | Unlink file from content | `Card` | No (DB only) |
| `files content featured` | Get/set featured image | `Card` | No (DB only) |
| `files ai list` | List AI-generated images | `Table` | No (DB only) |
| `files ai count` | Count AI-generated images | `Card` | No (DB only) |

---

## Core Commands

### files list

List all files with pagination and filtering.

```bash
sp files list
sp --json files list
sp files list --limit 50 --offset 0
sp files list --user user_abc123
sp files list --mime "image/*"
sp files list --mime "application/pdf"
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--limit` | `20` | Maximum number of results |
| `--offset` | `0` | Number of results to skip |
| `--user` | None | Filter by user ID |
| `--mime` | None | Filter by MIME type pattern (e.g., `image/*`) |

**MIME Pattern Matching:**
- Exact match: `image/png` matches only PNG files
- Wildcard: `image/*` matches all image types

**Output Structure:**
```json
{
  "files": [
    {
      "id": "b75940ac-c50f-4d46-9fdd-ebb4970b2a7d",
      "path": "/storage/files/uploads/contexts/.../image.png",
      "public_url": "/files/files/uploads/contexts/.../image.png",
      "mime_type": "image/png",
      "size_bytes": 102400,
      "ai_content": false,
      "created_at": "2024-01-15T10:30:00Z"
    }
  ],
  "total": 1,
  "limit": 20,
  "offset": 0
}
```

**Artifact Type:** `Table`
**Columns:** `id`, `path`, `mime_type`, `size_bytes`, `ai_content`, `created_at`

---

### files show

Display detailed information for a specific file.

```bash
sp files show <file-id>
sp --json files show b75940ac-c50f-4d46-9fdd-ebb4970b2a7d
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<id>` | Yes | File ID (UUID format) |

**Output Structure:**
```json
{
  "id": "b75940ac-c50f-4d46-9fdd-ebb4970b2a7d",
  "path": "/storage/files/uploads/contexts/.../image.png",
  "public_url": "/files/files/uploads/contexts/.../image.png",
  "mime_type": "image/png",
  "size_bytes": 102400,
  "user_id": "user_abc123",
  "context_id": "ctx_xyz789",
  "ai_content": false,
  "metadata": {},
  "created_at": "2024-01-15T10:30:00Z",
  "updated_at": "2024-01-15T10:30:00Z"
}
```

**Artifact Type:** `Card`

---

### files upload

Upload a file from the local filesystem.

```bash
sp files upload <path> --context <context-id>
sp files upload ./image.png --context ctx_abc123
sp files upload ./document.pdf --context ctx_abc123 --user user_xyz
sp files upload ./generated.png --context ctx_abc123 --ai
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<path>` | Yes | Path to file to upload |
| `--context` | Yes | Context ID (required) |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--user` | None | User ID |
| `--session` | None | Session ID |
| `--ai` | `false` | Mark as AI-generated content |

**Supported File Types:**
- Images: `jpg`, `jpeg`, `png`, `gif`, `webp`, `svg`, `bmp`, `tiff`, `ico`
- Documents: `pdf`, `doc`, `docx`, `xls`, `xlsx`, `ppt`, `pptx`
- Text: `txt`, `csv`, `md`, `html`, `json`, `xml`, `rtf`
- Audio: `mp3`, `wav`, `ogg`, `aac`, `flac`, `m4a`
- Video: `mp4`, `webm`, `mov`, `avi`, `mkv`

**Output Structure:**
```json
{
  "file_id": "b75940ac-c50f-4d46-9fdd-ebb4970b2a7d",
  "path": "/storage/files/uploads/contexts/.../image.png",
  "public_url": "/files/files/uploads/contexts/.../image.png",
  "size_bytes": 102400,
  "mime_type": "image/png",
  "checksum_sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
}
```

**Artifact Type:** `Card`

---

### files delete

Delete a file permanently.

```bash
sp files delete <file-id> --yes
sp files delete b75940ac-c50f-4d46-9fdd-ebb4970b2a7d --yes
sp files delete b75940ac-c50f-4d46-9fdd-ebb4970b2a7d --dry-run --yes
```

**Required Flags (non-interactive):**
| Flag | Required | Description |
|------|----------|-------------|
| `<id>` | Yes | File ID (UUID format) |
| `--yes` / `-y` | Yes | Skip confirmation (REQUIRED in non-interactive mode) |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--dry-run` | `false` | Preview deletion without executing |

**Output Structure:**
```json
{
  "file_id": "b75940ac-c50f-4d46-9fdd-ebb4970b2a7d",
  "message": "File '/path/to/file.png' deleted successfully"
}
```

**Dry-Run Output:**
```json
{
  "file_id": "b75940ac-c50f-4d46-9fdd-ebb4970b2a7d",
  "message": "[DRY-RUN] Would delete file '/path/to/file.png' (b75940ac-c50f-4d46-9fdd-ebb4970b2a7d)"
}
```

**Artifact Type:** `Card`

---

### files validate

Validate a file before upload.

```bash
sp files validate <path>
sp --json files validate ./image.png
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<path>` | Yes | Path to file to validate |

**Validation Checks:**
- File exists
- File size within limits
- MIME type is allowed
- File extension matches MIME type

**Output Structure:**
```json
{
  "valid": true,
  "mime_type": "image/png",
  "category": "images",
  "size_bytes": 102400,
  "max_size_bytes": 10485760,
  "errors": []
}
```

**Artifact Type:** `Card`

---

### files config

Show file upload configuration.

```bash
sp files config
sp --json files config
```

**Output Structure:**
```json
{
  "uploads_enabled": true,
  "max_file_size_bytes": 10485760,
  "persistence_mode": "local",
  "storage_root": "/var/www/html/tyingshoelaces/storage/files",
  "url_prefix": "/files",
  "allowed_types": {
    "images": true,
    "documents": true,
    "audio": true,
    "video": true
  },
  "storage_paths": {
    "uploads": "uploads",
    "images": "images",
    "documents": "documents",
    "audio": "audio",
    "video": "video"
  }
}
```

**Artifact Type:** `Card`

---

### files search

Search files by path pattern.

```bash
sp files search <query>
sp --json files search uploads
sp files search logo --limit 10
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<query>` | Yes | Search query (matches file paths) |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--limit` | `20` | Maximum number of results |

**Output Structure:**
```json
{
  "files": [
    {
      "id": "b75940ac-c50f-4d46-9fdd-ebb4970b2a7d",
      "path": "/storage/files/uploads/contexts/.../image.png",
      "public_url": "/files/files/uploads/contexts/.../image.png",
      "mime_type": "image/png",
      "size_bytes": 156108,
      "ai_content": false,
      "created_at": "2026-01-09T14:26:01.616511Z"
    }
  ],
  "query": "uploads",
  "total": 4
}
```

**Artifact Type:** `Table`
**Columns:** `id`, `path`, `mime_type`, `size_bytes`, `created_at`

---

### files stats

Show file storage statistics.

```bash
sp files stats
sp --json files stats
```

**Output Structure:**
```json
{
  "total_files": 4,
  "total_size_bytes": 363938,
  "ai_images_count": 0,
  "by_category": {
    "images": {
      "count": 2,
      "size_bytes": 356490
    },
    "documents": {
      "count": 2,
      "size_bytes": 7448
    },
    "audio": {
      "count": 0,
      "size_bytes": 0
    },
    "video": {
      "count": 0,
      "size_bytes": 0
    },
    "other": {
      "count": 0,
      "size_bytes": 0
    }
  }
}
```

**Artifact Type:** `Card`

---

## Content-File Link Commands

### files content list

List content-file links. Use `--content` to list files attached to content, or `--file` to list content linked to a file.

```bash
sp files content list --content content_abc123
sp files content list --file b75940ac-c50f-4d46-9fdd-ebb4970b2a7d
sp --json files content list --content content_abc123
```

**Required Flags (one of):**
| Flag | Description |
|------|-------------|
| `--content` | List files attached to this content ID |
| `--file` | List content linked to this file ID (reverse lookup) |

**Output Structure (with --content):**
```json
{
  "content_id": "content_abc123",
  "files": [
    {
      "file_id": "b75940ac-c50f-4d46-9fdd-ebb4970b2a7d",
      "path": "/storage/files/uploads/.../image.png",
      "mime_type": "image/png",
      "role": "featured",
      "display_order": 0
    }
  ]
}
```

**Output Structure (with --file):**
```json
{
  "file_id": "b75940ac-c50f-4d46-9fdd-ebb4970b2a7d",
  "links": [
    {
      "content_id": "content_abc123",
      "role": "featured",
      "display_order": 0,
      "created_at": "2024-01-15T10:30:00Z"
    }
  ]
}
```

**Artifact Type:** `Table`

---

### files content link

Link a file to content with a specific role.

```bash
sp files content link <file-id> --content <content-id> --role <role>
sp files content link b75940ac-c50f-4d46-9fdd-ebb4970b2a7d --content content_abc123 --role attachment
sp files content link b75940ac-c50f-4d46-9fdd-ebb4970b2a7d --content content_abc123 --role featured --order 0
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<file-id>` | Yes | File ID (UUID format) |
| `--content` | Yes | Content ID |
| `--role` | Yes | File role |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--order` | `0` | Display order |

**Available Roles:**
- `featured` - Featured/hero image
- `attachment` - General attachment
- `inline` - Inline content image
- `og-image` - Open Graph image
- `thumbnail` - Thumbnail image

**Output Structure:**
```json
{
  "file_id": "b75940ac-c50f-4d46-9fdd-ebb4970b2a7d",
  "content_id": "content_abc123",
  "role": "attachment",
  "message": "File linked to content successfully"
}
```

**Artifact Type:** `Card`

---

### files content unlink

Unlink a file from content.

```bash
sp files content unlink <file-id> --content <content-id> --yes
sp files content unlink b75940ac-c50f-4d46-9fdd-ebb4970b2a7d --content content_abc123 --yes
sp files content unlink b75940ac-c50f-4d46-9fdd-ebb4970b2a7d --content content_abc123 --dry-run --yes
```

**Required Flags (non-interactive):**
| Flag | Required | Description |
|------|----------|-------------|
| `<file-id>` | Yes | File ID (UUID format) |
| `--content` | Yes | Content ID |
| `--yes` / `-y` | Yes | Skip confirmation (REQUIRED in non-interactive mode) |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--dry-run` | `false` | Preview unlink without executing |

**Output Structure:**
```json
{
  "file_id": "b75940ac-c50f-4d46-9fdd-ebb4970b2a7d",
  "content_id": "content_abc123",
  "message": "File unlinked from content successfully"
}
```

**Artifact Type:** `Card`

---

### files content featured

Get or set the featured image for content.

```bash
sp files content featured <content-id>
sp files content featured content_abc123 --set b75940ac-c50f-4d46-9fdd-ebb4970b2a7d
sp --json files content featured content_abc123
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<content-id>` | Yes | Content ID |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--set` | None | File ID to set as featured image |

**Output Structure (get):**
```json
{
  "content_id": "content_abc123",
  "file": {
    "id": "b75940ac-c50f-4d46-9fdd-ebb4970b2a7d",
    "path": "/storage/files/.../image.png",
    "public_url": "/files/.../image.png",
    "mime_type": "image/png",
    "size_bytes": 156108,
    "ai_content": false,
    "created_at": "2024-01-15T10:30:00Z"
  },
  "message": "Featured image: /storage/files/.../image.png"
}
```

**Output Structure (set):**
```json
{
  "content_id": "content_abc123",
  "file": null,
  "message": "Featured image set successfully"
}
```

**Artifact Type:** `Card`

---

## AI-Generated Images Commands

### files ai list

List AI-generated images.

```bash
sp files ai list
sp --json files ai list
sp files ai list --limit 50
sp files ai list --user user_abc123
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--limit` | `20` | Maximum number of results |
| `--offset` | `0` | Number of results to skip |
| `--user` | None | Filter by user ID |

**Output Structure:**
```json
{
  "files": [
    {
      "id": "b75940ac-c50f-4d46-9fdd-ebb4970b2a7d",
      "path": "/storage/files/ai-images/.../generated.png",
      "public_url": "/files/ai-images/.../generated.png",
      "mime_type": "image/png",
      "size_bytes": 204800,
      "ai_content": true,
      "created_at": "2024-01-15T10:30:00Z"
    }
  ],
  "total": 1,
  "limit": 20,
  "offset": 0
}
```

**Artifact Type:** `Table`
**Columns:** `id`, `path`, `size_bytes`, `created_at`

---

### files ai count

Count AI-generated images. The `--user` flag is optional; when omitted, counts all AI images.

```bash
sp files ai count
sp --json files ai count
sp files ai count --user user_abc123
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--user` | None | Filter by user ID (optional, counts all if not specified) |

**Output Structure:**
```json
{
  "count": 150,
  "user_id": null
}
```

**Output Structure (with --user):**
```json
{
  "count": 25,
  "user_id": "user_abc123"
}
```

**Artifact Type:** `Card`

---

## Complete File Management Flow Example

This flow demonstrates the full file management lifecycle:

```bash
# Phase 1: Check configuration and storage stats
sp --json files config
sp --json files stats

# Phase 2: Validate file before upload
sp --json files validate ./image.png

# Phase 3: Upload file
sp --json files upload ./image.png --context ctx_abc123

# Phase 4: Verify upload
sp --json files list --limit 5
sp --json files show b75940ac-c50f-4d46-9fdd-ebb4970b2a7d

# Phase 5: Search for files
sp --json files search uploads

# Phase 6: Link file to content
sp files content link b75940ac-c50f-4d46-9fdd-ebb4970b2a7d --content content_xyz --role attachment

# Phase 7: List content-file links (both directions)
sp --json files content list --content content_xyz
sp --json files content list --file b75940ac-c50f-4d46-9fdd-ebb4970b2a7d

# Phase 8: Set featured image
sp files content featured content_xyz --set b75940ac-c50f-4d46-9fdd-ebb4970b2a7d

# Phase 9: Cleanup with dry-run preview
sp files content unlink b75940ac-c50f-4d46-9fdd-ebb4970b2a7d --content content_xyz --dry-run --yes
sp files delete b75940ac-c50f-4d46-9fdd-ebb4970b2a7d --dry-run --yes

# Phase 10: Actual cleanup
sp files content unlink b75940ac-c50f-4d46-9fdd-ebb4970b2a7d --content content_xyz --yes
sp files delete b75940ac-c50f-4d46-9fdd-ebb4970b2a7d --yes

# Phase 11: Verify deletion
sp --json files stats
```

---

## AI Image Workflow Example

```bash
# Phase 1: Count all AI-generated images
sp --json files ai count

# Phase 2: List AI-generated images
sp --json files ai list --limit 10

# Phase 3: Count AI images for specific user
sp --json files ai count --user user_abc123

# Phase 4: Upload new AI-generated image
sp files upload ./generated.png --context ctx_abc123 --ai

# Phase 5: Verify AI flag
sp --json files show b75940ac-c50f-4d46-9fdd-ebb4970b2a7d
# Should show "ai_content": true
```

---

## Error Handling

### Missing Required Flags

```bash
sp files upload ./image.png
# Error: --context is required

sp files delete b75940ac-c50f-4d46-9fdd-ebb4970b2a7d
# Error: --yes is required to delete files in non-interactive mode

sp files content unlink b75940ac-c50f-4d46-9fdd-ebb4970b2a7d --content content_abc
# Error: --yes is required to unlink files in non-interactive mode

sp files content list
# Error: Either --content or --file is required
```

### Invalid UUID Format

```bash
sp files show invalid-id
# Error: Invalid file ID format. Expected UUID like 'b75940ac-c50f-4d46-9fdd-ebb4970b2a7d', got 'invalid-id'

sp files delete not-a-uuid --yes
# Error: Invalid file ID format. Expected UUID like 'b75940ac-c50f-4d46-9fdd-ebb4970b2a7d', got 'not-a-uuid'
```

### File Not Found

```bash
sp files upload ./nonexistent.png --context ctx_abc
# Error: File not found: ./nonexistent.png

sp files show 00000000-0000-0000-0000-000000000000
# Error: File not found: 00000000-0000-0000-0000-000000000000
```

### Validation Errors

```bash
sp files upload ./toolarge.zip --context ctx_abc
# Error: File size 52428800 exceeds maximum 10485760 bytes

sp files upload ./script.exe --context ctx_abc
# Error: MIME type 'application/x-msdownload' not allowed
```

### Upload Disabled

```bash
sp files upload ./image.png --context ctx_abc
# Error: File uploads are disabled in configuration
```

---

## JSON Output

All commands support `--json` flag for structured output:

```bash
# Verify JSON is valid
sp --json files list | jq .

# Extract specific fields
sp --json files list | jq '.data.files[].id'
sp --json files show b75940ac-c50f-4d46-9fdd-ebb4970b2a7d | jq '.data.public_url'
sp --json files config | jq '.data.max_file_size_bytes'
sp --json files stats | jq '.data.by_category.images'

# Filter by criteria
sp --json files list | jq '.data.files[] | select(.ai_content == true)'
sp --json files list | jq '.data.files[] | select(.size_bytes > 100000)'
sp --json files search uploads | jq '.data.files[] | select(.mime_type | startswith("image/"))'
```

---

## Compliance Checklist

- [x] All `execute` functions accept `config: &CliConfig`
- [x] All commands return `CommandResult<T>` with proper artifact type
- [x] `delete` commands require `--yes` / `-y` flag in non-interactive mode
- [x] `unlink` commands require `--yes` / `-y` flag in non-interactive mode
- [x] `--dry-run` support for destructive operations
- [x] All output types derive `Serialize`, `Deserialize`, `JsonSchema`
- [x] No `println!` / `eprintln!` - uses `CliService`
- [x] No `unwrap()` / `expect()` - uses `?` with `.context()`
- [x] JSON output supported via `--json` flag
- [x] Proper error messages for missing required flags
- [x] Proper error messages for invalid UUID format
