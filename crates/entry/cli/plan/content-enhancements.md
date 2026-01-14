# Content Domain Enhancements Plan

## Current State Analysis

The content domain handles:
- Markdown ingestion from filesystem
- Database storage with full-text search
- API endpoints for content retrieval
- Link tracking and analytics
- Prerendered static HTML serving

---

## Friction Points Identified

### 1. **No End-to-End Publishing Workflow**

**Problem:** Publishing a blog requires multiple manual steps across different commands:
```bash
# Current workflow (fragmented)
sp content ingest ./blog --source blog          # 1. Ingest to DB
# ??? - No command to trigger prerender         # 2. Manual build step
# ??? - No command to verify URL works          # 3. Manual verification
```

**Impact:** High friction for content authors, easy to forget steps.

### 2. **No URL Verification Command**

**Problem:** After publishing, no way to verify:
- Content exists at expected URL
- Correct template is applied
- Prerendered HTML was generated
- API returns correct data

**Current workaround:** Manual `curl` commands or browser inspection.

### 3. **No Content Preview Command**

**Problem:** Cannot preview content before publishing to verify:
- Frontmatter is valid
- Rendering looks correct
- Links work

### 4. **Missing `content update` Command**

**Problem:** Can only create/override via ingest. No targeted update for:
- Single field changes (title, description)
- Toggling public/private status
- Updating metadata without re-ingesting

### 5. **Hardcoded Base URL for Links**

**Problem:** `link/generate.rs` has hardcoded `https://systemprompt.io`.
Should come from profile configuration.

### 6. **No Content Status/Health Check**

**Problem:** No quick way to see:
- Which content is published vs draft
- Which content is missing prerendered HTML
- Which URLs are 404ing

### 7. **Ingest Doesn't Report What Changed**

**Problem:** `content ingest` shows files processed but not:
- Which items were created vs updated
- What fields changed
- Version hash differences

### 8. **No Bulk Operations**

**Problem:** Missing commands for:
- Bulk publish/unpublish
- Bulk re-ingest after template changes
- Export content to markdown

---

## Proposed Enhancements

### Phase 1: Publishing Workflow (High Priority)

#### 1.1 `content publish` Command

Single command to publish content end-to-end:

```bash
sp content publish getting-started --source blog
# 1. Validates frontmatter
# 2. Ingests to database (if not already)
# 3. Triggers prerender for this content
# 4. Verifies URL responds with 200
# 5. Reports success with URL
```

**Flags:**
- `--skip-prerender` - Only update database
- `--skip-verify` - Don't check URL after publish
- `--force` - Republish even if unchanged

#### 1.2 `content verify` Command

Verify content is accessible:

```bash
sp content verify getting-started --source blog
# Output:
# {
#   "content_id": "abc123",
#   "database": true,
#   "prerendered": true,
#   "url": "https://example.com/blog/getting-started",
#   "status_code": 200,
#   "template": "article",
#   "last_modified": "2024-01-15T10:30:00Z"
# }
```

**Checks:**
- Content exists in database
- Prerendered HTML exists on disk
- URL returns 200 (optional, requires API running)
- Template association is correct

#### 1.3 `content preview` Command

Preview content rendering:

```bash
sp content preview ./blog/article.md --source blog
# Shows: parsed frontmatter, rendered preview, validation warnings
```

### Phase 2: Content Management (Medium Priority)

#### 2.1 `content update` Command

Update specific fields:

```bash
sp content update getting-started --source blog --set title="New Title"
sp content update getting-started --source blog --public false
sp content update getting-started --source blog --set keywords="new,keywords"
```

#### 2.2 `content status` Command

Show content health:

```bash
sp content status --source blog
# Output table:
# | slug | db | prerender | url_status | last_updated |
# |------|-----|-----------|------------|--------------|
# | getting-started | ✓ | ✓ | 200 | 2024-01-15 |
# | draft-post | ✓ | ✗ | 404 | 2024-01-14 |
```

#### 2.3 Enhanced `content ingest` Output

Show what changed:

```bash
sp content ingest ./blog --source blog
# {
#   "created": ["new-article"],
#   "updated": ["existing-article"],
#   "unchanged": ["old-article"],
#   "errors": []
# }
```

### Phase 3: Bulk Operations (Lower Priority)

#### 3.1 `content export` Command

Export content back to markdown:

```bash
sp content export getting-started --source blog --output ./exported/
sp content export --source blog --all --output ./backup/
```

#### 3.2 `content bulk` Command

Bulk operations:

```bash
sp content bulk publish --source blog --filter "kind=article"
sp content bulk unpublish --source blog --older-than 1y
sp content bulk prerender --source blog
```

---

## Implementation Priority

| Enhancement | Priority | Complexity | Impact |
|-------------|----------|------------|--------|
| `content verify` | HIGH | Low | Immediate friction reduction |
| `content publish` | HIGH | Medium | Streamlines publishing |
| `content status` | HIGH | Low | Visibility into content health |
| `content update` | MEDIUM | Low | Common use case |
| Enhanced ingest output | MEDIUM | Low | Better feedback |
| `content preview` | MEDIUM | Medium | Author productivity |
| `content export` | LOW | Medium | Backup/migration |
| `content bulk` | LOW | Medium | Scale operations |

---

## Publishing a New Blog (Current vs Proposed)

### Current Workflow (Manual)

```bash
# 1. Create markdown file
cat << 'EOF' > ./blog/new-post.md
---
title: My New Post
slug: my-new-post
description: A new blog post
published_at: 2024-01-15
kind: article
author: Author Name
---
# My New Post
Content here...
EOF

# 2. Ingest to database
sp content ingest ./blog --source blog

# 3. Verify in database
sp content show my-new-post --source blog

# 4. Trigger prerender (MANUAL - varies by setup)
cd /path/to/web && npm run build

# 5. Verify URL manually
curl -I https://example.com/blog/my-new-post

# 6. Check template applied (MANUAL - inspect HTML)
curl https://example.com/blog/my-new-post | grep "<title>"
```

### Proposed Workflow (Automated)

```bash
# 1. Create markdown file (same)
cat << 'EOF' > ./blog/new-post.md
...
EOF

# 2. Preview before publishing
sp content preview ./blog/new-post.md --source blog

# 3. Publish with full verification
sp content publish ./blog/new-post.md --source blog
# Output:
# ✓ Frontmatter valid
# ✓ Ingested to database (created)
# ✓ Prerender triggered
# ✓ URL verified: https://example.com/blog/my-new-post (200)
# ✓ Template: article
#
# Published: my-new-post
# URL: https://example.com/blog/my-new-post

# 4. Or verify existing content
sp content verify my-new-post --source blog
```

---

## Technical Notes

### URL Pattern Resolution

Content URLs are defined in content configuration YAML:

```yaml
content_sources:
  blog:
    sitemap:
      url_pattern: "/blog/{slug}"
```

The `content verify` command would:
1. Load content config
2. Find source's url_pattern
3. Substitute `{slug}` with content slug
4. Check if prerendered HTML exists at `dist/{url_pattern}/index.html`
5. Optionally HTTP request to verify live URL

### Prerender Integration

Publishing needs to trigger prerender. Options:
1. **CLI triggers build script** - `sp content publish` runs configured build command
2. **Webhook to build service** - POST to CI/CD to trigger rebuild
3. **Incremental prerender** - Only prerender changed content (ideal)

Current prerender is likely part of Vite build. Need to expose as CLI command or API.
