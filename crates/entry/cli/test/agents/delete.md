# agents delete

## Status
**PASS**

## Command
```
systemprompt --non-interactive agents delete nonexistent-agent --yes
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

⚠ Agent 'nonexistent-agent' not found, nothing to delete

Delete Agent
{
  "deleted": [],
  "message": "Agent 'nonexistent-agent' not found"
}
```
