# systemprompt-models Unit Tests

## Crate Overview
Shared data models used across all layers including A2A protocol, AI services, API request/response, artifacts, authentication, configuration, content, events, and more.

## Source Files (key modules)
- `src/a2a/` - Agent-to-Agent protocol models
- `src/ai/` - AI service models and tool definitions
- `src/api/` - API request/response models
- `src/artifacts/` - Artifact types (card, chart, dashboard, list, table, text)
- `src/auth/` - Authentication models
- `src/config/` - Configuration models
- `src/content/` - Content models
- `src/events/` - Event system models and payloads
- `src/execution/` - Execution context models
- `src/mcp/` - Model Context Protocol models
- `src/oauth/` - OAuth models
- `src/validators/` - Validation models

## Test Plan

### A2A Models
- `test_agent_card_serialize_deserialize` - AgentCard round-trip
- `test_task_status_transitions` - TaskStatus enum values
- `test_message_part_variants` - Message part type handling

### AI Models
- `test_ai_message_role_variants` - Role enum coverage
- `test_tool_definition_schema` - Tool schema validation
- `test_model_config_defaults` - Default configuration values

### Artifact Models
- `test_artifact_type_variants` - All artifact type variants
- `test_card_artifact_validation` - Card artifact structure
- `test_chart_data_serialization` - Chart data handling

### Config Models
- `test_config_merge_behavior` - Configuration merging
- `test_config_validation_rules` - Validation constraints
- `test_environment_config_parsing` - Environment parsing

### Event Models
- `test_event_payload_variants` - All payload types
- `test_event_serialization` - Event JSON format

## Mocking Requirements
- None (pure data models)

## Test Fixtures Needed
- Sample YAML/JSON configs
- Sample event payloads
- Sample artifact data
