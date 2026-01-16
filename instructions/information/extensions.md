# Extension System Guide

Technical guide for extending SystemPrompt with custom templates, components, and data providers.

---

## Crate Dependencies

Extension implementations depend on `systemprompt-provider-contracts` for provider trait definitions:

```
systemprompt-provider-contracts
├── LlmProvider, ToolProvider      <- AI/MCP providers
├── Job, JobContext, JobResult     <- Background jobs
├── ComponentRenderer              <- Template components
├── TemplateDataExtender           <- Data extenders
├── PageDataProvider               <- Page data providers
└── TemplateProvider               <- Template definitions
```

Import via the extension prelude:

```rust
use systemprompt::extension::prelude::*;
```

The prelude re-exports all provider contracts from `systemprompt-provider-contracts`.

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Extension Project                             │
│                                                                      │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────┐  │
│  │ Templates       │  │ Components      │  │ Data Providers      │  │
│  │ services/web/   │  │ ComponentRender │  │ PageDataProvider    │  │
│  │ templates/      │  │ trait impl      │  │ TemplateDataExtender│  │
│  └────────┬────────┘  └────────┬────────┘  └──────────┬──────────┘  │
│           │                    │                      │              │
│           └────────────────────┼──────────────────────┘              │
│                                │                                      │
│                    ┌───────────▼────────────┐                        │
│                    │ Extension trait impl   │                        │
│                    │ register_extension!()  │                        │
│                    └───────────┬────────────┘                        │
└────────────────────────────────┼─────────────────────────────────────┘
                                 │ inventory collects at compile time
                    ┌────────────▼────────────┐
                    │ TemplateRegistry        │
                    │ - providers             │
                    │ - loaders               │
                    │ - components            │
                    │ - page_providers        │
                    │ - extenders             │
                    └─────────────────────────┘
```

---

## Template System

### Directory Structure

Extensions define templates in `services/web/templates/`:

```
services/web/
  templates/
    homepage.html          <- Homepage template
    blog-post.html         <- Content type template
    blog-list.html         <- Parent route template
    partials/
      header.html          <- Shared partials
      footer.html
  web.yaml                 <- Template configuration
```

### Template Definition

Templates are discovered via `template.yaml` files:

```yaml
name: blog
priority: 500
content_types:
  - blog
  - article
source:
  type: file
  path: blog-post.html
```

| Field | Purpose |
|-------|---------|
| `name` | Template identifier |
| `priority` | Lower wins (500 = extension, 1000 = core default) |
| `content_types` | Content types this template handles |
| `source.path` | Path relative to templates directory |

### Template Variables

Templates receive data via Handlebars context:

```handlebars
<html>
<head>
    <title>{{title}}</title>
    <meta name="description" content="{{description}}">
    <link rel="stylesheet" href="{{CSS_BASE_PATH}}/main.css">
</head>
<body>
    <header>
        <img src="{{LOGO_PATH}}" alt="{{ORG_NAME}}">
    </header>

    <main>
        {{{CONTENT_HTML}}}
    </main>

    <aside>
        {{{POPULAR_ITEMS_HTML}}}
    </aside>

    <footer>
        {{{FOOTER_NAV}}}
    </footer>

    <script src="{{JS_BASE_PATH}}/main.js"></script>
</body>
</html>
```

### Standard Variables

| Variable | Source | Description |
|----------|--------|-------------|
| `site` | web.yaml | Full site configuration |
| `title` | Content item | Page title |
| `description` | Content item | Meta description |
| `CONTENT_HTML` | Rendered markdown | Main content body |
| `ORG_NAME` | content.yaml | Organization name |
| `ORG_URL` | content.yaml | Organization URL |
| `LOGO_PATH` | web.yaml | Logo file path |
| `FAVICON_PATH` | web.yaml | Favicon path |
| `JS_BASE_PATH` | Generated | JavaScript directory |
| `CSS_BASE_PATH` | Generated | CSS directory |
| `FOOTER_NAV` | Generated | Footer navigation HTML |

---

## Component Renderers

Components inject dynamic HTML into templates.

### Implementing ComponentRenderer

```rust
use std::sync::Arc;
use anyhow::Result;
use async_trait::async_trait;
use systemprompt::extension::prelude::*;

pub struct PopularItemsComponent;

#[async_trait]
impl ComponentRenderer for PopularItemsComponent {
    fn component_id(&self) -> &str {
        "popular-items"
    }

    fn variable_name(&self) -> &str {
        "POPULAR_ITEMS_HTML"
    }

    fn applies_to(&self) -> Vec<String> {
        vec!["blog".to_string(), "homepage".to_string()]
    }

    async fn render(&self, ctx: &ComponentContext<'_>) -> Result<RenderedComponent> {
        let html = match (ctx.all_items, ctx.popular_ids) {
            (Some(items), Some(popular_ids)) => {
                let popular: Vec<_> = items
                    .iter()
                    .filter(|item| {
                        item.get("id")
                            .and_then(|id| id.as_str())
                            .map(|id| popular_ids.contains(&id.to_string()))
                            .unwrap_or(false)
                    })
                    .take(5)
                    .collect();

                render_popular_list(&popular)
            }
            _ => String::new(),
        };

        Ok(RenderedComponent::new(self.variable_name(), html))
    }

    fn priority(&self) -> u32 {
        100
    }
}

fn render_popular_list(items: &[&serde_json::Value]) -> String {
    let mut html = String::from("<ul class=\"popular-items\">");
    for item in items {
        let title = item.get("title").and_then(|v| v.as_str()).unwrap_or("");
        let slug = item.get("slug").and_then(|v| v.as_str()).unwrap_or("");
        html.push_str(&format!(
            "<li><a href=\"/blog/{}\">{}</a></li>",
            slug, title
        ));
    }
    html.push_str("</ul>");
    html
}
```

### ComponentContext

```rust
pub struct ComponentContext<'a> {
    pub web_config: &'a serde_yaml::Value,
    pub item: Option<&'a Value>,
    pub all_items: Option<&'a [Value]>,
    pub popular_ids: Option<&'a [String]>,
}
```

| Field | Available For | Description |
|-------|---------------|-------------|
| `web_config` | All pages | Site configuration |
| `item` | Content pages | Current content item |
| `all_items` | Content pages | All items in source |
| `popular_ids` | Content pages | IDs of popular content |

Use `ComponentContext::for_page()` for static pages (homepage, about).
Use `ComponentContext::for_content()` for content pages (blog posts).

---

## Page Data Providers

Providers inject dynamic data into static pages.

### Implementing PageDataProvider

```rust
use std::sync::Arc;
use anyhow::Result;
use async_trait::async_trait;
use systemprompt::extension::prelude::*;
use systemprompt_core_database::DbPool;

pub struct FeaturedPostsProvider;

#[async_trait]
impl PageDataProvider for FeaturedPostsProvider {
    fn provider_id(&self) -> &str {
        "featured-posts"
    }

    fn applies_to_pages(&self) -> Vec<String> {
        vec!["homepage".to_string()]
    }

    async fn provide_page_data(&self, ctx: &PageContext<'_>) -> Result<serde_json::Value> {
        let db_pool = ctx.db_pool::<DbPool>()
            .ok_or_else(|| anyhow::anyhow!("Database pool not available"))?;

        let repo = ContentRepository::new(db_pool)?;
        let featured = repo.get_featured(3).await?;

        Ok(serde_json::json!({
            "featured_posts": featured
                .iter()
                .map(|p| serde_json::json!({
                    "title": p.title,
                    "slug": p.slug,
                    "description": p.description
                }))
                .collect::<Vec<_>>()
        }))
    }

    fn priority(&self) -> u32 {
        100
    }
}
```

### PageContext

```rust
pub struct PageContext<'a> {
    pub page_type: &'a str,
    pub web_config: &'a serde_yaml::Value,
}

impl PageContext {
    pub fn db_pool<T: 'static>(&self) -> Option<&T>;
}
```

The `db_pool()` method uses type erasure to access the database pool.

---

## Template Data Extenders

Extenders modify template data before rendering.

### Implementing TemplateDataExtender

```rust
use std::sync::Arc;
use anyhow::Result;
use async_trait::async_trait;
use systemprompt::extension::prelude::*;

pub struct RelatedPostsExtender;

#[async_trait]
impl TemplateDataExtender for RelatedPostsExtender {
    fn extender_id(&self) -> &str {
        "related-posts"
    }

    fn applies_to(&self) -> Vec<String> {
        vec!["blog".to_string()]
    }

    async fn extend(&self, ctx: &ExtenderContext<'_>) -> Result<ExtendedData> {
        let category = ctx.item
            .get("category_id")
            .and_then(|v| v.as_str());

        let related = match category {
            Some(cat) => find_related_posts(ctx.all_items, cat, ctx.item),
            None => vec![],
        };

        Ok(ExtendedData::new()
            .with_value("related_posts", serde_json::to_value(&related)?))
    }

    fn priority(&self) -> u32 {
        100
    }
}

fn find_related_posts(
    items: &[serde_json::Value],
    category: &str,
    current: &serde_json::Value,
) -> Vec<serde_json::Value> {
    items
        .iter()
        .filter(|item| {
            item.get("category_id")
                .and_then(|v| v.as_str())
                .map(|c| c == category)
                .unwrap_or(false)
        })
        .filter(|item| item.get("id") != current.get("id"))
        .take(3)
        .cloned()
        .collect()
}
```

---

## Registering Extensions

### Extension Implementation

```rust
use std::sync::Arc;
use systemprompt::extension::prelude::*;

pub struct BlogExtension;

impl Extension for BlogExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "blog",
            name: "Blog Extension",
            version: "0.1.0",
        }
    }

    fn component_renderers(&self) -> Vec<Arc<dyn ComponentRenderer>> {
        vec![
            Arc::new(PopularItemsComponent),
            Arc::new(TableOfContentsComponent),
        ]
    }

    fn template_data_extenders(&self) -> Vec<Arc<dyn TemplateDataExtender>> {
        vec![
            Arc::new(RelatedPostsExtender),
        ]
    }

    fn page_data_providers(&self) -> Vec<Arc<dyn PageDataProvider>> {
        vec![
            Arc::new(FeaturedPostsProvider),
        ]
    }
}

register_extension!(BlogExtension);
```

### Extension Discovery

Extensions register via the `inventory` crate. At runtime:

```rust
let extensions = ExtensionRegistry::discover();
for ext in extensions.extensions() {
    for component in ext.component_renderers() {
        registry_builder = registry_builder.with_component(component);
    }
    for extender in ext.template_data_extenders() {
        registry_builder = registry_builder.with_extender(extender);
    }
    for provider in ext.page_data_providers() {
        registry_builder = registry_builder.with_page_provider(provider);
    }
}
```

---

## Template Priority System

Templates resolve by priority (lower wins):

| Priority | Source |
|----------|--------|
| 500 | Extension templates |
| 1000 | Core default templates |

This allows extensions to override core templates.

```rust
let extension_provider = CoreTemplateProvider::discover_with_priority(
    &extension_path,
    CoreTemplateProvider::EXTENSION_PRIORITY,  // 500
).await?;

let core_provider = CoreTemplateProvider::discover_with_priority(
    &core_path,
    CoreTemplateProvider::DEFAULT_PRIORITY,    // 1000
).await?;
```

---

## Static Assets

### Scripts and Styles

Place assets in `services/web/`:

```
services/web/
  js/
    main.js
    analytics.js
  css/
    main.css
    theme.css
  assets/
    logo.svg
    favicon.ico
```

Reference in templates using base path variables:

```handlebars
<link rel="stylesheet" href="{{CSS_BASE_PATH}}/main.css">
<script src="{{JS_BASE_PATH}}/main.js"></script>
<img src="/assets/logo.svg" alt="Logo">
```

### Favicon and Branding

Configure in `services/web/web.yaml`:

```yaml
branding:
  logo:
    primary:
      svg: /assets/logo.svg
      png: /assets/logo.png
  favicon: /assets/favicon.ico
  twitter_handle: "@mycompany"
  display_sitename: true
```

---

## Homepage Implementation

### Template Structure

```handlebars
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>{{site.title}}</title>
    <meta name="description" content="{{site.description}}">
    <link rel="icon" href="{{FAVICON_PATH}}">
    <link rel="stylesheet" href="{{CSS_BASE_PATH}}/main.css">
</head>
<body>
    <header>
        {{#if DISPLAY_SITENAME}}
        <h1>{{site.title}}</h1>
        {{/if}}
        <img src="{{LOGO_PATH}}" alt="{{ORG_NAME}}">
    </header>

    <main>
        {{#each featured_posts}}
        <article>
            <h2><a href="/blog/{{slug}}">{{title}}</a></h2>
            <p>{{description}}</p>
        </article>
        {{/each}}
    </main>

    {{{FOOTER_NAV}}}

    <script src="{{JS_BASE_PATH}}/main.js"></script>
</body>
</html>
```

### Homepage Data Flow

```
1. load_prerender_context()
   ├── Load config files
   ├── Discover templates
   └── Register extension providers

2. prerender_homepage()
   ├── Build base data (branding, nav)
   ├── Call PageDataProviders
   │   └── Each provider adds data (featured_posts, etc.)
   ├── Call ComponentRenderers
   │   └── Each component adds HTML variables
   └── Render homepage.html template
```

---

## Content Type Implementation

### Blog Template Example

```handlebars
<!DOCTYPE html>
<html lang="en">
<head>
    <title>{{title}} | {{site.title}}</title>
    <meta name="description" content="{{description}}">
    <meta name="author" content="{{author}}">
    <meta property="og:type" content="article">
    <link rel="stylesheet" href="{{CSS_BASE_PATH}}/blog.css">
</head>
<body>
    <article>
        <header>
            <h1>{{title}}</h1>
            <time datetime="{{published_at}}">{{formatted_date}}</time>
            <span class="author">{{author}}</span>
        </header>

        <div class="content">
            {{{CONTENT_HTML}}}
        </div>

        {{#if related_posts}}
        <aside class="related">
            <h2>Related Posts</h2>
            <ul>
            {{#each related_posts}}
                <li><a href="/blog/{{slug}}">{{title}}</a></li>
            {{/each}}
            </ul>
        </aside>
        {{/if}}
    </article>

    {{{POPULAR_ITEMS_HTML}}}

    <script src="{{JS_BASE_PATH}}/blog.js"></script>
</body>
</html>
```

### Content Rendering Flow

```
1. process_all_sources()
   └── For each content source

2. process_source()
   ├── Fetch content from database
   ├── Convert to JSON
   └── Fetch popular IDs

3. render_single_item()
   ├── Render markdown to HTML
   ├── Prepare template data
   ├── Call TemplateDataExtenders
   │   └── Add related_posts, etc.
   ├── Call ComponentRenderers
   │   └── Add POPULAR_ITEMS_HTML, etc.
   └── Render template

4. write_rendered_page()
   └── Write to dist/{url_pattern}/index.html
```

---

## Configuration Files

### web.yaml

```yaml
title: "My Blog"
description: "A technical blog about software development"
language: "en"
templates_path: services/web/templates

branding:
  logo:
    primary:
      svg: /assets/logo.svg
  favicon: /assets/favicon.ico
  twitter_handle: "@myblog"
  display_sitename: true

navigation:
  footer:
    - label: "About"
      url: "/about"
    - label: "Contact"
      url: "/contact"
```

### content.yaml

```yaml
content_sources:
  blog:
    enabled: true
    source_id: "blog-posts"
    sitemap:
      enabled: true
      url_pattern: "/blog/{slug}"
      parent_route:
        enabled: true
        url_pattern: "/blog"
        template: "blog-list"

metadata:
  structured_data:
    organization:
      name: "My Company"
      url: "https://mycompany.com"
      logo: "https://mycompany.com/logo.png"
```

---

## Best Practices

### Template Organization

| Pattern | Description |
|---------|-------------|
| One template per content type | `blog-post.html`, `product.html`, `event.html` |
| Shared partials in `partials/` | `header.html`, `footer.html`, `sidebar.html` |
| Index templates with `-list` suffix | `blog-list.html` for `/blog` route |

### Component Design

| Pattern | Description |
|---------|-------------|
| Single responsibility | One component per feature |
| Graceful degradation | Return empty string if data unavailable |
| Priority ordering | Use priority to control render order |

### Data Provider Design

| Pattern | Description |
|---------|-------------|
| Page-specific filtering | Use `applies_to_pages()` to limit scope |
| Database access via downcast | Use `ctx.db_pool::<DbPool>()` |
| Return JSON objects | Provider data merges into template context |

### Error Handling

| Pattern | Description |
|---------|-------------|
| Log and continue | Components log errors but don't fail page |
| Propagate critical errors | Missing required data should fail |
| Use `context()` for error messages | Provide context for debugging |
