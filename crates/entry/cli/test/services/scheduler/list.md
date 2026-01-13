# services scheduler list

## Status
**PASS**

## Command
```
systemprompt --non-interactive services scheduler list
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

Available Jobs
ℹ   file_ingestion - Scans storage directory for image files and creates database entries
ℹ   cleanup_anonymous_users - Cleans up old anonymous users (30d)
ℹ   cleanup_inactive_sessions - Cleans up inactive sessions (1 hour threshold)
ℹ   feature_extraction - Extracts ML behavioral features from completed sessions
ℹ   database_cleanup - Cleans up orphaned logs, MCP executions, and expired OAuth tokens
ℹ   cleanup_empty_contexts - Deletes empty conversation contexts older than 1 hour
ℹ   behavioral_analysis - Analyzes fingerprint behavior patterns and flags suspicious activity
ℹ   content_ingestion - Ingests markdown content from configured directories into the database
ℹ   image_optimization - Converts images to WebP format for optimization
ℹ   publish_content - Publishes content through the full pipeline: images, ingestion, prerender, sitemap, CSS
```
