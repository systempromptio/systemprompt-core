# services status

## Status
**PASS**

## Command
```
systemprompt --non-interactive services status
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

Service Status
content: agent | stopped | PID: - | start
infrastructure: agent | stopped | PID: - | start
admin: agent | stopped | PID: - | start
systemprompt-admin: mcp | stopped | PID: - | start
systemprompt-infrastructure: mcp | stopped | PID: - | start
content-manager: mcp | stopped | PID: - | start
ℹ 0/6 services running
```
