# services restart

## Status
**PASS**

## Command
```
systemprompt --non-interactive services restart api
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

Restarting API Server
⚠ API server restart via CLI is not currently supported
ℹ To restart the API server:
ℹ   1. Stop the current server (Ctrl+C if running in foreground)
ℹ   2. Run: just api
```
