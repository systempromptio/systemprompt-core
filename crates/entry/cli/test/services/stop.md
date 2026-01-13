# services stop

## Status
**PASS**

## Command
```
systemprompt --non-interactive services stop
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

⚠ GeoIP database not configured - geographic data will not be available
ℹ   To enable geographic data:
ℹ   1. Download MaxMind GeoLite2-City database from: https://dev.maxmind.com/geoip/geolite2-free-geolocation-data
ℹ   2. Add paths.geoip_database to your profile pointing to the .mmdb file

Stopping MCP Servers
ℹ Stopping content-manager...
ℹ Stopping systemprompt-admin...
ℹ Stopping systemprompt-infrastructure...
✓ Stopped 3 MCP servers

Stopping Agents
ℹ Stopping admin...
ℹ Stopping content...
ℹ Stopping infrastructure...
✓ Stopped 3 agents

Stopping API Server
ℹ Stopping API server (PID: 19959)...
✓ API server stopped
✓ All requested services stopped
```
