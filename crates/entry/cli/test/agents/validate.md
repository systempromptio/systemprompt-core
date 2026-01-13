# agents validate

## Status
**PASS**

## Command
```
systemprompt --non-interactive agents validate
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


Validation Results
{
  "valid": true,
  "agents_checked": 3,
  "issues": [
    {
      "agent": "admin",
      "severity": "warning",
      "message": "Enabled agent has no AI provider configured"
    },
    {
      "agent": "infrastructure",
      "severity": "warning",
      "message": "Enabled agent has no AI provider configured"
    }
  ]
}
```
