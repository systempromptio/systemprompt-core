# agents status

## Status
**PASS**

## Command
```
systemprompt --non-interactive agents status
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

Agent Status
{
  "agents": [
    {
      "name": "admin",
      "enabled": true,
      "is_running": true,
      "pid": 20753,
      "port": 9002
    },
    {
      "name": "content",
      "enabled": true,
      "is_running": true,
      "pid": 20752,
      "port": 9001
    },
    {
      "name": "infrastructure",
      "enabled": true,
      "is_running": true,
      "pid": 20754,
      "port": 9003
    }
  ]
}
```
