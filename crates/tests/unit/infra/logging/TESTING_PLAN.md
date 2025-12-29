# systemprompt-core-logging Unit Tests

## Crate Overview
Centralized logging with database, console, and tracing support. Includes log filtering, request tracing, and AI execution tracking.

## Source Files
- `src/layer/` - Tracing subscriber layers
- `src/models/` - Log entries, filters, levels
- `src/repository/` - Log storage and retrieval
- `src/services/` - Logging services, CLI output modes
- `src/trace/` - Request tracing, AI execution tracking

## Test Plan

### Log Entry Tests
- `test_log_entry_creation` - Create log entry
- `test_log_entry_serialization` - JSON serialization
- `test_log_level_ordering` - Level comparison

### Log Filter Tests
- `test_log_filter_by_level` - Filter by log level
- `test_log_filter_by_module` - Filter by module
- `test_log_filter_by_time_range` - Time-based filtering
- `test_log_filter_combined` - Multiple filters

### Repository Tests
- `test_logging_repository_insert` - Insert log
- `test_logging_repository_query` - Query logs
- `test_analytics_repository_insert` - Insert analytics

### Tracing Tests
- `test_request_span_creation` - Create request span
- `test_system_span_creation` - Create system span
- `test_ai_trace_service_tracking` - AI execution tracking

### Output Mode Tests
- `test_cli_output_mode_json` - JSON output
- `test_cli_output_mode_text` - Text output
- `test_cli_output_mode_quiet` - Quiet mode

## Mocking Requirements
- Mock database for repository tests
- Mock tracing subscriber

## Test Fixtures Needed
- Sample log entries
- Sample span data
