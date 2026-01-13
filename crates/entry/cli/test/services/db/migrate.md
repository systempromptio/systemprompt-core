# services db migrate

## Status
**PASS**

## Command
```
systemprompt --non-interactive services db migrate
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

ℹ System path: /var/www/html/tyingshoelaces
ℹ Database type: postgres
ℹ Database URL: postgres://systemprompt:123@localhost:5432/systemprompt
ℹ Installing 12 modules
ℹ   database
ℹ   users
ℹ   mcp
ℹ   ai
ℹ   analytics
ℹ   log
ℹ   oauth
ℹ   api
ℹ   agent
ℹ   content
ℹ   scheduler
ℹ   files
✓ Database migration completed
```
