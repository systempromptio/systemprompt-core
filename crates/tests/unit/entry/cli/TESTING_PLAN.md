# systemprompt-cli Unit Tests

## Crate Overview
Command-line interface for agent orchestration, AI operations, and system management.

## Source Files
- `src/agents/` - Agent management commands
- `src/cli_settings/` - CLI configuration
- `src/cloud/` - Cloud integration commands
- `src/common/` - Common utilities
- `src/logs/` - Log viewing commands
- `src/presentation/` - Output formatting
- `src/services/` - CLI services
- `src/setup/` - Setup/initialization

## Test Plan

### Command Parsing Tests
- `test_parse_agent_commands` - Agent commands
- `test_parse_cloud_commands` - Cloud commands
- `test_parse_log_commands` - Log commands
- `test_parse_setup_commands` - Setup commands

### Agent Command Tests
- `test_agent_list` - List agents
- `test_agent_start` - Start agent
- `test_agent_stop` - Stop agent
- `test_agent_config` - Configure agent

### Cloud Command Tests
- `test_cloud_auth` - Authentication
- `test_cloud_sync` - Sync operations
- `test_cloud_checkout` - Checkout operations

### Log Command Tests
- `test_log_query` - Query logs
- `test_log_filter` - Filter logs
- `test_log_display` - Display formatting

### Output Formatting Tests
- `test_presentation_json` - JSON output
- `test_presentation_table` - Table output
- `test_presentation_text` - Text output

### Settings Tests
- `test_cli_settings_load` - Load settings
- `test_cli_settings_save` - Save settings

## Mocking Requirements
- Mock stdin/stdout
- Mock filesystem
- Mock cloud API

## Test Fixtures Needed
- Sample command inputs
- Sample outputs
