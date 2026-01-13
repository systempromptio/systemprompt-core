# agents create

## Status
**PASS**

## Command
```
systemprompt --non-interactive agents create --name test-agent --display-name "Test Agent" --description "Test agent"
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

ℹ Creating agent 'test-agent' on port 8001 (display: Test Agent)...
⚠ Agent creation modifies configuration files. Please add the agent configuration to your services.yaml manually for now.

Agent Created
{
  "name": "test-agent",
  "message": "Agent 'test-agent' configuration prepared. Add to services.yaml:\n\nagents:\n  test-agent:\n    name: test-agent\n    port: 8001\n    endpoint: /\n    enabled: false\n    card:\n      protocolVersion: \"1.0\"\n      displayName: \"Test Agent\"\n      description: \"Test agent\"\n      version: \"1.0.0\"\n    metadata:\n      provider: anthropic\n      model: claude-3-5-sonnet-20241022"
}
```
