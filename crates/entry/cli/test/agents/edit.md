# agents edit

## Status
**PASS**

## Command
```
systemprompt --non-interactive agents edit admin --display-name "Admin Agent Updated"
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

⚠ Agent editing modifies configuration files. Please update the agent configuration in your services.yaml manually for now.

Edit Agent: admin
{
  "name": "admin",
  "message": "Agent 'admin' edit prepared. Apply the following changes to services.yaml:\n\ncard.displayName: Admin Agent Updated",
  "changes": [
    "card.displayName: Admin Agent Updated"
  ]
}
```
