# mcp list

## Status
**PASS**

## Command
```
systemprompt --non-interactive mcp list
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


MCP Servers
{
  "servers": [
    {
      "name": "content-manager",
      "port": 5003,
      "enabled": true,
      "status": "configured"
    },
    {
      "name": "systemprompt-admin",
      "port": 5002,
      "enabled": true,
      "status": "configured"
    },
    {
      "name": "systemprompt-infrastructure",
      "port": 5004,
      "enabled": true,
      "status": "configured"
    }
  ]
}
```
