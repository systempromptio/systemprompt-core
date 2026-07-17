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


# Content CLI Commands

Publish, inspect, and delete content on infrastructure you own, every command auditable and scriptable. Content lives in your PostgreSQL, not a vendor's. Every command runs non-interactively for automation.

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
| `core content list` | List content with pagination | `Table` | No (DB only) |
| `core content show <id>` | Show content details | `Card` | No (DB only) |
| `core content search <query>` | Search content | `Table` | No (DB only) |
| `core content edit <id>` | Edit content fields | `Card` | No (DB only) |
| `core content delete <id>` | Delete content by ID | `Card` | No (DB only) |
| `core content delete-source <id>` | Delete all content from source | `Card` | No (DB only) |
| `core content popular` | Get popular content | `Table` | No (DB only) |
| `core content verify <id>` | Verify content is published and accessible | `Card` | No (DB only) |
| `core content status --source <id>` | Show content health status for a source | `Card` | No (DB only) |
| `core content link generate` | Generate trackable link | `Card` | No (DB only) |
| `core content link show` | Show link details | `Card` | No (DB only) |
| `core content link list` | List links | `Table` | No (DB only) |
| `core content link performance` | Link performance metrics | `Card` | No (DB only) |
| `core content link delete` | Delete a link | `Card` | No (DB only) |
| `core content analytics clicks` | Link click history | `Table` | No (DB only) |
| `core content analytics campaign` | Campaign analytics | `Card` | No (DB only) |
| `core content analytics journey` | Content navigation graph | `Table` | No (DB only) |
| `core content files link` | Link a file to content with a role | `Card` | No (DB only) |
| `core content files unlink` | Unlink a file from content | `Card` | No (DB only) |
| `core content files list` | List files attached to content | `Table` | No (DB only) |
| `core content files featured` | Get or set the featured image | `Card` | No (DB only) |

---

## Core Commands

### content list

List content with pagination and filtering.

```bash
sp core content list
sp --json core content list
sp core content list --limit 50 --offset 0
sp core content list --source blog
sp core content list --category tutorials
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--limit` | `20` | Maximum number of results |
| `--offset` | `0` | Number of results to skip |
| `--source` | None | Filter by source ID |
| `--category` | None | Filter by category ID |

**Output Structure:**
```json
{
  "items": [
    {
      "id": "content_abc123",
      "slug": "getting-started",
      "title": "Getting Started Guide",
      "kind": "article",
      "source_id": "blog",
      "category_id": "tutorials",
      "published_at": "2024-01-15T10:30:00Z"
    }
  ],
  "total": 1,
  "limit": 20,
  "offset": 0
}
```

**Artifact Type:** `Table`
**Columns:** `id`, `title`, `kind`, `source_id`, `published_at`

---

### content show

Show detailed content information.

```bash
sp core content show <content-id>
sp --json core content show dc2ae776-debb-4a75-9e8d-90c9131382e0
sp core content show getting-started --source blog
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<identifier>` | Yes | Content ID or slug |

**Optional Flags:**
| Flag | Description |
|------|-------------|
| `--source` | Source ID (required when using slug) |

**Output Structure:**
```json
{
  "id": "content_abc123",
  "slug": "getting-started",
  "title": "Getting Started Guide",
  "description": "A comprehensive guide to getting started",
  "body": "# Getting Started\n\nWelcome to...",
  "author": "John Doe",
  "published_at": "2024-01-15T10:30:00Z",
  "keywords": ["tutorial", "beginner"],
  "kind": "article",
  "image": "/images/getting-started.webp",
  "category_id": "tutorials",
  "source_id": "blog",
  "version_hash": "abc123...",
  "is_public": true,
  "updated_at": "2024-01-15T10:30:00Z"
}
```

**Artifact Type:** `Card`

---

### content search

Search content by query.

```bash
sp core content search <query>
sp --json core content search "getting started"
sp core content search "tutorial" --source blog
sp core content search "api" --category docs --limit 10
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<query>` | Yes | Search query |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--source` | None | Filter by source ID |
| `--category` | None | Filter by category ID |
| `--limit` | `20` | Maximum number of results |

**Output Structure:**
```json
{
  "results": [
    {
      "id": "content_abc123",
      "slug": "getting-started",
      "title": "Getting Started Guide",
      "description": "A comprehensive guide...",
      "image": "/images/getting-started.webp",
      "source_id": "blog",
      "category_id": "tutorials"
    }
  ],
  "total": 1,
  "query": "getting started"
}
```

**Artifact Type:** `Table`
**Columns:** `id`, `title`, `slug`, `source_id`

---

### content edit

Edit fields on an existing content item. Requires full profile context.

```bash
sp core content edit <content-id> --set title="New Title"
sp core content edit getting-started --source blog --set description="Updated"
sp core content edit <content-id> --public
sp core content edit <content-id> --body-file ./updated-body.md
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<identifier>` | Yes | Content ID or slug to edit |

**Optional Flags:**
| Flag | Description |
|------|-------------|
| `--source` | Source ID (required when using slug) |
| `--set <KEY=VALUE>` | Set a field value (repeatable) |
| `--public` | Make content public (conflicts with `--private`) |
| `--private` | Make content private (conflicts with `--public`) |
| `--body` | Body content inline |
| `--body-file` | File containing body content |

**Artifact Type:** `Card`

---

### content verify

Verify that a content item is published and accessible, optionally checking prerendered HTML and live HTTP status.

```bash
sp core content verify <content-id>
sp core content verify getting-started --source blog
sp core content verify getting-started --source blog --base-url https://example.com --url-pattern "/{source}/{slug}"
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<identifier>` | Yes | Content slug or ID |

**Optional Flags:**
| Flag | Description |
|------|-------------|
| `--source` | Source ID (required when using slug) |
| `--web-dist` | Web dist directory to check for prerendered HTML |
| `--base-url` | Base URL to check HTTP status |
| `--url-pattern` | URL pattern, e.g. `/{source}/{slug}` |

**Artifact Type:** `Card`

---

### content status

Show content health status for an entire source.

```bash
sp core content status --source blog
sp --json core content status --source blog --limit 100
```

**Required Flags:**
| Flag | Required | Description |
|------|----------|-------------|
| `--source` | Yes | Source ID |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--limit` | `50` | Maximum items to check |
| `--web-dist` | None | Web dist directory to check for prerendered HTML |
| `--url-pattern` | None | URL pattern, e.g. `/{source}/{slug}` |

**Artifact Type:** `Card`

---

### content delete

Delete content by ID.

```bash
sp core content delete <content-id> --yes
sp core content delete dc2ae776-debb-4a75-9e8d-90c9131382e0 --yes
sp core content delete getting-started --source blog --yes
```

**Required Flags (non-interactive):**
| Flag | Required | Description |
|------|----------|-------------|
| `<identifier>` | Yes | Content ID or slug |
| `--yes` / `-y` | Yes | Skip confirmation |

**Optional Flags:**
| Flag | Description |
|------|-------------|
| `--source` | Source ID (required when using slug) |

**Output Structure:**
```json
{
  "deleted": true,
  "content_id": "dc2ae776-debb-4a75-9e8d-90c9131382e0"
}
```

**Artifact Type:** `Card`

---

### content delete-source

Delete all content from a source.

```bash
sp core content delete-source <source-id> --yes
sp core content delete-source test-source --yes
```

**Required Flags (non-interactive):**
| Flag | Required | Description |
|------|----------|-------------|
| `<source_id>` | Yes | Source ID |
| `--yes` / `-y` | Yes | Skip confirmation |

**Output Structure:**
```json
{
  "deleted_count": 25,
  "source_id": "blog"
}
```

**Artifact Type:** `Card`

---

### content popular

Get popular content based on views.

```bash
sp core content popular --source blog
sp --json core content popular --source blog
sp core content popular --source blog --limit 10
sp core content popular --source blog --since 7d
sp core content popular --source docs --since 1w
```

**Required Flags:**
| Flag | Required | Description |
|------|----------|-------------|
| `--source` | Yes | Source ID |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--limit` | `10` | Maximum number of results |
| `--since` | `30d` | Time period (e.g., `7d`, `30d`, `1w`) |

**Output Structure:**
```json
{
  "items": [
    {
      "id": "content_abc123",
      "slug": "popular-article",
      "title": "Most Popular Article",
      "kind": "article",
      "source_id": "blog",
      "category_id": "tutorials",
      "published_at": "2024-01-15T10:30:00Z"
    }
  ],
  "source_id": "blog",
  "days": 30
}
```

**Artifact Type:** `Table`
**Columns:** `id`, `title`, `kind`, `published_at`

---

## Link Commands

### content link generate

Generate a trackable campaign link.

```bash
sp core content link generate --url https://example.com
sp core content link generate --url https://example.com --campaign my-campaign
sp core content link generate --url https://example.com --utm-source twitter --utm-medium social
sp core content link generate --url https://example.com --link-type redirect
```

**Required Flags:**
| Flag | Required | Description |
|------|----------|-------------|
| `--url` | Yes | Target URL |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--campaign` | None | Campaign ID |
| `--campaign-name` | None | Campaign name |
| `--content` | None | Source content ID |
| `--utm-source` | None | UTM source parameter |
| `--utm-medium` | None | UTM medium parameter |
| `--utm-campaign` | None | UTM campaign parameter |
| `--utm-term` | None | UTM term parameter |
| `--utm-content` | None | UTM content parameter |
| `--link-type` | `both` | Link type: `redirect`, `utm`, `both` |

**Output Structure:**
```json
{
  "link_id": "abc123",
  "short_code": "6WRVOTgT",
  "short_url": "https://systemprompt.io/r/6WRVOTgT",
  "target_url": "https://example.com",
  "full_url": "https://example.com?utm_source=...",
  "link_type": "both",
  "utm_params": {
    "source": "twitter",
    "medium": "social"
  }
}
```

**Artifact Type:** `Card`

---

### content link show

Show link details by short code.

```bash
sp core content link show <short-code>
sp --json core content link show 6WRVOTgT
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<short_code>` | Yes | Link short code |

**Output Structure:**
```json
{
  "id": "abc123",
  "short_code": "6WRVOTgT",
  "target_url": "https://example.com",
  "full_url": "https://example.com?utm_source=...",
  "link_type": "both",
  "campaign_id": "my-campaign",
  "campaign_name": "My Campaign",
  "click_count": 150,
  "unique_click_count": 120,
  "conversion_count": 10,
  "is_active": true,
  "created_at": "2024-01-15T10:30:00Z"
}
```

**Artifact Type:** `Card`

---

### content link list

List links by campaign or content.

```bash
sp core content link list --campaign my-campaign
sp core content link list --content content_abc123
sp --json core content link list --campaign my-campaign
```

**Required Flags (at least one):**
| Flag | Description |
|------|-------------|
| `--campaign` | Filter by campaign ID |
| `--content` | Filter by source content ID |

**Output Structure:**
```json
{
  "links": [
    {
      "id": "abc123",
      "short_code": "6WRVOTgT",
      "target_url": "https://example.com",
      "link_type": "both",
      "campaign_name": "My Campaign",
      "click_count": 150,
      "created_at": "2024-01-15T10:30:00Z"
    }
  ],
  "total": 1
}
```

**Artifact Type:** `Table`
**Columns:** `id`, `short_code`, `target_url`, `click_count`

---

### content link performance

Show link performance metrics.

```bash
sp core content link performance <link-id>
sp --json core content link performance abc123
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<link_id>` | Yes | Link ID |

**Output Structure:**
```json
{
  "link_id": "abc123",
  "click_count": 150,
  "unique_click_count": 120,
  "conversion_count": 10,
  "conversion_rate": 0.083
}
```

**Artifact Type:** `Card`

---

### content link delete

Delete a link.

```bash
sp core content link delete <link-id> --yes
sp core content link delete abc123 --yes
```

**Required Flags (non-interactive):**
| Flag | Required | Description |
|------|----------|-------------|
| `<link_id>` | Yes | Link ID |
| `--yes` / `-y` | Yes | Skip confirmation |

**Output Structure:**
```json
{
  "deleted": true,
  "link_id": "abc123"
}
```

**Artifact Type:** `Card`

---

## Analytics Commands

### content analytics clicks

Show click history for a link.

```bash
sp core content analytics clicks <link-id>
sp --json core content analytics clicks abc123
sp core content analytics clicks abc123 --limit 50
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<link_id>` | Yes | Link ID |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--limit` | `20` | Maximum number of results |
| `--offset` | `0` | Number of results to skip |

**Output Structure:**
```json
{
  "link_id": "abc123",
  "clicks": [
    {
      "click_id": "click_123",
      "session_id": "session_456",
      "user_id": "user_789",
      "clicked_at": "2024-01-15T10:30:00Z",
      "referrer_page": "/blog/article",
      "device_type": "desktop",
      "country": "US",
      "is_conversion": false
    }
  ],
  "total": 150
}
```

**Artifact Type:** `Table`
**Columns:** `click_id`, `session_id`, `clicked_at`, `device_type`, `country`

---

### content analytics campaign

Show campaign-level analytics.

```bash
sp core content analytics campaign <campaign-id>
sp --json core content analytics campaign my-campaign
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<campaign_id>` | Yes | Campaign ID |

**Output Structure:**
```json
{
  "campaign_id": "my-campaign",
  "total_clicks": 1500,
  "link_count": 10,
  "unique_visitors": 1200,
  "conversion_count": 50
}
```

**Artifact Type:** `Card`

---

### content analytics journey

Show content navigation graph.

```bash
sp core content analytics journey
sp --json core content analytics journey
sp core content analytics journey --limit 50
```

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--limit` | `20` | Maximum number of results |
| `--offset` | `0` | Number of results to skip |

**Output Structure:**
```json
{
  "nodes": [
    {
      "source_content_id": "content_abc",
      "target_url": "https://example.com",
      "click_count": 150
    }
  ]
}
```

**Artifact Type:** `Table`
**Columns:** `source_content_id`, `target_url`, `click_count`

---

## Content-File Commands

Associate stored files (uploaded via `core files`) with content items. Roles: `featured`, `attachment`, `inline`, `og-image`, `thumbnail`.

### content files link

```bash
sp core content files link <file-id> --content <content-id> --role featured
sp core content files link <file-id> --content <content-id> --role attachment --order 1
```

| Argument / Flag | Required | Description |
|-----------------|----------|-------------|
| `<file-id>` | Yes | File ID |
| `--content` | Yes | Content ID |
| `--role` | Yes | One of `featured`, `attachment`, `inline`, `og-image`, `thumbnail` |
| `--order` | No | Display order (default `0`) |

### content files unlink

```bash
sp core content files unlink <file-id> --content <content-id> --yes
sp core content files unlink <file-id> --content <content-id> --dry-run --yes
```

`--yes` / `-y` is required in non-interactive mode; `--dry-run` previews without executing.

### content files list

```bash
sp core content files list --content <content-id>
sp core content files list --file <file-id>
```

Pass `--content` to list files attached to a content item, or `--file` for the reverse lookup.

### content files featured

```bash
sp core content files featured <content-id>
sp core content files featured <content-id> --set <file-id>
```

Without `--set`, returns the current featured image; with `--set`, assigns it.

---

## Complete Content Management Flow Example

```bash
# Phase 1: Inspect existing content
sp --json core content list --source blog
sp --json core content show getting-started --source blog

# Phase 2: Search content
sp --json core content search "getting started"

# Phase 3: Edit a field
sp core content edit getting-started --source blog --set description="Updated"

# Phase 4: Verify it is published and reachable
sp core content verify getting-started --source blog --base-url https://example.com

# Phase 5: Check source-wide health
sp --json core content status --source blog

# Phase 6: Check popular content
sp --json core content popular --source blog --since 7d

# Phase 7: Generate trackable link
sp core content link generate --url https://example.com --campaign test --utm-source cli

# Phase 8: View link analytics
sp --json core content analytics clicks <link-id>

# Phase 9: Delete content
sp core content delete getting-started --source blog --yes

# Phase 10: Delete all from source
sp core content delete-source blog --yes
```

---

## Error Handling

### Missing Required Flags

```bash
sp core content delete content_abc
# Error: --yes is required to delete content in non-interactive mode

sp core content link list
# Error: Either --campaign or --content must be specified
```

### Content Not Found

```bash
sp core content show nonexistent
# Error: Source ID required when using slug

sp core content show nonexistent --source blog
# Error: Content not found: nonexistent in source blog

sp core content delete nonexistent --yes
# Error: Source ID required when using slug (use --source)
```

---

## JSON Output

All commands support `--json` flag for structured output:

```bash
# Verify JSON is valid
sp --json core content list | jq .

# Extract specific fields
sp --json core content list | jq '.items[].title'
sp --json core content show content_abc | jq '.body'
sp --json core content popular --source blog | jq '.items[] | {title, kind}'
```

---

## Compliance Checklist

- [x] All `execute` functions accept `config: &CliConfig`
- [x] All commands return `CommandOutput` with proper artifact type
- [x] `delete` commands require `--yes` / `-y` flag
- [x] All output types derive `Serialize`, `Deserialize`, `JsonSchema`
- [x] No `println!` / `eprintln!` - uses `render_result()`
- [x] No `unwrap()` / `expect()` - uses `?` with `.context()`
- [x] JSON output supported via `--json` flag
- [x] Uses `config.is_interactive()` for interactive checks


---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>CLI reference · Own how your organization uses AI.</sub>

</div>
