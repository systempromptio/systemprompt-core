# logs stream cleanup

## Status
**PASS**

## Command
```
systemprompt --non-interactive logs stream cleanup --older-than 30d --yes
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

Logs Cleaned Up
{
  "deleted_count": 0,
  "dry_run": false,
  "cutoff_date": "2025-12-14 16:13:35 UTC",
  "vacuum_performed": false
}
```
