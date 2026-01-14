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
| `files delete <id>` | Delete a file | `Text` | No (DB only) |
| `files validate <path>` | Validate a file before upload | `Card` | No |
| `files config` | Show file upload configuration | `Card` | No |
| `files content list` | List content-file links | `Table` | No (DB only) |
| `files content link` | Link file to content | `Text` | No (DB only) |
| `files content unlink` | Unlink file from content | `Text` | No (DB only) |
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
      "id": "file_abc123",
      "path": "uploads/2024/01/image.png",
      "public_url": "https://example.com/uploads/2024/01/image.png",
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
sp --json files show file_abc123
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<id>` | Yes | File ID to show |

**Output Structure:**
```json
{
  "id": "file_abc123",
  "path": "uploads/2024/01/image.png",
  "public_url": "https://example.com/uploads/2024/01/image.png",
  "mime_type": "image/png",
  "size_bytes": 102400,
  "checksum_sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
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
  "file_id": "file_abc123",
  "path": "uploads/2024/01/image.png",
  "public_url": "https://example.com/uploads/2024/01/image.png",
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
sp files delete file_abc123 --yes
```

**Required Flags (non-interactive):**
| Flag | Required | Description |
|------|----------|-------------|
| `<id>` | Yes | File ID to delete |
| `--yes` / `-y` | Yes | Skip confirmation (REQUIRED in non-interactive mode) |

**Output Structure:**
```json
{
  "deleted": "file_abc123",
  "message": "File 'file_abc123' deleted successfully"
}
```

**Artifact Type:** `Text`

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
  "path": "./image.png",
  "size_bytes": 102400,
  "mime_type": "image/png",
  "warnings": [],
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
  "enabled": true,
  "max_size_bytes": 10485760,
  "allowed_types": ["image/*", "application/pdf", "text/*"],
  "storage_path": "/var/www/html/tyingshoelaces/uploads",
  "public_url_base": "https://example.com/uploads"
}
```

**Artifact Type:** `Card`

---

## Content-File Link Commands

### files content list

List content-file links.

```bash
sp files content list
sp --json files content list
sp files content list --content content_abc123
sp files content list --file file_xyz789
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--content` | None | Filter by content ID |
| `--file` | None | Filter by file ID |

**Output Structure:**
```json
{
  "links": [
    {
      "content_id": "content_abc123",
      "file_id": "file_xyz789",
      "link_type": "attachment",
      "created_at": "2024-01-15T10:30:00Z"
    }
  ],
  "total": 1
}
```

**Artifact Type:** `Table`
**Columns:** `content_id`, `file_id`, `link_type`, `created_at`

---

### files content link

Link a file to content.

```bash
sp files content link --content <content-id> --file <file-id>
sp files content link --content content_abc123 --file file_xyz789
sp files content link --content content_abc123 --file file_xyz789 --type attachment
```

**Required Flags (non-interactive):**
| Flag | Required | Description |
|------|----------|-------------|
| `--content` | Yes | Content ID |
| `--file` | Yes | File ID |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--type` | `attachment` | Link type: `attachment`, `thumbnail`, `preview` |

**Output Structure:**
```json
{
  "content_id": "content_abc123",
  "file_id": "file_xyz789",
  "link_type": "attachment",
  "message": "File linked to content successfully"
}
```

**Artifact Type:** `Text`

---

### files content unlink

Unlink a file from content.

```bash
sp files content unlink --content <content-id> --file <file-id> --yes
sp files content unlink --content content_abc123 --file file_xyz789 --yes
```

**Required Flags (non-interactive):**
| Flag | Required | Description |
|------|----------|-------------|
| `--content` | Yes | Content ID |
| `--file` | Yes | File ID |
| `--yes` / `-y` | Yes | Skip confirmation |

**Output Structure:**
```json
{
  "content_id": "content_abc123",
  "file_id": "file_xyz789",
  "message": "File unlinked from content successfully"
}
```

**Artifact Type:** `Text`

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
      "id": "file_abc123",
      "path": "ai-images/2024/01/generated.png",
      "public_url": "https://example.com/ai-images/2024/01/generated.png",
      "mime_type": "image/png",
      "size_bytes": 204800,
      "prompt": "A sunset over mountains",
      "model": "dall-e-3",
      "created_at": "2024-01-15T10:30:00Z"
    }
  ],
  "total": 1
}
```

**Artifact Type:** `Table`
**Columns:** `id`, `path`, `size_bytes`, `model`, `created_at`

---

### files ai count

Count AI-generated images.

```bash
sp files ai count
sp --json files ai count
sp files ai count --user user_abc123
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--user` | None | Filter by user ID |

**Output Structure:**
```json
{
  "total_count": 150,
  "total_size_bytes": 31457280,
  "by_model": {
    "dall-e-3": 100,
    "stable-diffusion": 50
  }
}
```

**Artifact Type:** `Card`

---

## Complete File Management Flow Example

This flow demonstrates the full file management lifecycle:

```bash
# Phase 1: Check configuration
sp --json files config

# Phase 2: Validate file before upload
sp --json files validate ./image.png

# Phase 3: Upload file
sp --json files upload ./image.png --context ctx_abc123

# Phase 4: Verify upload
sp --json files list --limit 5
sp --json files show file_abc123

# Phase 5: Link file to content
sp files content link --content content_xyz --file file_abc123 --type attachment

# Phase 6: List content-file links
sp --json files content list --file file_abc123

# Phase 7: Cleanup (if needed)
sp files content unlink --content content_xyz --file file_abc123 --yes
sp files delete file_abc123 --yes

# Phase 8: Verify deletion
sp --json files list
```

---

## AI Image Workflow Example

```bash
# Phase 1: List AI-generated images
sp --json files ai list --limit 10

# Phase 2: Count AI images
sp --json files ai count

# Phase 3: Upload new AI-generated image
sp files upload ./generated.png --context ctx_abc123 --ai

# Phase 4: Verify AI flag
sp --json files show file_abc123
# Should show "ai_content": true
```

---

## Error Handling

### Missing Required Flags

```bash
sp files upload ./image.png
# Error: --context is required

sp files delete file_abc123
# Error: --yes is required to delete files in non-interactive mode

sp files content link --content content_abc
# Error: --file is required
```

### File Not Found

```bash
sp files upload ./nonexistent.png --context ctx_abc
# Error: File not found: ./nonexistent.png

sp files show nonexistent
# Error: File 'nonexistent' not found
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
sp --json files list | jq '.files[].id'
sp --json files show file_abc123 | jq '.public_url'
sp --json files config | jq '.max_size_bytes'

# Filter by criteria
sp --json files list | jq '.files[] | select(.ai_content == true)'
sp --json files list | jq '.files[] | select(.size_bytes > 100000)'
sp --json files ai list | jq '.files[] | select(.model == "dall-e-3")'
```

---

## Compliance Checklist

- [x] All `execute` functions accept `config: &CliConfig`
- [x] All commands return `CommandResult<T>` with proper artifact type
- [x] `delete` commands require `--yes` / `-y` flag
- [x] All output types derive `Serialize`, `Deserialize`, `JsonSchema`
- [x] No `println!` / `eprintln!` - uses `CliService`
- [x] No `unwrap()` / `expect()` - uses `?` with `.context()`
- [x] JSON output supported via `--json` flag
- [x] Proper error messages for missing required flags
