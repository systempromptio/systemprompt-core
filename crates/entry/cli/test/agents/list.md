# agents list

## Status
**PASS**

## Command
```
systemprompt --non-interactive agents list
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


Agents
{
  "agents": [
    {
      "name": "admin",
      "display_name": "Admin Agent - System Analytics & Management Expert",
      "port": 9002,
      "enabled": true,
      "is_primary": false,
      "is_default": false
    },
    {
      "name": "content",
      "display_name": "Content Marketing Strategist - World-Class Content Creation",
      "port": 9001,
      "enabled": true,
      "is_primary": false,
      "is_default": false
    },
    {
      "name": "infrastructure",
      "display_name": "Infrastructure Agent - Cloud Deployment & Sync Expert",
      "port": 9003,
      "enabled": true,
      "is_primary": false,
      "is_default": false
    }
  ]
}
```
