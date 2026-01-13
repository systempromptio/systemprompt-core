# mcp validate

## Status
**FAIL** (exit code: 1)

## Command
```
systemprompt --non-interactive mcp validate content
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

Error: Failed to validate MCP server

Caused by:
    MCP server 'content' not found
```
