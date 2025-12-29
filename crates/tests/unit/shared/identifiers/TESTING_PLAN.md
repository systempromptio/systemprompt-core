# systemprompt-identifiers Unit Tests

## Crate Overview
Type-safe identifier wrappers for all domain entities. Provides UUID-based IDs with database conversion traits (ToDbValue, FromDatabaseRow) and serialization support.

## Source Files
- `src/lib.rs` - Public exports
- `src/agent.rs` - AgentId, AgentName
- `src/ai.rs` - AiRequestId, ConfigId, MessageId
- `src/auth.rs` - JwtToken
- `src/client.rs` - ClientId, ClientType
- `src/content.rs` - ContentId, FileId, SkillId, etc.
- `src/context.rs` - ContextId
- `src/execution.rs` - ArtifactId, ExecutionStepId, LogId, TokenId
- `src/jobs.rs` - JobName, ScheduledJobId
- `src/links.rs` - CampaignId, LinkClickId, LinkId
- `src/mcp.rs` - AiToolCallId, McpExecutionId, McpServerId
- `src/roles.rs` - RoleId
- `src/session.rs` - SessionId
- `src/task.rs` - TaskId
- `src/trace.rs` - TraceId
- `src/user.rs` - UserId
- `src/macros.rs` - ID generation macros

## Test Plan

### ID Type Tests (per identifier type)

#### Happy Path Tests
- `test_<type>_new_creates_valid_id` - Create new ID successfully
- `test_<type>_from_uuid_converts_correctly` - Convert from UUID
- `test_<type>_to_string_formats_correctly` - String representation
- `test_<type>_serialize_json` - JSON serialization
- `test_<type>_deserialize_json` - JSON deserialization
- `test_<type>_to_db_value` - Database value conversion
- `test_<type>_clone_and_eq` - Clone and equality

#### Error Handling Tests
- `test_<type>_deserialize_invalid_uuid` - Invalid UUID string
- `test_<type>_deserialize_empty_string` - Empty string handling

#### Edge Cases
- `test_<type>_nil_uuid_handling` - Nil UUID behavior
- `test_<type>_max_uuid_handling` - Maximum UUID value

### Macro Tests

#### Happy Path Tests
- `test_define_id_macro_generates_struct` - Macro creates proper struct
- `test_define_id_macro_implements_traits` - Required traits implemented

## Mocking Requirements
- None (pure data types)

## Test Fixtures Needed
- Sample UUID values
- Sample invalid strings
