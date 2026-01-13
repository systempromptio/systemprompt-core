# agents show

## Status
**PASS**

## Command
```
systemprompt --non-interactive agents show admin
```

## Output
```

Validating configuration
  ✓ Services config (includes merged)
  ✓ Content config
  ✓ Web config
  ✓ Web metadata

▸ Validating domains
  ✓ [files] (valid)
  ✓ [web] (valid)
  ✓ [content] (valid)
  ✓ [agents] (valid)
  ✓ [mcp] (valid)
  ✓ [ai] (valid)


Agent: admin
{
  "name": "admin",
  "display_name": "Admin Agent - System Analytics & Management Expert",
  "description": "Specialized agent for system administration, analytics, and monitoring. Expert in using admin MCP tools to analyze traffic, users, conversations, and system health. Admin-only access.",
  "port": 9002,
  "endpoint": "/api/v1/agents/admin",
  "enabled": true,
  "provider": "-",
  "model": "-",
  "mcp_servers": [
    "systemprompt-admin"
  ],
  "skills_count": 1
}
```
