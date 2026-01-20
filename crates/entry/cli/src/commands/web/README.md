# Web CLI Commands

This document provides complete documentation for AI agents to use the web CLI commands. All commands support non-interactive mode for automation.

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
| `web content-types list` | List all content types | `Table` | No |
| `web content-types show <name>` | Display content type details | `Card` | No |
| `web content-types create` | Create new content type | `Text` | No |
| `web content-types edit <name>` | Edit content type | `Text` | No |
| `web content-types delete <name>` | Delete content type | `Text` | No |
| `web templates list` | List all templates | `Table` | No |
| `web templates show <name>` | Display template details | `Card` | No |
| `web templates create` | Create new template | `Text` | No |
| `web templates edit <name>` | Edit template | `Text` | No |
| `web templates delete <name>` | Delete template | `Text` | No |
| `web assets list` | List all assets | `Table` | No |
| `web assets show <path>` | Display asset details | `Card` | No |
| `web sitemap show` | Show sitemap configuration | `Table` | No |
| `web sitemap generate` | Generate sitemap.xml | `Text` | No |
| `web validate` | Validate web configuration | `Table` | No |

---

## Content Types Commands

### web content-types list

List all configured content types from the content configuration.

```bash
sp web content-types list
sp --json web content-types list
sp web content-types list --enabled
sp web content-types list --disabled
sp web content-types list --category blog
```

**Flags:**
| Flag | Description |
|------|-------------|
| `--enabled` | Show only enabled content types |
| `--disabled` | Show only disabled content types |
| `--category` | Filter by category ID |

**Output Structure:**
```json
{
  "content_types": [
    {
      "name": "blog",
      "source_id": "blog",
      "category_id": "blog",
      "enabled": true,
      "path": "content/blog",
      "url_pattern": "/blog/{slug}"
    }
  ]
}
```

**Artifact Type:** `Table`
**Columns:** `name`, `source_id`, `category_id`, `enabled`, `path`, `url_pattern`

---

### web content-types show

Display detailed configuration for a specific content type.

```bash
sp web content-types show <name>
sp --json web content-types show blog
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<name>` | Yes | Content type name to show |

**Output Structure:**
```json
{
  "name": "blog",
  "source_id": "blog",
  "category_id": "blog",
  "enabled": true,
  "path": "content/blog",
  "description": "Blog posts and articles",
  "allowed_content_types": ["article"],
  "sitemap": {
    "enabled": true,
    "url_pattern": "/blog/{slug}",
    "priority": 0.8,
    "changefreq": "weekly",
    "fetch_from": "database",
    "parent_route": {
      "enabled": true,
      "url": "/blog",
      "priority": 0.9,
      "changefreq": "daily"
    }
  },
  "branding": null,
  "indexing": {
    "clear_before": false,
    "recursive": true,
    "override_existing": false
  }
}
```

**Artifact Type:** `Card`

---

### web content-types create

Create a new content type configuration.

```bash
sp web content-types create \
  --name "tutorials" \
  --path "content/tutorials" \
  --source-id "tutorials" \
  --category-id "learning" \
  --description "Step-by-step tutorials" \
  --url-pattern "/tutorials/{slug}" \
  --priority 0.7 \
  --changefreq weekly \
  --enabled
```

**Required Flags (non-interactive):**
| Flag | Required | Description |
|------|----------|-------------|
| `--name` | Yes | Content type identifier (lowercase alphanumeric + hyphens) |
| `--path` | Yes | Content path relative to services |
| `--source-id` | Yes | Source identifier |
| `--category-id` | Yes | Category identifier |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--description` | Empty | Content type description |
| `--url-pattern` | None | URL pattern for sitemap (e.g., `/blog/{slug}`) |
| `--priority` | `0.5` | Sitemap priority (0.0-1.0) |
| `--changefreq` | `weekly` | Sitemap change frequency |
| `--enabled` | `false` | Enable content type after creation |

**Validation Rules:**
- Name: 2+ characters, lowercase alphanumeric with hyphens only
- Priority: Must be between 0.0 and 1.0
- Changefreq: Valid values are `always`, `hourly`, `daily`, `weekly`, `monthly`, `yearly`, `never`

**Output Structure:**
```json
{
  "name": "tutorials",
  "message": "Content type 'tutorials' created successfully"
}
```

**Artifact Type:** `Text`

---

### web content-types edit

Edit an existing content type configuration.

```bash
sp web content-types edit <name> --enable
sp web content-types edit <name> --disable
sp web content-types edit <name> --path "content/new-path"
sp web content-types edit <name> --description "Updated description"
sp web content-types edit <name> --url-pattern "/new/{slug}"
sp web content-types edit <name> --priority 0.9
sp web content-types edit <name> --changefreq daily
sp web content-types edit <name> --set sitemap.priority=0.8
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<name>` | Yes | Content type name to edit |
| At least one change | Yes | Must specify at least one modification |

**Modification Flags:**
| Flag | Description |
|------|-------------|
| `--enable` | Enable the content type |
| `--disable` | Disable the content type |
| `--path` | Change content path |
| `--description` | Change description |
| `--url-pattern` | Change sitemap URL pattern |
| `--priority` | Change sitemap priority (0.0-1.0) |
| `--changefreq` | Change sitemap change frequency |
| `--set <key=value>` | Set arbitrary config value |

**Supported --set Keys:**
- `description`
- `path`
- `enabled` (boolean)
- `sitemap.url_pattern`
- `sitemap.priority`
- `sitemap.changefreq`

**Output Structure:**
```json
{
  "name": "tutorials",
  "message": "Content type 'tutorials' updated successfully with 2 change(s)",
  "changes": [
    "enabled: true",
    "sitemap.priority: 0.8"
  ]
}
```

**Artifact Type:** `Text`

---

### web content-types delete

Delete a content type configuration.

```bash
sp web content-types delete <name> --yes
```

**Required Flags (non-interactive):**
| Flag | Required | Description |
|------|----------|-------------|
| `<name>` | Yes | Content type name to delete |
| `--yes` / `-y` | Yes | Skip confirmation (REQUIRED in non-interactive mode) |

**Output Structure:**
```json
{
  "deleted": ["tutorials"],
  "message": "Content type 'tutorials' deleted successfully"
}
```

**Artifact Type:** `Text`

---

## Templates Commands

### web templates list

List all configured templates from the templates configuration.

```bash
sp web templates list
sp --json web templates list
```

**Output Structure:**
```json
{
  "templates": [
    {
      "name": "blog-post",
      "content_types": ["blog"],
      "file_exists": true,
      "file_path": "/var/www/html/tyingshoelaces/services/web/templates/blog-post.html"
    }
  ]
}
```

**Artifact Type:** `Table`
**Columns:** `name`, `content_types`, `file_exists`, `file_path`

---

### web templates show

Display detailed information for a specific template.

```bash
sp web templates show <name>
sp --json web templates show blog-post
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<name>` | Yes | Template name to show |

**Output Structure:**
```json
{
  "name": "blog-post",
  "content_types": ["blog"],
  "file_path": "/var/www/html/tyingshoelaces/services/web/templates/blog-post.html",
  "file_exists": true,
  "variables": ["TITLE", "CONTENT", "DATE", "AUTHOR"],
  "preview_lines": [
    "<!DOCTYPE html>",
    "<html>",
    "<head>",
    "  <title>{{TITLE}}</title>",
    "</head>"
  ]
}
```

**Artifact Type:** `Card`

---

### web templates create

Create a new template configuration.

```bash
sp web templates create \
  --name "tutorial" \
  --content-types "tutorials,guides"

echo "<html>{{CONTENT}}</html>" | sp web templates create \
  --name "tutorial" \
  --content-types "tutorials" \
  --content -

sp web templates create \
  --name "tutorial" \
  --content-types "tutorials" \
  --content /path/to/template.html
```

**Required Flags (non-interactive):**
| Flag | Required | Description |
|------|----------|-------------|
| `--name` | Yes | Template name (lowercase alphanumeric + hyphens) |
| `--content-types` | Yes | Content types to link (comma-separated) |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--content` | None | HTML content: use `-` for stdin, path for file, or literal string |

**Validation Rules:**
- Name: 2+ characters, lowercase alphanumeric with hyphens only
- At least one content type required

**Output Structure:**
```json
{
  "name": "tutorial",
  "file_path": "/var/www/html/tyingshoelaces/services/web/templates/tutorial.html",
  "message": "Template 'tutorial' created with HTML file at /path/to/tutorial.html"
}
```

**Artifact Type:** `Text`

---

### web templates edit

Edit an existing template configuration.

```bash
sp web templates edit <name> --add-content-type guides
sp web templates edit <name> --remove-content-type blog
sp web templates edit <name> --content-types "tutorials,guides"

echo "<html>{{NEW_CONTENT}}</html>" | sp web templates edit <name> --content -
sp web templates edit <name> --content /path/to/new-template.html
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<name>` | Yes | Template name to edit |
| At least one change | Yes | Must specify at least one modification |

**Modification Flags:**
| Flag | Description |
|------|-------------|
| `--add-content-type` | Add content type to template |
| `--remove-content-type` | Remove content type from template |
| `--content-types` | Replace all content types (comma-separated) |
| `--content` | Replace HTML content: use `-` for stdin, path for file |

**Output Structure:**
```json
{
  "name": "tutorial",
  "message": "Template 'tutorial' updated successfully with 2 change(s)",
  "changes": [
    "added content_type: guides",
    "updated HTML file: /path/to/tutorial.html"
  ]
}
```

**Artifact Type:** `Text`

---

### web templates delete

Delete a template configuration.

```bash
sp web templates delete <name> --yes
sp web templates delete <name> --yes --delete-file
```

**Required Flags (non-interactive):**
| Flag | Required | Description |
|------|----------|-------------|
| `<name>` | Yes | Template name to delete |
| `--yes` / `-y` | Yes | Skip confirmation (REQUIRED in non-interactive mode) |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--delete-file` | `false` | Also delete the .html file |

**Output Structure:**
```json
{
  "deleted": "tutorial",
  "file_deleted": true,
  "message": "Template 'tutorial' deleted (including HTML file)"
}
```

**Artifact Type:** `Text`

---

## Assets Commands

### web assets list

List all assets from the web assets directory.

```bash
sp web assets list
sp --json web assets list
sp web assets list --asset-type css
sp web assets list --asset-type logo
sp web assets list --asset-type favicon
sp web assets list --asset-type font
sp web assets list --asset-type image
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--asset-type` | `all` | Filter by asset type: `all`, `css`, `logo`, `favicon`, `font`, `image` |

**Asset Type Detection:**
- `favicon`: Files starting with "favicon"
- `logo`: Files in `logos/` directory or containing "logo" in filename
- `css`: Files with `.css` extension
- `font`: Files with `.ttf`, `.woff`, `.woff2`, `.otf`, `.eot` extensions
- `image`: Files with `.png`, `.jpg`, `.jpeg`, `.gif`, `.webp`, `.svg`, `.ico` extensions
- `other`: Everything else

**Output Structure:**
```json
{
  "assets": [
    {
      "path": "logos/logo.svg",
      "asset_type": "logo",
      "size_bytes": 2048,
      "modified": "2024-01-15T10:30:00Z"
    }
  ]
}
```

**Artifact Type:** `Table`
**Columns:** `path`, `asset_type`, `size_bytes`, `modified`

---

### web assets show

Display detailed information for a specific asset.

```bash
sp web assets show <path>
sp --json web assets show logos/logo.svg
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<path>` | Yes | Asset path relative to assets directory |

**Output Structure:**
```json
{
  "path": "logos/logo.svg",
  "absolute_path": "/var/www/html/tyingshoelaces/services/web/assets/logos/logo.svg",
  "asset_type": "logo",
  "size_bytes": 2048,
  "modified": "2024-01-15T10:30:00Z",
  "referenced_in": [
    "web config: /path/to/config.yaml",
    "metadata: /path/to/metadata.yaml"
  ]
}
```

**Artifact Type:** `Card`

---

## Sitemap Commands

### web sitemap show

Show sitemap configuration and routes from all enabled content types.

```bash
sp web sitemap show
sp --json web sitemap show
sp web sitemap show --preview
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--preview` | `false` | Show XML preview output |

**Output Structure:**
```json
{
  "routes": [
    {
      "url": "/blog",
      "priority": 0.9,
      "changefreq": "daily",
      "source": "blog (parent)"
    },
    {
      "url": "/blog/{slug}",
      "priority": 0.8,
      "changefreq": "weekly",
      "source": "blog"
    }
  ],
  "total_routes": 2
}
```

**Artifact Type:** `Table`
**Columns:** `url`, `priority`, `changefreq`, `source`

---

### web sitemap generate

Generate a sitemap.xml file from content configuration.

```bash
sp web sitemap generate
sp --json web sitemap generate
sp web sitemap generate --output /path/to/sitemap.xml
sp web sitemap generate --base-url https://example.com
sp web sitemap generate --include-dynamic
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--output` | `{web_path}/dist/sitemap.xml` | Output path for sitemap.xml |
| `--base-url` | From metadata or `https://example.com` | Base URL for sitemap entries |
| `--include-dynamic` | `false` | Include dynamic content from database |

**Output Structure:**
```json
{
  "output_path": "/var/www/html/tyingshoelaces/services/web/dist/sitemap.xml",
  "routes_count": 5,
  "message": "Sitemap generated with 5 URLs at /path/to/sitemap.xml"
}
```

**Artifact Type:** `Text`

---

## Validate Command

### web validate

Validate web configuration for errors and warnings.

```bash
sp web validate
sp --json web validate
sp web validate --only config
sp web validate --only templates
sp web validate --only assets
sp web validate --only sitemap
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--only` | `all` | Only check specific category: `all`, `config`, `templates`, `assets`, `sitemap` |

**Validation Checks:**

| Category | Checks |
|----------|--------|
| `config` | Web config exists and parses, content config exists and parses, web path exists, templates/assets directories exist |
| `templates` | templates.yaml parses, each template has HTML file, content types exist, no orphaned content types |
| `assets` | Referenced logos exist, favicon exists |
| `sitemap` | Priority values 0.0-1.0, valid changefreq values, URL patterns start with `/`, parent routes valid |

**Output Structure:**
```json
{
  "valid": true,
  "errors": [],
  "warnings": [
    {
      "category": "templates",
      "message": "Content type 'tutorials' has no associated template",
      "suggestion": "Link a template to this content type"
    }
  ]
}
```

**Severity Levels:**
- `errors` - Configuration is invalid, functionality will not work
- `warnings` - Configuration issue but may still work

**Artifact Type:** `Table`

---

## Jobs Integration

The web CLI integrates with scheduled jobs for content ingestion and publishing.

### Available Jobs

| Job | Description |
|-----|-------------|
| `content_ingestion` | Ingests markdown content from configured directories into the database |
| `publish_content` | Full publishing pipeline: images, ingestion, prerender, sitemap, CSS |

### List Available Jobs

```bash
sp infra jobs list
sp --json jobs list
```

### Run Content Ingestion

Ingests markdown files from the content path into the database:

```bash
sp infra jobs run content_ingestion
```

This job:
1. Scans content directories defined in content config
2. Parses markdown frontmatter
3. Creates/updates content records in the database
4. Links content to appropriate content types

### Run Full Publish Pipeline

Executes the complete publishing workflow:

```bash
sp infra jobs run publish_content
```

This job performs:
1. Image optimization and processing
2. Content ingestion from markdown files
3. Pre-rendering HTML pages
4. Sitemap generation
5. CSS compilation and optimization

---

## Complete End-to-End Flow

This flow demonstrates creating a new content type with a template, adding content, running ingestion jobs, generating the sitemap, and verifying the published URL.

### Phase 1: Validate Current State

```bash
sp --json web validate
sp --json web content-types list
sp --json web templates list
sp infra jobs list
```

### Phase 2: Create New Content Type

```bash
sp web content-types create \
  --name "tutorials" \
  --path "content/tutorials" \
  --source-id "tutorials" \
  --category-id "learning" \
  --description "Step-by-step programming tutorials" \
  --url-pattern "/tutorials/{slug}" \
  --priority 0.7 \
  --changefreq weekly \
  --enabled

sp --json web content-types show tutorials
```

### Phase 3: Create Template for Content Type

```bash
cat << 'EOF' | sp web templates create --name "tutorial" --content-types "tutorials" --content -
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>{{TITLE}} | Tutorials</title>
    <meta name="description" content="{{DESCRIPTION}}">
</head>
<body>
    <article>
        <h1>{{TITLE}}</h1>
        <time>{{DATE}}</time>
        <div class="content">{{CONTENT}}</div>
    </article>
</body>
</html>
EOF

sp --json web templates show tutorial
```

### Phase 4: Create Content Markdown File

Create the markdown content file in the configured path.

**Required Frontmatter Fields:**
| Field | Type | Description |
|-------|------|-------------|
| `title` | String | Content title |
| `slug` | String | URL-safe identifier (lowercase, hyphens) |
| `description` | String | Short description for SEO/previews |
| `kind` | String | Content kind (e.g., `article`, `page`) |
| `public` | Boolean | Whether content is publicly visible |
| `tags` | Array | List of tags for categorization |
| `published_at` | DateTime | ISO 8601 date when content was published |
| `updated_at` | DateTime | ISO 8601 date when content was last updated |

**Optional Fields:**
| Field | Type | Description |
|-------|------|-------------|
| `author` | String | Content author name |
| `image` | String | Featured image path |

```bash
mkdir -p /var/www/html/tyingshoelaces/services/content/tutorials

cat << 'EOF' > /var/www/html/tyingshoelaces/services/content/tutorials/getting-started-rust.md
---
title: Getting Started with Rust
slug: getting-started-rust
description: Learn the basics of Rust programming
kind: article
public: true
tags:
  - rust
  - programming
  - tutorial
published_at: 2024-01-15T10:00:00Z
updated_at: 2024-01-15T10:00:00Z
author: Developer
---

# Introduction

Rust is a systems programming language focused on safety, speed, and concurrency.

## Installation

Install Rust using rustup:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## Hello World

```rust
fn main() {
    println!("Hello, world!");
}
```
EOF
```

### Phase 5: Run Content Ingestion

Ingest the markdown content into the database:

```bash
sp infra jobs run content_ingestion

sp core content list --source tutorials
sp --json content show getting-started-rust --source tutorials
```

### Phase 6: Validate Configuration

```bash
sp --json web validate
sp --json web validate --only templates
sp --json web validate --only sitemap
```

### Phase 7: Run Full Publish Pipeline

Execute the complete publishing workflow:

```bash
sp infra jobs run publish_content
```

This will:
- Process images
- Re-ingest content
- Pre-render HTML pages
- Generate sitemap
- Compile CSS

### Phase 8: Generate Sitemap (Manual)

If you only need the sitemap without full publishing:

```bash
sp web sitemap show
sp web sitemap show --preview
sp web sitemap generate --base-url https://example.com
```

Verify the generated sitemap:

```bash
cat /var/www/html/tyingshoelaces/services/web/dist/sitemap.xml
```

### Phase 9: Verify Published URL

```bash
sp --json web sitemap show | jq '.routes[] | select(.source == "tutorials")'

sp --json content show getting-started-rust --source tutorials | jq '.slug, .title'

curl -s "https://example.com/tutorials/getting-started-rust" | head -20
```

### Phase 10: Cleanup (Optional)

```bash
sp core content delete getting-started-rust --source tutorials --yes
rm /var/www/html/tyingshoelaces/services/content/tutorials/getting-started-rust.md
sp web templates delete tutorial --yes --delete-file
sp web content-types delete tutorials --yes
sp --json web validate
```

---

## Integration with Content CLI and Jobs

The web CLI works alongside the content CLI and jobs for full content management:

| Task | Command |
|------|---------|
| Create content type schema | `sp web content-types create` |
| Create content template | `sp web templates create` |
| Create actual content | `sp core content create --source <type>` |
| List content | `sp core content list --source <type>` |
| Show content | `sp core content show <slug> --source <type>` |
| Delete content | `sp core content delete <slug> --source <type> --yes` |
| Index content | `sp core content index --source <type>` |
| Ingest from markdown | `sp infra jobs run content_ingestion` |
| Full publish pipeline | `sp infra jobs run publish_content` |
| Generate sitemap | `sp infra jobs run publish_content` (preferred) or `sp web sitemap generate` |
| Validate all | `sp web validate` |

### Workflow Diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         CONTENT PUBLISHING WORKFLOW                          │
└─────────────────────────────────────────────────────────────────────────────┘

1. SETUP PHASE
   ┌──────────────────┐    ┌──────────────────┐    ┌──────────────────┐
   │ content-types    │───▶│ templates        │───▶│ web validate     │
   │ create           │    │ create           │    │                  │
   └──────────────────┘    └──────────────────┘    └──────────────────┘

2. CONTENT CREATION
   ┌──────────────────┐    ┌──────────────────┐
   │ Create markdown  │───▶│ jobs run         │
   │ files in path    │    │ content_ingestion│
   └──────────────────┘    └──────────────────┘
         OR
   ┌──────────────────┐
   │ content create   │
   │ (via CLI)        │
   └──────────────────┘

3. PUBLISH PHASE
   ┌──────────────────┐    ┌──────────────────┐    ┌──────────────────┐
   │ jobs run         │───▶│ sitemap          │───▶│ Verify URLs      │
   │ publish_content  │    │ generate         │    │ are accessible   │
   └──────────────────┘    └──────────────────┘    └──────────────────┘
```

---

## Output Type Summary

| Command | Return Type | Artifact Type | Metadata |
|---------|-------------|---------------|----------|
| `content-types list` | `ContentTypeListOutput` | `Table` | columns |
| `content-types show` | `ContentTypeDetailOutput` | `Card` | title |
| `content-types create` | `ContentTypeCreateOutput` | `Text` | title |
| `content-types edit` | `ContentTypeEditOutput` | `Text` | title |
| `content-types delete` | `ContentTypeDeleteOutput` | `Text` | title |
| `templates list` | `TemplateListOutput` | `Table` | columns |
| `templates show` | `TemplateDetailOutput` | `Card` | title |
| `templates create` | `TemplateCreateOutput` | `Text` | title |
| `templates edit` | `TemplateEditOutput` | `Text` | title |
| `templates delete` | `TemplateDeleteOutput` | `Text` | title |
| `assets list` | `AssetListOutput` | `Table` | columns |
| `assets show` | `AssetDetailOutput` | `Card` | title |
| `sitemap show` | `SitemapShowOutput` | `Table` | columns |
| `sitemap generate` | `SitemapGenerateOutput` | `Text` | title |
| `validate` | `ValidationOutput` | `Table` | - |

---

## Error Handling

### Missing Required Flags

```bash
sp web content-types show
# Error: --name is required in non-interactive mode

sp web content-types delete tutorials
# Error: --yes is required to delete content types in non-interactive mode

sp web content-types create --name test
# Error: --path is required in non-interactive mode

sp web templates create --name test
# Error: --content-types is required in non-interactive mode
```

### Validation Errors

```bash
sp web content-types create --name "Test Type" --path x --source-id x --category-id x
# Error: Name must be lowercase alphanumeric with hyphens only

sp web content-types edit blog --priority 1.5
# Error: Priority must be between 0.0 and 1.0

sp web content-types edit blog --url-pattern "/new"
# Error: Content type 'blog' has no sitemap configuration. Create sitemap config first.
```

### Not Found Errors

```bash
sp web content-types show nonexistent
# Error: Content type 'nonexistent' not found

sp web templates show nonexistent
# Error: Template 'nonexistent' not found

sp web assets show nonexistent.css
# Error: Asset 'nonexistent.css' not found
```

### Configuration Errors

```bash
sp web validate
# Error: Web config not found at /path/to/config.yaml

sp web sitemap generate
# Error: Failed to read content config at /path/to/config.yaml
```

---

## JSON Output

All commands support `--json` flag for structured output:

```bash
sp --json web content-types list | jq '.content_types[].name'

sp --json web content-types show blog | jq '.sitemap.url_pattern'

sp --json web templates list | jq '.templates[] | select(.file_exists == true)'

sp --json web assets list | jq '.assets[] | select(.asset_type == "logo")'

sp --json web sitemap show | jq '.routes[] | select(.priority > 0.7)'

sp --json web validate | jq '.errors[]'

sp --json web validate | jq 'if .valid then "Configuration OK" else "Issues found" end'
```

---

## Compliance Checklist

- [x] All `execute` functions accept `config: &CliConfig`
- [x] All commands return `CommandResult<T>` with proper artifact type
- [x] `delete` commands require `--yes` / `-y` flag
- [x] All output types derive `Serialize`, `Deserialize`, `JsonSchema`
- [x] No `println!` / `eprintln!` - uses `CliService`
- [x] No `unwrap()` / `expect()` - uses `?` with `.context()`
- [x] `resolve_input` pattern used for interactive/non-interactive selection
- [x] JSON output supported via `--json` flag
- [x] Proper error messages for missing required flags
- [x] No inline comments per Rust standards
