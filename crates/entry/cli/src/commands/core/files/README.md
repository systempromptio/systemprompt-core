<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://systemprompt.io/files/images/logo.svg">
  <source media="(prefers-color-scheme: light)" srcset="https://systemprompt.io/files/images/logo-dark.svg">
  <img src="https://systemprompt.io/files/images/logo.svg" alt="systemprompt.io" width="180">
</picture>

### Production infrastructure for AI agents

[**Website**](https://systemprompt.io) · [**Documentation**](https://systemprompt.io/documentation/) · [**Guides**](https://systemprompt.io/guides) · [**Core**](https://github.com/systempromptio/systemprompt-core) · [**CLI Reference**](https://github.com/systempromptio/systemprompt-core/tree/main/crates/entry/cli) · [**Discord**](https://discord.gg/wkAbSuPWpr)

</div>

---


# Files CLI Commands

Every file an agent uploads, generates, or serves passes through your own storage and your own database, on infrastructure you own. This document is the complete reference for the files CLI. Every command supports non-interactive mode for automation.

Content-to-file linking (attaching a file to a content record, setting a featured image) lives under a separate command group, `core content files`, not here.

---

## Prerequisites

```bash
export SYSTEMPROMPT_PROFILE=~/.systemprompt/profiles/local/profile.yaml
cd /var/www/html/systemprompt-core
cargo build --package systemprompt-cli

alias sp="./target/debug/systemprompt --non-interactive"
```

---

## Command Reference

| Command | Description | Artifact Type | Requires Services |
|---------|-------------|---------------|-------------------|
| `core files list` | List files with pagination | `Table` | No (DB only) |
| `core files show <id>` | Show detailed file information | `Card` | No (DB only) |
| `core files upload <path>` | Upload a file | `Card` | No (DB only) |
| `core files delete <id>` | Delete a file | `Card` | No (DB only) |
| `core files validate <path>` | Validate a file before upload | `Card` | No |
| `core files config` | Show file upload configuration | `Card` | No |
| `core files search <query>` | Search files by path pattern | `Table` | No (DB only) |
| `core files stats` | Show file storage statistics | `Card` | No (DB only) |
| `core files ai list` | List AI-generated images | `Table` | No (DB only) |
| `core files ai show <id>` | Show AI-generated image details | `Card` | No (DB only) |
| `core files ai count` | Count AI-generated images | `Card` | No (DB only) |

---

## Core Commands

### files list

List all files with pagination and filtering.

```bash
sp core files list
sp --json core files list
sp core files list --limit 50 --offset 0
sp core files list --user user_abc123
sp core files list --mime "image/*"
sp core files list --mime "application/pdf"
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
sp core files show <file-id>
sp --json core files show b75940ac-c50f-4d46-9fdd-ebb4970b2a7d
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
sp core files upload <path> --context <context-id>
sp core files upload ./image.png --context ctx_abc123
sp core files upload ./document.pdf --context ctx_abc123 --user user_xyz
sp core files upload ./generated.png --context ctx_abc123 --ai
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
sp core files delete <file-id> --yes
sp core files delete b75940ac-c50f-4d46-9fdd-ebb4970b2a7d --yes
sp core files delete b75940ac-c50f-4d46-9fdd-ebb4970b2a7d --dry-run --yes
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
sp core files validate <path>
sp --json core files validate ./image.png
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
sp core files config
sp --json core files config
```

**Output Structure:**
```json
{
  "uploads_enabled": true,
  "max_file_size_bytes": 10485760,
  "persistence_mode": "local",
  "storage_root": "<your-project>/storage/files",
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
sp core files search <query>
sp --json core files search uploads
sp core files search logo --limit 10
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
sp core files stats
sp --json core files stats
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

## AI-Generated Images Commands

### files ai list

List AI-generated images.

```bash
sp core files ai list
sp --json core files ai list
sp core files ai list --limit 50
sp core files ai list --user user_abc123
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

### files ai show

Show details for a specific AI-generated image.

```bash
sp core files ai show <id>
sp --json core files ai show b75940ac-c50f-4d46-9fdd-ebb4970b2a7d
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<id>` | Yes | File ID (UUID format) |

**Output Structure:**
```json
{
  "id": "b75940ac-c50f-4d46-9fdd-ebb4970b2a7d",
  "path": "/storage/files/ai-images/.../generated.png",
  "public_url": "/files/ai-images/.../generated.png",
  "mime_type": "image/png",
  "size_bytes": 204800,
  "user_id": "user_abc123",
  "context_id": "ctx_xyz789",
  "ai_content": true,
  "metadata": {},
  "created_at": "2024-01-15T10:30:00Z",
  "updated_at": "2024-01-15T10:30:00Z"
}
```

**Artifact Type:** `Card`

---

### files ai count

Count AI-generated images. The `--user` flag is optional; when omitted, counts all AI images.

```bash
sp core files ai count
sp --json core files ai count
sp core files ai count --user user_abc123
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
sp --json core files config
sp --json core files stats

# Phase 2: Validate file before upload
sp --json core files validate ./image.png

# Phase 3: Upload file
sp --json core files upload ./image.png --context ctx_abc123

# Phase 4: Verify upload
sp --json core files list --limit 5
sp --json core files show b75940ac-c50f-4d46-9fdd-ebb4970b2a7d

# Phase 5: Search for files
sp --json core files search uploads

# Phase 6: Cleanup with dry-run preview
sp core files delete b75940ac-c50f-4d46-9fdd-ebb4970b2a7d --dry-run --yes

# Phase 7: Actual cleanup
sp core files delete b75940ac-c50f-4d46-9fdd-ebb4970b2a7d --yes

# Phase 8: Verify deletion
sp --json core files stats
```

---

## AI Image Workflow Example

```bash
# Phase 1: Count all AI-generated images
sp --json core files ai count

# Phase 2: List AI-generated images
sp --json core files ai list --limit 10

# Phase 3: Count AI images for specific user
sp --json core files ai count --user user_abc123

# Phase 4: Upload new AI-generated image
sp core files upload ./generated.png --context ctx_abc123 --ai

# Phase 5: Verify AI flag
sp --json core files show b75940ac-c50f-4d46-9fdd-ebb4970b2a7d
# Should show "ai_content": true
```

---

## Error Handling

### Missing Required Flags

```bash
sp core files upload ./image.png
# Error: --context is required

sp core files delete b75940ac-c50f-4d46-9fdd-ebb4970b2a7d
# Error: --yes is required to delete files in non-interactive mode
```

### Invalid UUID Format

```bash
sp core files show invalid-id
# Error: Invalid file ID format. Expected UUID like 'b75940ac-c50f-4d46-9fdd-ebb4970b2a7d', got 'invalid-id'

sp core files delete not-a-uuid --yes
# Error: Invalid file ID format. Expected UUID like 'b75940ac-c50f-4d46-9fdd-ebb4970b2a7d', got 'not-a-uuid'
```

### File Not Found

```bash
sp core files upload ./nonexistent.png --context ctx_abc
# Error: File not found: ./nonexistent.png

sp core files show 00000000-0000-0000-0000-000000000000
# Error: File not found: 00000000-0000-0000-0000-000000000000
```

### Validation Errors

```bash
sp core files upload ./toolarge.zip --context ctx_abc
# Error: File size 52428800 exceeds maximum 10485760 bytes

sp core files upload ./script.exe --context ctx_abc
# Error: MIME type 'application/x-msdownload' not allowed
```

### Upload Disabled

```bash
sp core files upload ./image.png --context ctx_abc
# Error: File uploads are disabled in configuration
```

---

## JSON Output

All commands support `--json` flag for structured output:

```bash
# Verify JSON is valid
sp --json core files list | jq .

# Extract specific fields
sp --json core files list | jq '.data.files[].id'
sp --json core files show b75940ac-c50f-4d46-9fdd-ebb4970b2a7d | jq '.data.public_url'
sp --json core files config | jq '.data.max_file_size_bytes'
sp --json core files stats | jq '.data.by_category.images'

# Filter by criteria
sp --json core files list | jq '.data.files[] | select(.ai_content == true)'
sp --json core files list | jq '.data.files[] | select(.size_bytes > 100000)'
sp --json core files search uploads | jq '.data.files[] | select(.mime_type | startswith("image/"))'
```

---

## Compliance Checklist

- [x] All `execute` functions accept `config: &CliConfig`
- [x] All commands return `CommandOutput` with proper artifact type
- [x] `delete` commands require `--yes` / `-y` flag in non-interactive mode
- [x] `--dry-run` support for destructive operations
- [x] All output types derive `Serialize`, `Deserialize`, `JsonSchema`
- [x] No `println!` / `eprintln!` - uses `CliService`
- [x] No `unwrap()` / `expect()` - uses `?` with `.context()`
- [x] JSON output supported via `--json` flag
- [x] Proper error messages for missing required flags
- [x] Proper error messages for invalid UUID format


---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>CLI reference · Own how your organization uses AI.</sub>

</div>
