# Content Templates

This document explains the relationship between content `kind`, `content_type`, `allowed_content_types`, and templates in the content publishing system.

---

## Key Concepts

### Content Kind (Frontmatter)

Each markdown content file has a `kind` field in its frontmatter that identifies what type of content it is:

```yaml
---
title: My Blog Post
kind: article
date: 2024-01-15
image: /images/blog/my-post.jpg
---
```

Common kind values:
- `article` - Blog posts, news articles
- `tutorial` - Step-by-step guides
- `legal` - Privacy policies, terms of service
- `page` - Generic static pages
- `homepage` - Homepage content

### Content Type (Database)

When content is ingested into the database, the `kind` field becomes the `content_type` field. This is the value used during prerendering to find the appropriate template.

### Allowed Content Types (Source Config)

In `content.yaml`, each content source can specify which content types it accepts:

```yaml
content_sources:
  blog:
    path: services/content/blog
    source_id: blog
    category_id: blog
    enabled: true
    allowed_content_types:
      - article
      - tutorial
```

### Template Content Types

Templates declare which content types they can render via `templates.yaml`:

```yaml
templates:
  blog-post:
    content_types:
      - article
      - tutorial
  legal-page:
    content_types:
      - legal
      - page
  homepage:
    content_types:
      - homepage
```

---

## How Templates Are Matched

During prerendering, the system:

1. Reads content from the database with its `content_type`
2. Calls `template_registry.find_template_for_content_type(content_type)`
3. Returns the first template that lists that content_type

```
Content (content_type: "article")
        │
        ▼
TemplateRegistry.find_template_for_content_type("article")
        │
        ▼
Scans templates for one with content_types containing "article"
        │
        ▼
Finds "blog-post" template → Renders content
```

---

## Template Priority

When multiple templates can handle the same content type, priority determines which is used:

1. **Extension templates** (priority ~100) - Project-specific templates in `templates/` directory
2. **Embedded defaults** (priority 1000) - Built-in fallback templates

Lower priority numbers win. Extension templates override embedded defaults.

---

## Troubleshooting Template Errors

### Error: "No template for content type 'X'"

```json
{
  "step": "prerender",
  "success": false,
  "error": {
    "summary": "No template for content type 'legal'",
    "suggestion": "Change content type from 'legal' to one of: article, tutorial, homepage"
  }
}
```

**Cause**: No template in your project handles the content type.

**Solutions**:

1. **Add a template** - Create a template that handles this content type:
   ```yaml
   # templates/templates.yaml
   templates:
     legal-page:
       content_types:
         - legal
         - page
   ```

2. **Change the content kind** - Update the frontmatter to use a supported type:
   ```yaml
   ---
   title: Privacy Policy
   kind: page  # Changed from 'legal' to 'page'
   ---
   ```

3. **Update allowed_content_types** - Ensure the source config allows this type:
   ```yaml
   content_sources:
     legal:
       allowed_content_types:
         - legal
         - page
   ```

### Error: "Missing field 'image' for content 'X'"

**Cause**: Content is missing a required field.

**Solutions**:

1. **Add the field** - Update frontmatter:
   ```yaml
   ---
   title: My Post
   image: /files/images/placeholder.svg
   ---
   ```

2. **Use empty image gracefully** - As of recent updates, empty images default to a placeholder. If you're seeing this error, upgrade to the latest version.

---

## Configuration Files Reference

### content.yaml

Defines content sources and their allowed types:

```yaml
content_sources:
  blog:
    path: services/content/blog
    source_id: blog
    category_id: blog
    enabled: true
    description: Blog posts
    allowed_content_types:
      - article
      - tutorial
    sitemap:
      enabled: true
      url_pattern: /blog/{slug}
      changefreq: weekly
      priority: 0.8
    branding:
      name: Our Blog
      description: Latest articles
      image: /images/blog-og.jpg
      keywords:
        - technology
        - tutorials

  legal:
    path: services/content/legal
    source_id: legal
    category_id: legal
    enabled: true
    allowed_content_types:
      - legal
      - page
    sitemap:
      enabled: true
      url_pattern: /{slug}
```

### templates.yaml

Defines available templates and which content types they handle:

```yaml
templates:
  blog-post:
    content_types:
      - article
      - tutorial

  legal-page:
    content_types:
      - legal
      - page

  homepage:
    content_types:
      - homepage
```

### web.yaml (Branding)

Required branding fields for content publishing:

```yaml
branding:
  copyright: "© 2024 Your Company. All rights reserved."
  twitter_handle: "@yourhandle"
  display_sitename: true
  favicon: /favicon.ico
  logo:
    primary:
      svg: /images/logo.svg
```

---

## Common Kind/Type Mappings

| Frontmatter `kind` | Use Case | Typical Template |
|-------------------|----------|------------------|
| `article` | Blog posts, news | blog-post |
| `tutorial` | How-to guides | blog-post or tutorial |
| `legal` | Privacy, terms | legal-page |
| `page` | Static pages | legal-page or page |
| `homepage` | Site homepage | homepage |
| `landing` | Marketing pages | landing-page |
| `documentation` | Docs, references | docs-page |

---

## Creating a New Content Type

1. **Define the kind in frontmatter**:
   ```yaml
   ---
   title: New Case Study
   kind: case-study
   ---
   ```

2. **Add to allowed_content_types in content.yaml**:
   ```yaml
   content_sources:
     case-studies:
       allowed_content_types:
         - case-study
   ```

3. **Create a template in templates.yaml**:
   ```yaml
   templates:
     case-study-page:
       content_types:
         - case-study
   ```

4. **Create the template file** (`templates/case-study-page.html`):
   ```html
   <!DOCTYPE html>
   <html>
   <head><title>{{title}}</title></head>
   <body>
     <h1>{{title}}</h1>
     <div class="case-study">
       {{{content}}}
     </div>
   </body>
   </html>
   ```

---

## Embedded Default Templates

The system includes these embedded default templates:

| Template | Content Types | Purpose |
|----------|---------------|---------|
| `homepage` | homepage | Basic homepage layout |

Projects should provide their own templates for other content types. Embedded defaults serve as fallbacks and examples.

---

## Validation Errors

The system now validates branding configuration on startup. If you see validation errors like:

```
web_config.branding.copyright: Missing required field 'copyright'
  Suggestion: Add 'copyright: "© 2024 Your Company"' under branding
```

These indicate missing required fields in `web.yaml`. Fix them before running `content publish`.

Required branding fields:
- `branding.copyright`
- `branding.twitter_handle`
- `branding.display_sitename`
- `branding.favicon`
- `branding.logo.primary.svg`
