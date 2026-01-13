# services start

## Status
**PASS**

## Command
```
systemprompt --non-interactive services start --all
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


Building Web Assets
ℹ Running npm run build...

</SYSTEMPROMPT.io>
Starting services...

✓ Web assets built successfully
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
  ⚠ Port 8080 in use by PID 19959
ℹ 
ℹ ✗ Startup failed after 13.3s
ℹ   Port 8080 is occupied by PID 19959. Use --kill-port-process to terminate.
ℹ 
Error: Port 8080 is occupied by PID 19959. Use --kill-port-process to terminate.
```
