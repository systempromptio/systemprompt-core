# systemprompt-core-api Unit Tests

## Crate Overview
HTTP API server and gateway. Handles routing, middleware, health checks, and context extraction.

## Source Files
- `src/routes/` - API endpoint handlers
- `src/services/health/` - HealthChecker
- `src/services/middleware/` - Context, analytics, JWT middleware
- `src/services/server/` - ApiServer, lifecycle
- `src/models/` - API models

## Test Plan

### Health Check Tests
- `test_health_checker_all_healthy` - All healthy
- `test_health_checker_degraded` - Degraded state
- `test_health_checker_unhealthy` - Unhealthy state
- `test_module_health` - Module health

### Middleware Tests
- `test_context_middleware_extraction` - Extract context
- `test_analytics_middleware_tracking` - Track analytics
- `test_jwt_middleware_validation` - Validate JWT
- `test_header_context_extraction` - Header extraction

### Server Tests
- `test_server_startup` - Server startup
- `test_server_shutdown` - Graceful shutdown
- `test_server_config` - Configuration

### Route Tests
- `test_engagement_routes` - Engagement endpoints
- `test_proxy_routes` - Proxy endpoints
- `test_stream_routes` - Streaming endpoints
- `test_sync_routes` - Sync endpoints

## Mocking Requirements
- Mock HTTP requests
- Mock middleware dependencies

## Test Fixtures Needed
- Sample HTTP requests
- Sample headers
