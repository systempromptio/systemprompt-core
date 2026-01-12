# Services Configuration System

This document explains the relationship between **Profiles**, **Services**, and the build/deploy process. Understanding this architecture is essential for configuring and deploying SystemPrompt applications.

---

## Core Concepts

### Profiles vs Services

| Aspect | Profiles | Services |
|--------|----------|----------|
| **Purpose** | WHERE things run | WHAT runs |
| **Location** | `.systemprompt/profiles/` | `services/` |
| **Format** | Single YAML per environment | Multiple YAML files, organized by domain |
| **Contains** | Paths, URLs, database, security, rate limits | Content, agents, MCP servers, theming, skills |
| **Environment-specific** | Yes (dev, staging, prod) | No (same across environments) |
| **Runtime loading** | Via `SYSTEMPROMPT_PROFILE` env var | Via `paths.services` in profile |

**Key principle**: Profiles tell the system WHERE to find things. Services tell the system WHAT to do with them.

---

## Profile Structure

Profiles are environment-specific configuration files that define infrastructure settings.

```
.systemprompt/profiles/
├── local/
│   └── profile.yaml        # Local development
├── groot/
│   └── profile.yaml        # Production (cloud)
│   └── docker/             # Docker build files
└── staging/
    └── profile.yaml        # Staging environment
```

### Profile YAML Schema

```yaml
name: groot                           # Profile identifier
display_name: Groot                   # Human-readable name
target: cloud                         # local | cloud

site:
  name: systemprompt.io
  github_link: null

database:
  type: postgres
  external_db_access: true            # Allow external connections

server:
  host: 0.0.0.0
  port: 8080
  api_server_url: https://myapp.systemprompt.io
  api_internal_url: http://localhost:8080
  api_external_url: https://myapp.systemprompt.io
  use_https: true
  cors_allowed_origins:
    - https://myapp.systemprompt.io

paths:
  system: /app                        # Root application directory
  services: /app/services             # Services config directory
  bin: /app/bin                       # Binary directory
  web_path: /app/web                  # Web assets directory
  storage: /app/storage               # User files storage
  geoip_database: null                # Optional GeoIP database

security:
  jwt_issuer: systemprompt
  jwt_access_token_expiration: 86400
  jwt_refresh_token_expiration: 2592000
  jwt_audiences: [web, api, a2a, mcp]

rate_limits:
  disabled: false
  oauth_public_per_second: 2
  # ... per-endpoint limits

runtime:
  environment: production             # development | staging | production
  log_level: normal                   # verbose | normal | quiet
  output_format: json                 # text | json
  no_color: true
  non_interactive: true

cloud:
  credentials_path: ../../credentials.json
  tenants_path: ../../tenants.json
  tenant_id: f800de73-2bdf-4c44-8b65-7b2f2bbad456
  cli_enabled: false
  validation: strict                  # strict | warn | skip

secrets:
  secrets_path: ''                    # Empty = load from env vars
  validation: strict
  source: env                         # file | env
```

---

## Services Structure

Services contain application-specific configuration that defines WHAT the application does.

```
services/
├── config/
│   ├── config.yaml         # Master config (includes all others)
│   ├── blog.yaml           # Blog extension config
│   └── extensions.yaml     # Extension registry
├── content/
│   ├── config.yaml         # Content sources and routing
│   ├── blog/               # Blog markdown files
│   └── legal/              # Legal pages markdown
├── web/
│   ├── config.yaml         # Theming, branding, navigation
│   ├── metadata.yaml       # Site metadata for HTML injection
│   ├── templates/          # HTML templates
│   │   ├── blog-post.html
│   │   ├── blog-list.html
│   │   └── templates.yaml
│   └── assets/             # Static assets
│       ├── css/            # Stylesheets
│       ├── fonts/          # Font files
│       ├── images/         # Images
│       └── logos/          # Brand logos
├── agents/                 # Agent definitions
│   ├── edward.yaml
│   ├── content.yaml
│   └── admin.yaml
├── mcp/                    # MCP server configurations
│   ├── admin.yaml
│   ├── content-manager.yaml
│   └── infrastructure.yaml
├── skills/                 # Skill definitions
│   ├── config.yaml
│   └── {skill_name}/
│       └── config.yaml
├── ai/
│   └── config.yaml         # AI provider settings
└── scheduler/
    └── config.yaml         # Job scheduling
```

---

## Key Services Configuration Files

### Master Config (`services/config/config.yaml`)

Aggregates all service configurations via includes:

```yaml
includes:
  - ../agents/edward.yaml
  - ../agents/content.yaml
  - ../mcp/admin.yaml
  - ../mcp/content-manager.yaml
  - ../skills/config.yaml
  - ../ai/config.yaml
  - ../web/config.yaml
  - ../scheduler/config.yaml

settings:
  agentPortRange: [9000, 9999]
  mcpPortRange: [5000, 5999]
  autoStartEnabled: true
  validationStrict: true
```

### Content Config (`services/content/config.yaml`)

Defines content sources and URL routing:

```yaml
content_sources:
  blog:
    path: "services/content/blog"     # Markdown source directory
    source_id: "blog"                 # Unique identifier
    category_id: "blog"               # Category mapping
    enabled: true
    branding:
      name: "Blog"
      description: "Technical articles..."
      image: "/files/images/blog/og-blog-default.png"
    sitemap:
      enabled: true
      url_pattern: "/blog/{slug}"     # URL routing pattern
      priority: 0.8
      changefreq: "weekly"
      parent_route:
        enabled: true
        url: "/blog"                  # Blog list page URL

  legal:
    path: "services/content/legal"
    source_id: "legal"
    sitemap:
      url_pattern: "/legal/{slug}"
```

**URL Pattern Resolution**: The `url_pattern` field defines how URLs map to content. `{slug}` is extracted from the URL path and used to lookup content in the database.

### Web Config (`services/web/config.yaml`)

Defines theming, branding, and navigation:

```yaml
paths:
  templates: "services/web/templates"  # HTML templates
  assets: "services/web/assets"        # Static assets

content:
  config_file: "services/content/config.yaml"
  sources:
    - blog
    - legal

branding:
  name: "tyingshoelaces"
  logo:
    primary:
      svg: "/assets/logos/logo.svg"
      webp: "/assets/logos/logo.webp"
  favicon: "/vite.svg"
  twitter_handle: "@tyingshoelaces_"

navigation:
  footer:
    resources:
      - path: "/blog"
        label: "Blog"
```

---

## How Profiles and Services Connect

### Path Resolution Flow

```
┌─────────────────────────────────────────────────────────────┐
│ Profile (e.g., groot/profile.yaml)                          │
│                                                             │
│ paths:                                                      │
│   system: /app                                              │
│   services: /app/services  ────────────────────────────┐    │
│   web_path: /app/web                                   │    │
│   storage: /app/storage                                │    │
└─────────────────────────────────────────────────────────│────┘
                                                         │
                                                         ▼
┌─────────────────────────────────────────────────────────────┐
│ Services Directory (/app/services)                          │
│                                                             │
│ ├── config/config.yaml  ← Master config                     │
│ ├── content/config.yaml ← Content routing                   │
│ ├── web/config.yaml     ← Theming                          │
│ └── ...                                                     │
└─────────────────────────────────────────────────────────────┘
```

### Runtime Configuration Loading

```rust
// 1. Profile loaded via SYSTEMPROMPT_PROFILE env var
ProfileBootstrap::init()?;

// 2. Services path derived from profile
let services_path = profile.paths.services;

// 3. Content config loaded from services
let content_config = ContentConfig::load(&services_path.join("content/config.yaml"))?;

// 4. Web config loaded for theming
let web_config = WebConfig::load(&services_path.join("web/config.yaml"))?;
```

---

## Build Process

The build process combines core web assets with service-specific assets.

### Environment Variables

| Variable | Purpose | Set By |
|----------|---------|--------|
| `SYSTEMPROMPT_WEB_CONFIG_PATH` | Path to `services/web/config.yaml` | `just web-build` |
| `SYSTEMPROMPT_WEB_METADATA_PATH` | Path to `services/web/metadata.yaml` | `just web-build` |
| `SYSTEMPROMPT_EXTENSIONS_PATH` | Path to `extensions/` | `just web-build` |
| `SYSTEMPROMPT_PROFILE` | Path to profile YAML | Manual or deployment |

### Build Flow

```
just web-build
    │
    ├─► Set environment variables from services/
    │
    ├─► npm run build (in core/web/)
    │   ├─► Run theme:generate (reads services/web/config.yaml)
    │   │   └─► Generates: core/web/src/styles/theme.generated.css
    │   │   └─► Generates: core/web/src/theme.config.ts
    │   │
    │   └─► vite build
    │       └─► Output: core/web/dist/
    │
    └─► Copy CSS from services/web/assets/css/ to core/web/dist/css/
```

### Deploy Flow

```
just deploy
    │
    ├─► just build --release        # Compile Rust binary
    │
    ├─► just web-build              # Build web assets
    │
    └─► systemprompt cloud deploy   # Deploy to cloud
        ├─► Docker build
        │   └─► COPY core/web/dist /app/web/dist
        │   └─► COPY services/ /app/services/
        │
        └─► Push to Fly.io
```

---

## Static Asset Serving

### Asset Path Mapping

| URL Path | Source | Description |
|----------|--------|-------------|
| `/css/*.css` | `core/web/dist/css/` | Stylesheets (copied from services/web/assets/css/) |
| `/js/*.js` | `core/web/dist/assets/` | Vite-bundled JavaScript |
| `/assets/logos/*` | `core/web/dist/assets/logos/` | Brand logos |
| `/images/*` | `core/web/dist/images/` | Content images |
| `/fonts/*` | `core/web/dist/fonts/` | Font files |
| `/files/*` | `{storage}/files/` | User-uploaded files |

### Route Classification

The `RouteClassifier` determines how requests are handled:

```rust
RouteType::StaticAsset   // CSS, JS, images, fonts (by extension)
RouteType::HtmlContent   // Blog posts, legal pages (by URL pattern)
RouteType::ApiEndpoint   // /api/v1/* routes
RouteType::NotFound      // Everything else
```

Static assets are identified by:1. Path prefix: `/assets/`, `/files/`, `/generated/`, `/.well-known/`
2. File extension: `.js`, `.css`, `.png`, `.jpg`, `.svg`, `.woff2`, etc.

---

## Common Issues

### CSS/Assets Return 404

**Cause**: `just web-build` not run, or CSS not copied to dist.

**Fix**:
```bash
just web-build
```

**Verification**:
```bash
ls core/web/dist/css/
# Should show: blog.css, syntax-highlight.css, etc.
```

### Content Not Found (But Exists in DB)

**Cause**: Static HTML not pre-rendered.

**Fix**:
```bash
systemprompt services generator build --full
```

### Profile Not Found

**Cause**: `SYSTEMPROMPT_PROFILE` not set.

**Fix**:
```bash
export SYSTEMPROMPT_PROFILE=.systemprompt/profiles/local/profile.yaml
```

---

## Best Practices

### 1. Never Hardcode Paths

Use config values, not hardcoded paths:

```yaml
# GOOD: In services/web/config.yaml
branding:
  logo:
    primary:
      svg: "/assets/logos/logo.svg"

# BAD: Hardcoded in templates
<img src="/some/hardcoded/path/logo.svg">
```

### 2. Use Template Variables

Templates should use variables injected from config:

```html
<!-- GOOD -->
<link rel="stylesheet" href="{{CSS_PATH}}/blog.css" />
<img src="{{LOGO_PATH}}" alt="{{ORG_NAME}}" />

<!-- BAD -->
<link rel="stylesheet" href="/css/blog.css" />
```

### 3. Separate Concerns

- **Profile**: Infrastructure (paths, URLs, credentials)
- **Services/web**: Presentation (theming, branding)
- **Services/content**: Data (content sources, routing)
- **Services/agents**: Behavior (agent definitions)

### 4. Run Full Build Before Deploy

```bash
just deploy  # Runs: build --release, web-build, cloud deploy
```

---

## Summary

| Component | Location | Purpose |
|-----------|----------|---------|
| Profile | `.systemprompt/profiles/{env}/profile.yaml` | Environment config (WHERE) |
| Services Master | `services/config/config.yaml` | Aggregates all services |
| Content Config | `services/content/config.yaml` | URL routing, content sources |
| Web Config | `services/web/config.yaml` | Theming, branding, navigation |
| Web Assets | `services/web/assets/` | CSS, fonts, images, logos |
| Web Templates | `services/web/templates/` | HTML templates |
| Build Output | `core/web/dist/` | Final bundled assets |
