# cloud init

## Status
**PASS**

## Command
```
systemprompt --non-interactive cloud init
```

## Output
```

Initialize Project
Project: systemprompt-core
Root: /var/www/html/systemprompt-core
ℹ   Created .systemprompt/.gitignore
ℹ   Created .systemprompt/.dockerignore
ℹ   Created .systemprompt/Dockerfile
ℹ   Created .systemprompt/entrypoint.sh
✓ Created .systemprompt/

Creating Services Boilerplate
ℹ   Created /var/www/html/systemprompt-core/logs/.gitignore
ℹ   Created /var/www/html/systemprompt-core/services/config/config.yaml
ℹ   Created /var/www/html/systemprompt-core/services/agents/assistant.yaml
ℹ   Created /var/www/html/systemprompt-core/services/agents/admin.yaml
ℹ   Created /var/www/html/systemprompt-core/services/mcp/systemprompt-admin.yaml
ℹ   Created /var/www/html/systemprompt-core/services/ai/config.yaml
ℹ   Created /var/www/html/systemprompt-core/services/content/config.yaml
ℹ   Created /var/www/html/systemprompt-core/services/web/config.yaml
ℹ   Created /var/www/html/systemprompt-core/services/web/metadata.yaml
ℹ   Created /var/www/html/systemprompt-core/services/scheduler/config.yaml
ℹ   Created /var/www/html/systemprompt-core/services/web/templates/page.html
ℹ   Created /var/www/html/systemprompt-core/services/web/templates/blog-post.html
ℹ   Created /var/www/html/systemprompt-core/services/web/templates/blog-list.html
ℹ   Created /var/www/html/systemprompt-core/services/web/templates/page-list.html
ℹ   Created /var/www/html/systemprompt-core/services/content/blog/welcome/index.md
ℹ   Created /var/www/html/systemprompt-core/services/content/legal/privacy-policy.md
ℹ   Created /var/www/html/systemprompt-core/services/content/legal/cookie-policy.md
ℹ   Created /var/www/html/systemprompt-core/services/skills/.gitkeep
✓ Cloned systemprompt-admin MCP server
✓ Services boilerplate created

Next Steps
ℹ 1. systemprompt cloud auth login     # Authenticate
ℹ 2. systemprompt cloud tenant create  # Create a tenant
ℹ 3. systemprompt cloud profile create local  # Create a profile
```
