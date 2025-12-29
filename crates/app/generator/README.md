# systemprompt-generator

Static site generation module. Generates prerendered HTML pages, sitemaps, and optimized assets for the web frontend.

## Structure

```
src/
├── lib.rs                    # Public exports
├── api.rs                    # API helpers
├── assets.rs                 # Asset handling
├── cards.rs                  # Card generation
├── images.rs                 # Image optimization
├── markdown.rs               # Markdown rendering
├── prerender.rs              # Page prerendering
├── prerender_index.rs        # Index page generation
├── prerender_parent.rs       # Parent route rendering
├── sitemap.rs                # Sitemap generation
├── sitemap_xml.rs            # XML output
├── templates.rs              # Template engine
├── templates_*.rs            # Template helpers
├── web_build.rs              # Build orchestration
├── web_build_steps.rs        # Build pipeline steps
├── web_build_validation.rs   # Output validation
└── jobs/
    ├── mod.rs                # Job exports
    └── publish_content.rs    # Content publishing job
```

## Key Features

| Feature | Description |
|---------|-------------|
| Prerendering | Generates static HTML for all content pages |
| Sitemap | Generates XML sitemaps for SEO |
| Image Optimization | Converts images to WebP format |
| Template Engine | Handlebars-based templating |
| Build Pipeline | TypeScript, Vite, CSS organization |

## Dependencies

| Crate | Purpose |
|-------|---------|
| `systemprompt-core-database` | Database pool |
| `systemprompt-core-content` | Content repository |
| `systemprompt-models` | Configuration types |
| `systemprompt-identifiers` | Typed identifiers |
