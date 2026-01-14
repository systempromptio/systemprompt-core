# Content CLI Commands

This document provides complete documentation for AI agents to use the content CLI commands. All commands support non-interactive mode for automation.

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
| `content list` | List content with pagination | `Table` | No (DB only) |
| `content show <id>` | Show content details | `Card` | No (DB only) |
| `content search <query>` | Search content | `Table` | No (DB only) |
| `content ingest` | Ingest markdown files | `Text` | No (DB only) |
| `content delete <id>` | Delete content by ID | `Text` | No (DB only) |
| `content delete-source` | Delete all content from source | `Text` | No (DB only) |
| `content popular` | Get popular content | `Table` | No (DB only) |
| `content link list` | List content links | `Table` | No (DB only) |
| `content link create` | Create content link | `Text` | No (DB only) |
| `content link delete` | Delete content link | `Text` | No (DB only) |
| `content analytics views` | Content view analytics | `Table` | No (DB only) |
| `content analytics engagement` | Content engagement metrics | `Card` | No (DB only) |

---

## Core Commands

### content list

List content with pagination and filtering.

```bash
sp content list
sp --json content list
sp content list --limit 50 --offset 0
sp content list --source blog
sp content list --category tutorials
sp content list --status published
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--limit` | `20` | Maximum number of results |
| `--offset` | `0` | Number of results to skip |
| `--source` | None | Filter by source ID |
| `--category` | None | Filter by category |
| `--status` | None | Filter by status: `draft`, `published`, `archived` |

**Output Structure:**
```json
{
  "content": [
    {
      "id": "content_abc123",
      "slug": "getting-started",
      "title": "Getting Started Guide",
      "source_id": "blog",
      "category_id": "tutorials",
      "status": "published",
      "created_at": "2024-01-15T10:30:00Z",
      "updated_at": "2024-01-15T10:30:00Z"
    }
  ],
  "total": 1,
  "limit": 20,
  "offset": 0
}
```

**Artifact Type:** `Table`
**Columns:** `id`, `slug`, `title`, `source_id`, `status`, `created_at`

---

### content show

Show detailed content information.

```bash
sp content show <content-id>
sp --json content show content_abc123
sp content show blog/getting-started
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<id>` | Yes | Content ID or source/slug path |

**Output Structure:**
```json
{
  "id": "content_abc123",
  "slug": "getting-started",
  "title": "Getting Started Guide",
  "description": "A comprehensive guide to getting started",
  "source_id": "blog",
  "category_id": "tutorials",
  "status": "published",
  "content": "# Getting Started\n\nWelcome to...",
  "metadata": {
    "author": "John Doe",
    "tags": ["tutorial", "beginner"]
  },
  "views": 1520,
  "created_at": "2024-01-15T10:30:00Z",
  "updated_at": "2024-01-15T10:30:00Z"
}
```

**Artifact Type:** `Card`

---

### content search

Search content by query.

```bash
sp content search <query>
sp --json content search "getting started"
sp content search "tutorial" --source blog
sp content search "api" --limit 10
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<query>` | Yes | Search query |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--source` | None | Filter by source ID |
| `--limit` | `20` | Maximum number of results |

**Output Structure:**
```json
{
  "results": [
    {
      "id": "content_abc123",
      "slug": "getting-started",
      "title": "Getting Started Guide",
      "snippet": "...getting started with the platform...",
      "score": 0.95,
      "source_id": "blog"
    }
  ],
  "query": "getting started",
  "total": 1
}
```

**Artifact Type:** `Table`
**Columns:** `id`, `title`, `snippet`, `score`, `source_id`

---

### content ingest

Ingest markdown files from a directory into the database.

```bash
sp content ingest --source blog
sp content ingest --source blog --path ./content/blog
sp content ingest --source blog --recursive
sp content ingest --source blog --dry-run
```

**Required Flags (non-interactive):**
| Flag | Required | Description |
|------|----------|-------------|
| `--source` | Yes | Source ID for ingested content |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--path` | From config | Path to content directory |
| `--recursive` | `true` | Recursively process subdirectories |
| `--dry-run` | `false` | Preview without making changes |
| `--override` | `false` | Override existing content |

**Output Structure:**
```json
{
  "source_id": "blog",
  "path": "/var/www/html/tyingshoelaces/services/content/blog",
  "files_processed": 25,
  "created": 20,
  "updated": 5,
  "skipped": 0,
  "errors": [],
  "message": "Ingested 25 files into source 'blog'"
}
```

**Artifact Type:** `Text`

---

### content delete

Delete content by ID.

```bash
sp content delete <content-id> --yes
sp content delete content_abc123 --yes
sp content delete blog/getting-started --yes
```

**Required Flags (non-interactive):**
| Flag | Required | Description |
|------|----------|-------------|
| `<id>` | Yes | Content ID or source/slug path |
| `--yes` / `-y` | Yes | Skip confirmation |

**Output Structure:**
```json
{
  "deleted": "content_abc123",
  "message": "Content 'content_abc123' deleted successfully"
}
```

**Artifact Type:** `Text`

---

### content delete-source

Delete all content from a source.

```bash
sp content delete-source <source-id> --yes
sp content delete-source blog --yes
sp content delete-source tutorials --yes --hard
```

**Required Flags (non-interactive):**
| Flag | Required | Description |
|------|----------|-------------|
| `<source>` | Yes | Source ID |
| `--yes` / `-y` | Yes | Skip confirmation |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--hard` | `false` | Permanently delete (cannot recover) |

**Output Structure:**
```json
{
  "source_id": "blog",
  "deleted_count": 25,
  "hard_delete": false,
  "message": "Deleted 25 content items from source 'blog'"
}
```

**Artifact Type:** `Text`

---

### content popular

Get popular content based on views.

```bash
sp content popular
sp --json content popular
sp content popular --limit 10
sp content popular --since 7d
sp content popular --source blog
```

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--limit` | `10` | Maximum number of results |
| `--since` | `30d` | Time period |
| `--source` | None | Filter by source ID |

**Output Structure:**
```json
{
  "content": [
    {
      "id": "content_abc123",
      "slug": "popular-article",
      "title": "Most Popular Article",
      "views": 5200,
      "source_id": "blog"
    }
  ],
  "period": "30d",
  "total": 10
}
```

**Artifact Type:** `Table`
**Columns:** `id`, `title`, `views`, `source_id`

---

## Link Commands

### content link list

List content links (related content).

```bash
sp content link list
sp --json content link list
sp content link list --content content_abc123
sp content link list --type related
```

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--content` | None | Filter by content ID |
| `--type` | None | Filter by link type |

**Output Structure:**
```json
{
  "links": [
    {
      "source_id": "content_abc123",
      "target_id": "content_xyz789",
      "link_type": "related",
      "weight": 0.85,
      "created_at": "2024-01-15T10:30:00Z"
    }
  ],
  "total": 1
}
```

**Artifact Type:** `Table`
**Columns:** `source_id`, `target_id`, `link_type`, `weight`

---

### content link create

Create a content link.

```bash
sp content link create --source <content-id> --target <content-id>
sp content link create --source content_abc --target content_xyz --type related
sp content link create --source content_abc --target content_xyz --weight 0.9
```

**Required Flags (non-interactive):**
| Flag | Required | Description |
|------|----------|-------------|
| `--source` | Yes | Source content ID |
| `--target` | Yes | Target content ID |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--type` | `related` | Link type: `related`, `next`, `previous`, `parent` |
| `--weight` | `1.0` | Link weight (0.0 to 1.0) |

**Output Structure:**
```json
{
  "source_id": "content_abc123",
  "target_id": "content_xyz789",
  "link_type": "related",
  "message": "Content link created successfully"
}
```

**Artifact Type:** `Text`

---

### content link delete

Delete a content link.

```bash
sp content link delete --source <content-id> --target <content-id> --yes
sp content link delete --source content_abc --target content_xyz --yes
```

**Required Flags (non-interactive):**
| Flag | Required | Description |
|------|----------|-------------|
| `--source` | Yes | Source content ID |
| `--target` | Yes | Target content ID |
| `--yes` / `-y` | Yes | Skip confirmation |

**Output Structure:**
```json
{
  "source_id": "content_abc123",
  "target_id": "content_xyz789",
  "message": "Content link deleted successfully"
}
```

**Artifact Type:** `Text`

---

## Analytics Commands

### content analytics views

View content analytics.

```bash
sp content analytics views
sp --json content analytics views
sp content analytics views --content content_abc123
sp content analytics views --since 7d
```

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--content` | None | Filter by content ID |
| `--since` | `30d` | Time period |
| `--group-by` | `day` | Group by: `hour`, `day`, `week` |

**Output Structure:**
```json
{
  "views": [
    {
      "timestamp": "2024-01-14",
      "content_id": "content_abc123",
      "views": 150,
      "unique_visitors": 120
    }
  ],
  "period": "7d",
  "total_views": 1050
}
```

**Artifact Type:** `Table`

---

### content analytics engagement

Content engagement metrics.

```bash
sp content analytics engagement
sp --json content analytics engagement
sp content analytics engagement --content content_abc123
```

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--content` | None | Filter by content ID |
| `--since` | `30d` | Time period |

**Output Structure:**
```json
{
  "content_id": "content_abc123",
  "period": "30d",
  "metrics": {
    "total_views": 5200,
    "unique_visitors": 3800,
    "avg_time_on_page_seconds": 185,
    "avg_scroll_depth": 0.72,
    "bounce_rate": 0.35
  }
}
```

**Artifact Type:** `Card`

---

## Complete Content Management Flow Example

```bash
# Phase 1: Create content directory and files
mkdir -p /services/content/tutorials
cat << 'EOF' > /services/content/tutorials/getting-started.md
---
title: Getting Started
slug: getting-started
description: A beginner's guide
author: Developer
tags: [tutorial, beginner]
---

# Getting Started

Welcome to our platform...
EOF

# Phase 2: Ingest content
sp content ingest --source tutorials

# Phase 3: Verify content
sp --json content list --source tutorials
sp --json content show tutorials/getting-started

# Phase 4: Search content
sp --json content search "getting started"

# Phase 5: Check popular content
sp --json content popular --since 7d

# Phase 6: View analytics
sp --json content analytics views --since 7d
sp --json content analytics engagement

# Phase 7: Create related links
sp content link create --source content_abc --target content_xyz --type related

# Phase 8: Delete content
sp content delete tutorials/getting-started --yes

# Phase 9: Delete all from source
sp content delete-source tutorials --yes
```

---

## Error Handling

### Missing Required Flags

```bash
sp content ingest
# Error: --source is required

sp content delete content_abc
# Error: --yes is required to delete content in non-interactive mode
```

### Content Not Found

```bash
sp content show nonexistent
# Error: Content 'nonexistent' not found

sp content delete nonexistent --yes
# Error: Content 'nonexistent' not found
```

---

## JSON Output

All commands support `--json` flag for structured output:

```bash
# Verify JSON is valid
sp --json content list | jq .

# Extract specific fields
sp --json content list | jq '.content[].title'
sp --json content show content_abc | jq '.metadata'
sp --json content popular | jq '.content[] | {title, views}'
```

---

## Compliance Checklist

- [x] All `execute` functions accept `config: &CliConfig`
- [x] All commands return `CommandResult<T>` with proper artifact type
- [x] `delete` commands require `--yes` / `-y` flag
- [x] All output types derive `Serialize`, `Deserialize`, `JsonSchema`
- [x] No `println!` / `eprintln!` - uses `render_result()`
- [x] No `unwrap()` / `expect()` - uses `?` with `.context()`
- [x] JSON output supported via `--json` flag
