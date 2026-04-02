# 22 -- Integration & DB-Dependent Test Infrastructure Roadmap

This document plans a wave-by-wave campaign to add repository-level integration tests for every untested database repository in the codebase. Each wave is designed to be executable by 3 parallel agents with clear file targets, method lists, and verification steps.

---

## Context

### Why This Matters

The codebase has 453 `sqlx::query` call sites across 7 domain crates and the scheduler, served by 55+ repository structs. Only 3 repositories have integration tests today: `UserRepository` (21 tests), `BannedIpRepository` (17 tests), and `FileRepository` (19 tests). That is 57 integration tests covering 3 out of 55+ repositories -- roughly 5% coverage of the data layer.

Every SQL query is compile-time verified by `sqlx::query!()`, so syntax errors are caught at build time. What is NOT caught: correctness of multi-table joins, transaction isolation, cascade deletes, upsert conflict resolution, pagination boundary conditions, and business-rule enforcement encoded in SQL (e.g., `WHERE is_active = true`). These require live-database integration tests.

### Current State

- **270 integration test functions** exist in `crates/tests/integration/`, but most are E2E HTTP flows (session creation, analytics events, A2A protocol), not direct repository tests
- **57 repository-level tests** across `UserRepository`, `BannedIpRepository`, `FileRepository`
- **443 `sqlx::query` calls** in domain crates + **10** in scheduler = **453 total SQL call sites**
- **8,535 unit tests** all passing (Phases 1-5 complete)

### Goal

Add ~600 new repository-level integration tests organized into 7 phases (10 waves). Each test instantiates a repository struct, calls a public async method against a live Postgres database, asserts the result, and cleans up.

---

## Current Integration Test Architecture

### TestContext (`crates/tests/integration/common/context.rs`)

Central test environment struct providing:
- `db: Arc<Database>` -- connection to Postgres via `DATABASE_URL` env var
- `http: Client` -- reqwest client with cookie store for E2E tests
- `base_url: String` -- from `API_EXTERNAL_URL`
- `fingerprint: String` -- UUID-based, used for session isolation
- Helper methods: `make_request()`, `get_anonymous_token()`, `create_context()`, `make_authenticated_request()`, `cleanup()`

### Direct Repository Pattern (preferred for new tests)

The existing `UserRepository` and `FileRepository` tests bypass `TestContext` entirely. They use a minimal `get_db()` helper:

```rust
async fn get_db() -> Option<Database> {
    let database_url = std::env::var("DATABASE_URL").ok()?;
    Database::new_postgres(&database_url).await.ok()
}
```

Then instantiate repositories directly via `Repository::new(db.as_pool()?)` or `Repository::new(db.pool())`. This is the pattern all new tests should follow.

### Factory Pattern (`crates/tests/integration/common/factories.rs`)

- `SessionFactory` -- builds HTTP headers (user-agent, IP, fingerprint, UTM)
- `ConversationFactory` -- builds message payloads
- `fingerprint()` -- generates `test-{uuid}` strings
- `user_agent()` -- random browser UA string

### Cleanup Pattern (`crates/tests/integration/common/cleanup.rs`)

`TestCleanup` tracks IDs for deferred deletion:
- `track_fingerprint()`, `track_task()`, `track_session()`, `track_content()`, `track_source()`
- `cleanup_all()` deletes in order: fingerprints -> tasks -> sessions -> content -> sources -> test_sources

### Extension Migration Weight Ordering

Schemas are applied in weight order. Tests must respect FK dependencies:

| Weight | Domain | Tables |
|--------|--------|--------|
| 10 | users | `users`, `banned_ips`, `user_sessions` |
| 20 | analytics | `fingerprint_reputation`, `engagement_events`, `funnels`, `funnel_progress` |
| 25 | mcp | `mcp_sessions`, `mcp_tool_executions`, `mcp_artifacts` |
| 30 | oauth | `oauth_clients`, `oauth_auth_codes`, `oauth_refresh_tokens`, `webauthn_*` |
| 35 | ai | `ai_requests`, `ai_request_messages`, `ai_request_tool_calls` |
| 40 | agent | `user_contexts`, `agent_tasks`, `task_messages`, `message_parts`, `task_artifacts`, `artifact_parts`, `task_execution_steps`, `services`, `agents`, `agent_skills`, `task_push_notification_configs` |
| 45 | content | `markdown_content`, `markdown_categories`, `campaign_links`, `link_clicks` |
| 50 | files | `files`, `content_files`, `ai_image_analytics` |

---

## Prerequisites (Phase 0) -- Enhanced Test Infrastructure

### 0.1 Domain-Specific Test Data Factories

Create `crates/tests/integration/common/domain_factories.rs` with builder structs:

**OAuthFactories:**
- `OAuthClientFactory` -- builds `CreateClientParams` with unique `client_{uuid}` IDs, test redirect URIs, default scopes
- `AuthCodeFactory` -- builds `AuthCodeParams` with PKCE challenge generation
- `RefreshTokenFactory` -- builds `RefreshTokenParams` with future expiry
- `WebAuthnCredentialFactory` -- builds `WebAuthnCredentialParams` with random credential_id/public_key bytes
- `SetupTokenFactory` -- builds `CreateSetupTokenParams` with SHA-256 hashed tokens

**AgentFactories:**
- `ContextFactory` -- builds context creation params with test user_id + session_id
- `TaskFactory` -- builds task params with context_id, agent_name, initial state
- `MessageFactory` -- builds message params with text parts
- `AgentFactory` -- builds `Agent` struct with unique agent_id, name, description
- `SkillFactory` -- builds `Skill` struct with unique skill_id, file_path
- `ExecutionStepFactory` -- builds `ExecutionStep` with task_id, tool name, status

**AnalyticsFactories:**
- `SessionFactory` (extend existing) -- add `CreateSessionParams` builder for `SessionRepository`
- `FingerprintFactory` -- builds fingerprint hash strings for `FingerprintRepository`
- `FunnelFactory` -- builds `CreateFunnelInput` with steps
- `EngagementFactory` -- builds engagement event params
- `AnalyticsEventFactory` -- builds event params with type, category, session_id

**ContentFactories:**
- `ContentFactory` -- builds `CreateContentParams` with source_id, slug, markdown body
- `LinkFactory` -- builds campaign link params with short_code, target_url

**McpFactories:**
- `McpSessionFactory` -- builds session creation params with server_name, transport_type
- `ToolExecutionFactory` -- builds `ToolExecutionRequest` with tool_name, server_name
- `McpArtifactFactory` -- builds `CreateMcpArtifact` with server_name, content

**AiFactories:**
- `AiRequestFactory` -- builds `CreateAiRequest` with model, provider, token counts

### 0.2 Enhanced Cleanup Utilities

Extend `TestCleanup` in `crates/tests/integration/common/cleanup.rs`:

```
track_oauth_client(client_id: String)
track_auth_code(code: String)
track_refresh_token(token_id: String)
track_context(context_id: String)
track_agent(agent_id: String)
track_mcp_session(session_id: String)
track_ai_request(request_id: String)
track_funnel(funnel_id: String)
```

Add cascade-aware deletion order in `cleanup_all()`:
1. `task_push_notification_configs` -> `task_execution_steps` -> `message_parts` -> `task_messages` -> `artifact_parts` -> `task_artifacts` -> `agent_tasks`
2. `context_agents` -> `user_contexts`
3. `ai_request_tool_calls` -> `ai_request_messages` -> `ai_requests`
4. `mcp_tool_executions` -> `mcp_artifacts` -> `mcp_sessions`
5. `oauth_auth_codes` -> `oauth_refresh_tokens` -> `webauthn_*` -> `oauth_client_*` -> `oauth_clients`
6. `funnel_progress` -> `funnels`
7. `engagement_events` -> `fingerprint_reputation`
8. `link_clicks` -> `campaign_links` -> `markdown_content`

### 0.3 Test Database Helper

Create `crates/tests/integration/common/db_test_helpers.rs`:

```rust
pub async fn get_db() -> Option<Database> { ... }
pub async fn get_db_pool() -> Option<DbPool> { ... }
pub fn unique_id(prefix: &str) -> String { format!("{prefix}_{}", Uuid::new_v4()) }
pub fn test_user_id() -> UserId { UserId::new(unique_id("test_user")) }
pub fn test_session_id() -> SessionId { SessionId::new(unique_id("sess")) }
pub fn test_client_id() -> ClientId { ClientId::new(unique_id("client")) }
```

### Files to Create/Modify

| File | Action |
|------|--------|
| `crates/tests/integration/common/domain_factories.rs` | CREATE |
| `crates/tests/integration/common/db_test_helpers.rs` | CREATE |
| `crates/tests/integration/common/cleanup.rs` | MODIFY -- add domain-specific tracking |
| `crates/tests/integration/common/mod.rs` | MODIFY -- export new modules |

---

## Phase 1: OAuth Domain (~89 tests)

**Priority: SECURITY-CRITICAL.** OAuth repository correctness determines authentication integrity.

### 1.1 OAuthRepository -- Auth Codes (~12 tests)

**File:** `crates/tests/integration/oauth/src/repository/auth_code.rs`

**Source:** `crates/domain/oauth/src/repository/oauth/auth_code.rs`
**Tables:** `oauth_auth_codes`

Methods to test:
- `store_authorization_code(params)` -- happy path, verify all fields stored
- `store_authorization_code(params)` -- with PKCE challenge + method
- `store_authorization_code(params)` -- with resource parameter
- `get_client_id_from_auth_code(code)` -- returns correct ClientId
- `get_client_id_from_auth_code(code)` -- returns None for nonexistent code
- `validate_authorization_code(code, client_id, redirect_uri, verifier)` -- happy path, marks used
- `validate_authorization_code()` -- rejects already-used code
- `validate_authorization_code()` -- rejects expired code (insert with past expiry)
- `validate_authorization_code()` -- rejects redirect_uri mismatch
- `validate_authorization_code()` -- PKCE S256 verification succeeds
- `validate_authorization_code()` -- PKCE S256 verification fails on wrong verifier
- `validate_authorization_code()` -- rejects plain PKCE method

### 1.2 OAuthRepository -- Refresh Tokens (~10 tests)

**File:** `crates/tests/integration/oauth/src/repository/refresh_token.rs`

**Source:** `crates/domain/oauth/src/repository/oauth/refresh_token.rs`
**Tables:** `oauth_refresh_tokens`

Methods to test:
- `store_refresh_token(params)` -- happy path
- `validate_refresh_token(token_id, client_id)` -- returns (UserId, scope)
- `validate_refresh_token()` -- returns error for nonexistent token
- `validate_refresh_token()` -- returns error for expired token
- `validate_refresh_token()` -- returns error for wrong client_id
- `consume_refresh_token(token_id, client_id)` -- deletes after returning
- `consume_refresh_token()` -- second consume fails
- `revoke_refresh_token(token_id)` -- returns true, removes token
- `revoke_refresh_token()` -- returns false for nonexistent
- `cleanup_expired_refresh_tokens()` -- removes expired, keeps valid
- `get_client_id_from_refresh_token(token_id)` -- returns Some/None

### 1.3 ClientRepository (~28 tests)

**File:** `crates/tests/integration/oauth/src/repository/client.rs`

**Source:** `crates/domain/oauth/src/repository/client/` (queries.rs, mutations.rs, cleanup.rs, relations.rs)
**Tables:** `oauth_clients`, `oauth_client_redirect_uris`, `oauth_client_grant_types`, `oauth_client_response_types`, `oauth_client_scopes`, `oauth_client_contacts`

Methods to test:

*Queries:*
- `get_by_client_id(client_id)` -- returns client with all relations loaded
- `get_by_client_id()` -- returns None for nonexistent
- `get_by_client_id()` -- returns None for inactive client
- `get_by_client_id_any(client_id)` -- returns inactive client too
- `list()` -- returns only active clients
- `list_paginated(limit, offset)` -- respects pagination
- `count()` -- returns active client count
- `find_by_redirect_uri(uri)` -- finds client by redirect URI
- `find_by_redirect_uri()` -- returns None for unknown URI
- `find_by_redirect_uri_with_scope(uri, scopes)` -- filters by scope

*Mutations:*
- `create(params)` -- creates client with all relations (redirect_uris, grant_types, response_types, scopes, contacts)
- `create(params)` -- default grant_types/response_types when None
- `update(params)` -- updates name, URIs, scopes in transaction
- `update(params)` -- returns None for nonexistent client
- `update_secret(client_id, hash)` -- updates secret hash
- `delete(client_id)` -- cascade deletes all relations
- `deactivate(client_id)` -- sets is_active = false
- `activate(client_id)` -- sets is_active = true

*Cleanup:*
- `cleanup_inactive()` -- deletes inactive clients
- `cleanup_old_test(days)` -- deletes old test_* clients
- `deactivate_old_test(days)` -- deactivates old test_* clients
- `delete_unused(cutoff)` -- deletes never-used clients older than cutoff
- `delete_stale(cutoff)` -- deletes clients not used since cutoff
- `list_inactive()` -- lists deactivated clients
- `list_old(timestamp)` -- lists clients older than timestamp
- `list_unused(cutoff)` -- lists never-used clients
- `list_stale(cutoff)` -- lists stale clients
- `update_last_used(client_id, timestamp)` -- updates last_used_at

### 1.4 OAuthRepository -- WebAuthn (~8 tests)

**File:** `crates/tests/integration/oauth/src/repository/webauthn.rs`

**Source:** `crates/domain/oauth/src/repository/webauthn.rs`
**Tables:** `webauthn_credentials`

Methods to test:
- `store_webauthn_credential(params)` -- happy path with all fields
- `store_webauthn_credential(params)` -- with transports array
- `get_webauthn_credentials(user_id)` -- returns credentials ordered by created_at DESC
- `get_webauthn_credentials(user_id)` -- returns empty vec for unknown user
- `get_webauthn_credentials(user_id)` -- returns multiple credentials
- `update_webauthn_credential_counter(credential_id, counter)` -- increments counter, sets last_used_at
- `update_webauthn_credential_counter()` -- counter overflow protection (i32::MAX)
- `store_webauthn_credential()` -- duplicate credential_id behavior

### 1.5 OAuthRepository -- Setup Tokens (~11 tests)

**File:** `crates/tests/integration/oauth/src/repository/setup_token.rs`

**Source:** `crates/domain/oauth/src/repository/setup_token.rs`
**Tables:** `webauthn_setup_tokens`

Methods to test:
- `store_setup_token(params)` -- returns token ID for credential_link purpose
- `store_setup_token(params)` -- returns token ID for recovery purpose
- `validate_setup_token(hash)` -- returns Valid with correct record
- `validate_setup_token(hash)` -- returns NotFound for unknown hash
- `validate_setup_token(hash)` -- returns Expired for past expiry
- `validate_setup_token(hash)` -- returns AlreadyUsed for consumed token
- `consume_setup_token(id)` -- returns true, sets used_at
- `consume_setup_token(id)` -- returns false for already-consumed
- `consume_setup_token(id)` -- returns false for nonexistent
- `cleanup_expired_setup_tokens()` -- deletes expired + old used tokens
- `revoke_user_setup_tokens(user_id)` -- marks all user tokens as used

### 1.6 OAuthRepository -- User Queries (~5 tests)

**File:** `crates/tests/integration/oauth/src/repository/user.rs`

**Source:** `crates/domain/oauth/src/repository/oauth/user.rs`
**Tables:** `users`

Methods to test:
- `find_user_by_email(email)` -- returns OAuthUser
- `find_user_by_email(email)` -- returns None for unknown
- `get_authenticated_user(user_id)` -- returns AuthenticatedUser with permissions
- `get_authenticated_user(user_id)` -- returns error for unknown user
- `get_authenticated_user(user_id)` -- handles multiple roles

### 1.7 OAuthRepository -- Facade Methods (~15 tests)

**File:** `crates/tests/integration/oauth/src/repository/oauth_facade.rs`

**Source:** `crates/domain/oauth/src/repository/oauth/mod.rs`
**Tables:** all oauth tables

Methods to test (these delegate to ClientRepository but add logging/validation):
- `create_client(params)` -- creates via facade with instrumentation
- `list_clients()` -- lists active clients
- `list_clients_paginated(limit, offset)` -- pagination
- `count_clients()` -- count
- `find_client_by_id(client_id)` -- lookup
- `find_client_by_redirect_uri(uri)` -- redirect URI lookup
- `find_client_by_redirect_uri_with_scope(uri, scopes)` -- scoped lookup
- `update_client(client_id, name, uris, scopes)` -- validates then updates
- `update_client()` -- rejects empty name
- `update_client()` -- rejects empty redirect_uris
- `update_client_full(client)` -- full object update
- `delete_client(client_id)` -- deletes, returns bool
- `cleanup_inactive_clients()` -- cleanup
- `cleanup_unused_clients(days)` -- cleanup by age
- `update_client_last_used(client_id)` -- touch timestamp

---

## Phase 2: Agent Domain (~110 tests)

### 2.1 ContextRepository (~14 tests)

**File:** `crates/tests/integration/agents/repository/context.rs`

**Source:** `crates/domain/agent/src/repository/context/` (queries.rs, mutations.rs)
**Tables:** `user_contexts`, `context_agents`

Methods to test:
- `create_context(name, user_id, session_id)` -- returns context_id
- `get_context(context_id)` -- returns context with fields
- `get_context()` -- returns None for nonexistent
- `list_contexts_basic(user_id)` -- returns user's contexts
- `list_contexts_with_stats(user_id)` -- returns contexts with task/message counts
- `find_by_session_id(session_id)` -- returns matching context
- `find_by_session_id()` -- returns None for unknown session
- `get_context_events_since(context_id, since)` -- returns events after timestamp
- `validate_context_ownership(context_id, user_id)` -- succeeds for owner
- `validate_context_ownership()` -- fails for non-owner
- `update_context_name(context_id, name)` -- updates name
- `delete_context(context_id)` -- deletes context
- `delete_context()` -- cascade behavior for related data
- `list_contexts_basic()` -- empty vec for user with no contexts

### 2.2 TaskRepository (~18 tests)

**File:** `crates/tests/integration/agents/repository/task.rs`

**Source:** `crates/domain/agent/src/repository/task/` (mod.rs, queries.rs, mutations.rs, task_updates.rs)
**Tables:** `agent_tasks`, `task_messages`, `message_parts`

Methods to test:
- `create_task(params)` -- returns task_id
- `get_task(task_id)` -- returns Task with all fields
- `get_task()` -- returns None for nonexistent
- `get_task_by_str(task_id_str)` -- string variant
- `list_tasks_by_context(context_id)` -- returns tasks for context
- `list_tasks_by_context_str(context_id_str)` -- string variant
- `get_tasks_by_user_id(user_id)` -- returns user's tasks
- `get_tasks_by_user_id_str(user_id_str)` -- string variant
- `track_agent_in_context(context_id, agent_name)` -- creates context_agents entry
- `track_agent_in_context_str()` -- string variant
- `update_task_state(task_id, new_state)` -- transitions state
- `update_task_state_str()` -- string variant
- `update_task_failed_with_error(task_id, error)` -- sets failed state + error
- `get_task_context_info(task_id)` -- returns context info for task
- `update_task_and_save_messages(task_id, ...)` -- transactional update with messages
- `delete_task(task_id)` -- cascade deletes messages, parts, steps
- `create_task()` -- multiple tasks in same context
- `list_tasks_by_context()` -- empty vec for context with no tasks

### 2.3 MessageRepository (~10 tests)

**File:** `crates/tests/integration/agents/repository/message.rs`

**Source:** `crates/domain/agent/src/repository/context/message/` (mod.rs, queries.rs, persistence.rs, parts.rs)
**Tables:** `task_messages`, `message_parts`

Methods to test:
- `get_messages_by_task(task_id)` -- returns messages with parts
- `get_messages_by_context(context_id)` -- returns all messages in context
- `get_next_sequence_number(task_id)` -- returns next seq number
- `persist_message_sqlx(pool, params)` -- persists message + parts
- `persist_message_with_tx(tx, params)` -- transactional variant
- `get_messages_by_task()` -- empty for task with no messages
- `get_messages_by_context()` -- ordered by sequence
- `get_next_sequence_number()` -- returns 1 for first message
- `get_next_sequence_number()` -- increments correctly after messages
- `persist_message_sqlx()` -- with multiple parts (text, data, file)

### 2.4 AgentServiceRepository (~12 tests)

**File:** `crates/tests/integration/agents/repository/agent_service.rs`

**Source:** `crates/domain/agent/src/repository/agent_service/mod.rs`
**Tables:** `services`

Methods to test:
- `register_agent(agent_name, pid, port)` -- creates service entry
- `register_agent_starting(agent_name)` -- creates with starting status
- `mark_running(agent_name)` -- transitions to running
- `get_agent_status(agent_name)` -- returns current status
- `get_agent_status()` -- returns None for unknown agent
- `mark_crashed(agent_name)` -- sets crashed status
- `mark_stopped(agent_name)` -- sets stopped status
- `mark_error(agent_name)` -- sets error status
- `list_running_agents()` -- returns only running agents
- `list_running_agent_pids()` -- returns PIDs of running agents
- `remove_agent_service(agent_name)` -- deletes entry
- `update_health_status(agent_name, ...)` -- updates health fields

### 2.5 ExecutionStepRepository (~10 tests)

**File:** `crates/tests/integration/agents/repository/execution.rs`

**Source:** `crates/domain/agent/src/repository/execution/mod.rs`
**Tables:** `task_execution_steps`

Methods to test:
- `create(step)` -- inserts execution step
- `get(step_id)` -- returns step
- `get()` -- returns None for nonexistent
- `list_by_task(task_id)` -- returns steps for task
- `complete_step(step_id, output, ...)` -- marks complete with output
- `fail_step(step_id, error)` -- marks failed with error
- `fail_in_progress_steps_for_task(task_id)` -- bulk fail
- `complete_planning_step(step_id, ...)` -- marks planning step complete
- `mcp_execution_id_exists(id)` -- checks existence
- `list_by_task()` -- empty for task with no steps

### 2.6 AgentRepository (~9 tests)

**File:** `crates/tests/integration/agents/repository/agent.rs`

**Source:** `crates/domain/agent/src/repository/content/agent.rs`
**Tables:** `agents`

Methods to test:
- `create(agent)` -- inserts agent
- `get_by_agent_id(agent_id)` -- returns agent
- `get_by_agent_id()` -- returns None for nonexistent
- `get_by_name(name)` -- returns agent by name
- `get_by_name()` -- returns None for unknown name
- `list_enabled()` -- returns only enabled agents
- `list_all()` -- returns all agents
- `update(agent_id, agent)` -- updates agent fields
- `delete(agent_id)` -- removes agent

### 2.7 SkillRepository (~8 tests)

**File:** `crates/tests/integration/agents/repository/skill.rs`

**Source:** `crates/domain/agent/src/repository/content/skill.rs`
**Tables:** `agent_skills`

Methods to test:
- `create(skill)` -- inserts skill
- `get_by_skill_id(skill_id)` -- returns skill
- `get_by_skill_id()` -- returns None for nonexistent
- `get_by_file_path(path)` -- finds by file path
- `get_by_file_path()` -- returns None for unknown path
- `list_enabled()` -- returns only enabled skills
- `list_all()` -- returns all skills
- `update(skill_id, skill)` -- updates skill fields

### 2.8 ArtifactRepository (~8 tests)

**File:** `crates/tests/integration/agents/repository/artifact.rs`

**Source:** `crates/domain/agent/src/repository/content/artifact/` (queries.rs, mutations.rs, parts.rs)
**Tables:** `task_artifacts`, `artifact_parts`

Methods to test:
- `create_artifact(params)` -- creates artifact with parts
- `get_artifact_by_id(artifact_id)` -- returns artifact
- `get_artifact_by_id()` -- returns None for nonexistent
- `get_artifacts_by_task(task_id)` -- returns task's artifacts
- `get_artifacts_by_context(context_id)` -- returns context's artifacts
- `get_artifacts_by_user_id(user_id)` -- returns user's artifacts
- `get_all_artifacts(limit, offset)` -- pagination
- `delete_artifact(artifact_id)` -- cascade deletes parts

### 2.9 PushNotificationConfigRepository (~7 tests)

**File:** `crates/tests/integration/agents/repository/push_notification.rs`

**Source:** `crates/domain/agent/src/repository/content/push_notification.rs`
**Tables:** `task_push_notification_configs`

Methods to test:
- `add_config(task_id, config)` -- creates config
- `get_config(task_id, config_id)` -- returns config
- `get_config()` -- returns None for nonexistent
- `list_configs(task_id)` -- returns all configs for task
- `delete_config(task_id, config_id)` -- deletes specific config
- `delete_all_for_task(task_id)` -- deletes all configs
- `list_configs()` -- empty vec for task with no configs

---

## Phase 3: MCP + AI (~50 tests)

### 3.1 McpSessionRepository (~10 tests)

**File:** `crates/tests/integration/mcp/repository/session.rs`

**Source:** `crates/domain/mcp/src/repository/session/mod.rs`
**Tables:** `mcp_sessions`

Methods to test:
- `create(session_id, server_name, transport_type)` -- creates session
- `exists(session_id)` -- returns true for existing
- `exists()` -- returns false for nonexistent
- `find_active(session_id)` -- returns active session record
- `find_active()` -- returns None for closed session
- `update_last_event_id(session_id, event_id)` -- updates event ID
- `update_activity(session_id)` -- touches last_activity_at
- `close(session_id)` -- marks session closed
- `delete_stale(retention_days)` -- deletes old sessions
- `cleanup_expired()` -- removes expired sessions

### 3.2 ToolUsageRepository (~10 tests)

**File:** `crates/tests/integration/mcp/repository/tool_usage.rs`

**Source:** `crates/domain/mcp/src/repository/tool_usage/mod.rs`
**Tables:** `mcp_tool_executions`

Methods to test:
- `start_execution(request)` -- creates execution, returns McpExecutionId
- `complete_execution(id, result, duration)` -- marks complete
- `log_execution_sync(request, result, duration)` -- synchronous insert
- `find_by_id(id)` -- returns execution record
- `find_by_id()` -- returns None for nonexistent
- `find_by_ai_call_id(ai_call_id)` -- returns execution by AI call
- `find_context_id(execution_id)` -- returns context_id
- `list_tool_stats(limit)` -- returns aggregated tool stats
- `update_context_timestamp(context_id)` -- touches context
- `start_execution()` then `complete_execution()` -- full lifecycle

### 3.3 McpArtifactRepository (~8 tests)

**File:** `crates/tests/integration/mcp/repository/artifact.rs`

**Source:** `crates/domain/mcp/src/repository/artifact/mod.rs`
**Tables:** `mcp_artifacts`

Methods to test:
- `save(artifact)` -- persists artifact
- `find_by_id(artifact_id)` -- returns record
- `find_by_id()` -- returns None for nonexistent
- `find_by_id_str(artifact_id)` -- string variant
- `list_by_server(server_name, limit, offset)` -- returns server's artifacts
- `delete(artifact_id)` -- removes artifact
- `cleanup_expired()` -- removes expired artifacts
- `list_by_server()` -- pagination works correctly

### 3.4 AiRequestRepository (~22 tests)

**File:** `crates/tests/integration/ai/repository/ai_requests.rs`

**Source:** `crates/domain/ai/src/repository/ai_requests/` (queries.rs, mutations.rs, message_operations.rs)
**Tables:** `ai_requests`, `ai_request_messages`, `ai_request_tool_calls`

*Queries:*
- `get_by_id(id)` -- returns request
- `get_by_id()` -- returns None for nonexistent
- `get_provider_usage(start, end)` -- returns aggregated provider stats
- `get_user_usage(user_id)` -- returns user's AI usage
- `get_session_usage(session_id)` -- returns session's AI usage

*Mutations:*
- `create(request)` -- creates AI request record
- `update_completion(id, completion)` -- updates with completion data
- `update_error(id, error)` -- updates with error data
- `insert(record)` -- low-level insert

*Message operations:*
- `insert_message(request_id, role, content, seq)` -- adds message
- `get_messages(request_id)` -- returns messages ordered by sequence
- `get_max_sequence(request_id)` -- returns highest sequence number
- `insert_tool_call(request_id, tool_name, args)` -- adds tool call
- `get_tool_calls(request_id)` -- returns tool calls
- `add_response_message(request_id, content)` -- adds assistant response
- `link_tool_calls_to_recent_executions(request_id)` -- links to MCP executions

*Edge cases:*
- `create()` then `update_completion()` -- full lifecycle
- `create()` then `update_error()` -- error lifecycle
- `get_messages()` -- empty for request with no messages
- `get_provider_usage()` -- empty range returns zeros
- `get_user_usage()` -- unknown user returns zeros
- `insert_message()` -- sequence ordering preserved

---

## Phase 4: Content + Links (~45 tests)

### 4.1 ContentRepository (~15 tests)

**File:** `crates/tests/integration/content/repository/content.rs`

**Source:** `crates/domain/content/src/repository/content/` (mod.rs, queries.rs, mutations.rs)
**Tables:** `markdown_content`, `markdown_categories`

Methods to test:
- `create(params)` -- creates content with all fields
- `get_by_id(id)` -- returns content
- `get_by_id()` -- returns None for nonexistent
- `get_by_slug(slug)` -- returns content by slug
- `get_by_slug()` -- returns None for unknown slug
- `get_by_source_and_slug(source_id, slug)` -- compound lookup
- `list(limit, offset)` -- paginated listing
- `list_by_source(source_id)` -- source-filtered listing
- `list_by_source_limited(source_id, limit, offset)` -- paginated source listing
- `list_all(limit, offset)` -- all content
- `update(params)` -- updates content fields
- `delete(id)` -- removes content
- `delete_by_source(source_id)` -- bulk delete by source
- `category_exists(category_id)` -- checks category existence
- `get_popular_content_ids(limit)` -- returns popular content

### 4.2 LinkRepository (~9 tests)

**File:** `crates/tests/integration/content/repository/link.rs`

**Source:** `crates/domain/content/src/repository/link/mod.rs`
**Tables:** `campaign_links`

Methods to test:
- `create_link(params)` -- creates campaign link
- `get_link_by_short_code(code)` -- finds by short code
- `get_link_by_short_code()` -- returns None for unknown code
- `list_links_by_campaign(campaign)` -- lists campaign links
- `list_links_by_source_content(source_id)` -- lists source links
- `get_link_by_id(id)` -- finds by ID
- `find_link_by_source_and_target(source, target)` -- compound lookup
- `delete_link(id)` -- removes link
- `create_link()` -- unique short_code enforcement

### 4.3 LinkAnalyticsRepository (~8 tests)

**File:** `crates/tests/integration/content/repository/link_analytics.rs`

**Source:** `crates/domain/content/src/repository/link/analytics.rs`
**Tables:** `link_clicks`, `campaign_links`

Methods to test:
- `record_click(params)` -- records a click event
- `increment_link_clicks(link_id)` -- increments click counter
- `get_clicks_by_link(link_id, start, end)` -- returns click data
- `get_link_performance(start, end)` -- returns performance metrics
- `check_session_clicked_link(session_id, link_id)` -- dedup check
- `get_content_journey_map(start, end)` -- returns journey data
- `get_campaign_performance(campaign, start, end)` -- campaign metrics
- `record_click()` then `get_clicks_by_link()` -- full flow

### 4.4 SearchRepository (~5 tests)

**File:** `crates/tests/integration/content/repository/search.rs`

**Source:** `crates/domain/content/src/repository/search/mod.rs`
**Tables:** `markdown_content` (full-text search)

Methods to test:
- `search_by_category(category, limit, offset)` -- category search
- `search_by_category()` -- empty results for unknown category
- `search_by_keyword(keyword, limit, offset)` -- keyword search
- `search_by_keyword()` -- empty results for no matches
- `search_by_keyword()` -- pagination

### 4.5 FileRepository Extensions (~8 tests)

Extend existing `crates/tests/integration/files/repository.rs` with additional edge case tests:
- `list_all()` -- pagination boundary (offset > total)
- `list_by_user()` -- empty user
- `find_by_path()` -- path with special characters
- `insert()` -- very long path strings
- `delete()` -- double delete is idempotent
- `update_metadata()` -- large JSON metadata
- `insert_file()` -- with all optional fields null
- `count_ai_images_by_user()` -- user with zero images returns 0

---

## Phase 5: Analytics (~130 tests)

The analytics domain is the largest, with 16 repository structs. Tests here are read-heavy (most methods query existing data with date ranges).

### 5.1 SessionRepository (~25 tests)

**File:** `crates/tests/integration/analytics/repository/session.rs`

**Source:** `crates/domain/analytics/src/repository/session/` (mod.rs, queries.rs, mutations.rs, behavioral.rs)
**Tables:** `user_sessions`

Methods to test:
- `create_session(params)` -- creates session
- `find_by_id(session_id)` -- returns session
- `find_by_id()` -- returns None for nonexistent
- `find_by_fingerprint(fingerprint, hours)` -- finds sessions by fingerprint
- `list_active_by_user(user_id)` -- lists active sessions
- `update_activity(session_id)` -- touches last_seen_at
- `increment_request_count(session_id)` -- increments counter
- `increment_task_count(session_id)` -- increments counter
- `increment_ai_request_count(session_id)` -- increments counter
- `increment_message_count(session_id)` -- increments counter
- `end_session(session_id)` -- marks session ended
- `mark_as_scanner(session_id)` -- flags scanner
- `mark_as_behavioral_bot(session_id, reason)` -- flags bot
- `check_and_mark_behavioral_bot(session_id)` -- auto-detection
- `cleanup_inactive(hours)` -- removes old sessions
- `migrate_user_sessions(old_user, new_user)` -- reassigns sessions
- `find_recent_by_fingerprint(fingerprint, hours)` -- recency check
- `exists(session_id)` -- existence check
- `increment_ai_usage(session_id, tokens, cost)` -- usage tracking
- `update_behavioral_detection(session_id, ...)` -- behavioral update
- `escalate_throttle(session_id, level)` -- throttle escalation
- `get_throttle_level(session_id)` -- returns current level
- `count_sessions_by_fingerprint(fingerprint, hours)` -- count
- `get_session_for_behavioral_analysis(session_id)` -- returns analysis data
- `has_analytics_events(session_id)` -- checks for events

### 5.2 FingerprintRepository (~12 tests)

**File:** `crates/tests/integration/analytics/repository/fingerprint.rs`

**Source:** `crates/domain/analytics/src/repository/fingerprint/` (queries.rs, mutations.rs)
**Tables:** `fingerprint_reputation`

Methods to test:
- `upsert_fingerprint(hash, session_id, user_agent, ...)` -- creates or updates
- `get_by_hash(hash)` -- returns fingerprint record
- `get_by_hash()` -- returns None for unknown
- `count_active_sessions(hash)` -- returns count
- `find_reusable_session(hash)` -- finds reusable session_id
- `get_fingerprints_for_analysis()` -- returns analysis candidates
- `get_high_risk_fingerprints(threshold)` -- returns high-risk entries
- `flag_fingerprint(hash, reason)` -- sets flag
- `update_velocity_metrics(hash, ...)` -- updates velocity
- `update_active_session_count(hash, count)` -- updates count
- `increment_request_count(hash)` -- increments
- `clear_flag(hash)` -- removes flag
- `adjust_reputation_score(hash, delta)` -- adjusts score, returns new

### 5.3 AnalyticsEventsRepository (~6 tests)

**File:** `crates/tests/integration/analytics/repository/events.rs`

**Source:** `crates/domain/analytics/src/repository/events.rs`
**Tables:** `analytics_events`

Methods to test:
- `create_event(params)` -- creates event
- `create_events_batch(events)` -- batch insert
- `count_events_by_type(event_type, start, end)` -- count by type
- `find_by_session(session_id)` -- returns session's events
- `find_by_content(content_id)` -- returns content's events
- `create_event()` -- with all optional fields

### 5.4 EngagementRepository (~6 tests)

**File:** `crates/tests/integration/analytics/repository/engagement.rs`

**Source:** `crates/domain/analytics/src/repository/engagement.rs`
**Tables:** `engagement_events`

Methods to test:
- `create_engagement(params)` -- creates engagement event
- `find_by_id(id)` -- returns event
- `find_by_id()` -- returns None
- `list_by_session(session_id)` -- session events
- `list_by_user(user_id, limit)` -- user events with limit
- `get_session_engagement_summary(session_id)` -- returns summary

### 5.5 FunnelRepository (~10 tests)

**File:** `crates/tests/integration/analytics/repository/funnel.rs`

**Source:** `crates/domain/analytics/src/repository/funnel/` (finders.rs, mutations.rs, stats.rs)
**Tables:** `funnels`, `funnel_progress`

Methods to test:
- `create_funnel(input)` -- creates funnel with steps
- `find_by_id(id)` -- returns funnel with steps
- `find_by_id()` -- returns None
- `find_by_name(name)` -- finds by name
- `list_active()` -- lists active funnels
- `list_all()` -- lists all funnels
- `deactivate(id)` -- deactivates funnel
- `delete(id)` -- deletes funnel
- `record_progress(session_id, funnel_id, step)` -- records step progress
- `get_stats(funnel_id, start, end)` -- returns funnel stats

### 5.6 Read-Only Analytics Repositories (~71 tests)

These repositories are primarily read-only query methods against existing data. Tests verify they execute without error and return expected types.

**OverviewAnalyticsRepository** (8 tests) -- `crates/tests/integration/analytics/repository/overview.rs`
- `get_conversation_count(start, end)`
- `get_agent_metrics(start, end)`
- `get_request_metrics(start, end)`
- `get_tool_metrics(start, end)`
- `get_active_session_count(since)`
- `get_total_session_count(start, end)`
- `get_cost(start, end)`
- All methods with empty date range

**RequestAnalyticsRepository** (5 tests) -- `crates/tests/integration/analytics/repository/requests.rs`
- `get_stats(start, end)`
- `list_models(start, end)`
- `get_requests_for_trends(start, end)`
- `list_requests(start, end, limit, offset)`
- Pagination boundary

**CostAnalyticsRepository** (7 tests) -- `crates/tests/integration/analytics/repository/costs.rs`
- `get_summary(start, end)`
- `get_previous_cost(start, end)`
- `get_breakdown_by_model(start, end)`
- `get_breakdown_by_provider(start, end)`
- `get_breakdown_by_agent(start, end)`
- `get_costs_for_trends(start, end)`
- Empty range returns zeros

**ConversationAnalyticsRepository** (8 tests) -- `crates/tests/integration/analytics/repository/conversations.rs`
- `list_conversations(start, end, limit, offset)`
- `get_context_count(start, end)`
- `get_task_stats(start, end)`
- `get_message_count(start, end)`
- `get_context_timestamps(start, end)`
- `get_task_timestamps(start, end)`
- `get_message_timestamps(start, end)`
- Pagination

**ContentAnalyticsRepository** (4 tests) -- `crates/tests/integration/analytics/repository/content_analytics.rs`
- `get_top_content(start, end, limit)`
- `get_stats(start, end)`
- `get_content_for_trends(start, end)`
- Empty range

**AgentAnalyticsRepository** (9 tests) -- `crates/tests/integration/analytics/repository/agents.rs`
- `list_agents(params)`
- `agent_exists(agent_name, start, end)`
- `get_agent_summary(agent_name, start, end)`
- `get_status_breakdown(agent_name, start, end)`
- `get_top_errors(agent_name, start, end)`
- `get_hourly_distribution(agent_name, start, end)`
- `get_stats(start, end)`
- `get_ai_stats(start, end)`
- `get_tasks_for_trends(start, end)`

**ToolAnalyticsRepository** (8 tests) -- `crates/tests/integration/analytics/repository/tools.rs`
- `list_tools(params)`
- `get_stats(start, end)`
- `tool_exists(tool_name, start, end)`
- `get_tool_summary(tool_name, start, end)`
- `get_status_breakdown(tool_name, start, end)`
- `get_top_errors(tool_name, start, end)`
- `get_usage_by_agent(tool_name, start, end)`
- `get_executions_for_trends(tool_name, start, end)`

**CoreStatsRepository** (14 tests) -- `crates/tests/integration/analytics/repository/core_stats.rs`
- `get_platform_overview()`
- `get_cost_overview()`
- `get_user_metrics_with_trends()`
- `get_activity_trend(days)`
- `get_recent_conversations(limit)`
- `get_content_stats(limit)`
- `get_browser_breakdown(limit)`
- `get_device_breakdown(limit)`
- `get_geographic_breakdown(limit)`
- `get_bot_traffic_stats()`
- `get_top_users(limit)`
- `get_top_agents(limit)`
- `get_top_tools(limit)`
- Zero limit edge case

**TrafficAnalyticsRepository** (6 tests) -- `crates/tests/integration/analytics/repository/traffic.rs`
- `get_sources(start, end, limit)`
- `get_geo_breakdown(start, end, limit)`
- `get_device_breakdown(start, end, limit)`
- `get_bot_totals(start, end)`
- `get_bot_breakdown(start, end, limit)`
- Empty range

**CliSessionAnalyticsRepository** (7 tests) -- `crates/tests/integration/analytics/repository/cli_sessions.rs`
- `get_stats(start, end)`
- `get_active_session_count(since)`
- `get_live_sessions(cutoff)`
- `get_active_count(cutoff)`
- `get_sessions_for_trends(start, end)`
- `get_active_count_since(start)`
- `get_total_count(start, end)`

**AnalyticsQueryRepository** (2 tests) -- `crates/tests/integration/analytics/repository/queries.rs`
- `get_ai_provider_usage(start, end)`
- Empty range

---

## Phase 6: Scheduler + Infrastructure (~20 tests)

### 6.1 JobRepository (~7 tests)

**File:** `crates/tests/integration/scheduler/repository/jobs.rs`

**Source:** `crates/app/scheduler/src/repository/jobs/mod.rs`
**Tables:** `scheduled_jobs`

Methods to test:
- `upsert_job(name, cron, enabled)` -- creates or updates job
- `find_job(name)` -- returns job
- `find_job()` -- returns None for nonexistent
- `list_enabled_jobs()` -- returns enabled jobs
- `update_job_execution(name, timestamp, status)` -- records execution
- `increment_run_count(name)` -- increments
- `upsert_job()` -- update existing job

### 6.2 SecurityRepository (~5 tests)

**File:** `crates/tests/integration/scheduler/repository/security.rs`

**Source:** `crates/app/scheduler/src/repository/security/mod.rs`
**Tables:** `user_sessions` (read-only queries)

Methods to test:
- `find_high_volume_ips(threshold)` -- returns high-volume IPs
- `find_scanner_ips(threshold)` -- returns scanner IPs
- `find_recent_ips()` -- returns recent IPs
- `find_high_risk_country_ips(threshold)` -- returns high-risk country IPs
- All methods with no matching data

### 6.3 SchedulerRepository Facade (~5 tests)

**File:** `crates/tests/integration/scheduler/repository/scheduler.rs`

**Source:** `crates/app/scheduler/src/repository/mod.rs`
**Tables:** `scheduled_jobs`, `user_contexts`

Methods to test:
- `upsert_job()` via facade
- `find_job()` via facade
- `list_enabled_jobs()` via facade
- `update_job_execution()` via facade
- `cleanup_empty_contexts(hours)` -- removes empty contexts

### 6.4 AnalyticsRepository (Scheduler) (~3 tests)

**File:** `crates/tests/integration/scheduler/repository/analytics.rs`

**Source:** `crates/app/scheduler/src/repository/analytics/mod.rs`
**Tables:** `user_contexts`

Methods to test:
- `cleanup_empty_contexts(hours)` -- removes empty contexts
- `cleanup_empty_contexts()` with zero hours
- `cleanup_empty_contexts()` with large hours value

---

## Phase 7: CLI DB Commands

Cross-reference to `21-cli-e2e-roadmap.md`. CLI commands that execute database operations (status, migrate, query) should be tested via subprocess integration tests, not repository tests. This phase is documented separately.

---

## Wave Execution Model

| Wave | Phase | Focus | Agent A | Agent B | Agent C | Est. Tests |
|------|-------|-------|---------|---------|---------|------------|
| 1 | 0 | Infrastructure | domain_factories.rs | db_test_helpers.rs | cleanup.rs extensions | 0 (infra only) |
| 2 | 1.1-1.3 | OAuth core | auth_code (12) | refresh_token (10) | client (28) | ~50 |
| 3 | 1.4-1.7 | OAuth remaining | webauthn (8) | setup_token (11) | user + facade (20) | ~39 |
| 4 | 2.1-2.3 | Agent context/task | context (14) | task (18) | message (10) | ~42 |
| 5 | 2.4-2.6 | Agent services | agent_service (12) | execution (10) | agent (9) | ~31 |
| 6 | 2.7-2.9 + 3.1 | Agent content + MCP sessions | skill (8) + artifact (8) | push_notification (7) | mcp session (10) | ~33 |
| 7 | 3.2-3.4 | MCP + AI | tool_usage (10) | mcp artifact (8) | ai_requests (22) | ~40 |
| 8 | 4.1-4.3 | Content + Links | content (15) | link (9) | link_analytics (8) | ~32 |
| 9 | 4.4-4.5 + 5.1-5.2 | Search + Analytics core | search (5) + file ext (8) | session (25) | fingerprint (12) | ~50 |
| 10 | 5.3-5.6 + 6 | Analytics remaining + Scheduler | events (6) + engagement (6) + funnel (10) | read-only repos (71) | scheduler (20) | ~113 |

**Total estimated: ~430 new tests** (some methods may need fewer or more tests as implementation details become clear)

### Dependencies Between Waves

```
Wave 1 (infra) --> Wave 2 (OAuth needs factories)
                --> Wave 4 (Agent needs factories)
                --> Wave 7 (AI needs factories)
                --> Wave 9 (Analytics needs factories)

Wave 2 (OAuth clients) --> Wave 3 (OAuth auth_code/refresh_token need clients)
Wave 4 (contexts/tasks) --> Wave 5 (execution steps need tasks)
                         --> Wave 6 (artifacts/skills need contexts)
Wave 4 (contexts) --> Wave 7 (MCP/AI link to contexts)
```

Waves 8 and 10 have no upstream dependencies beyond Wave 1.

---

## Verification Protocol

### Per-Wave Verification

After each wave completes:

1. **Build check:**
   ```bash
   cargo test --manifest-path crates/tests/Cargo.toml --workspace --no-run
   ```

2. **Run new tests with DATABASE_URL:**
   ```bash
   DATABASE_URL=postgres://systemprompt_admin:...@localhost:5432/systemprompt-web \
     cargo test --manifest-path crates/tests/Cargo.toml -p <test-crate> -- --nocapture
   ```

3. **Verify no test pollution:** Run twice in sequence. Both runs must pass (tests must clean up after themselves).

4. **Verify no orphaned data:**
   ```sql
   SELECT COUNT(*) FROM oauth_clients WHERE client_id LIKE 'client_test_%';
   SELECT COUNT(*) FROM users WHERE email LIKE 'test_%@example.com';
   SELECT COUNT(*) FROM user_contexts WHERE user_id LIKE 'test_user_%';
   ```

5. **Verify existing tests still pass:**
   ```bash
   cargo test --manifest-path crates/tests/Cargo.toml --workspace
   ```

### Cleanup Verification

Every test must follow the pattern:

```rust
#[tokio::test]
async fn method_name_behavior_description() -> Result<()> {
    let Some(db) = get_db().await else {
        eprintln!("Skipping test (database not available)");
        return Ok(());
    };

    // Setup: create test data with unique IDs
    // Act: call the method under test
    // Assert: verify results
    // Cleanup: delete all test data (even on assertion failure, use guard patterns)

    Ok(())
}
```

Tests must be skippable (return early when no DATABASE_URL) and must never fail due to pre-existing database state.

---

## Test File Organization

### New Crate Structure

Repository tests should live alongside existing integration tests. For domains that already have integration test crates (e.g., `crates/tests/integration/oauth/`), add repository test modules. For domains that need new integration test crates, create them.

**OAuth** -- extend `crates/tests/integration/oauth/`:
```
crates/tests/integration/oauth/src/
  repository/
    mod.rs
    auth_code.rs
    refresh_token.rs
    client.rs
    webauthn.rs
    setup_token.rs
    user.rs
    oauth_facade.rs
```

**Agent** -- create `crates/tests/integration/agents/repository/`:
```
crates/tests/integration/agents/
  repository/
    mod.rs
    context.rs
    task.rs
    message.rs
    agent_service.rs
    execution.rs
    agent.rs
    skill.rs
    artifact.rs
    push_notification.rs
```

**Analytics** -- add to existing analytics directory:
```
crates/tests/integration/analytics/
  repository/
    mod.rs
    session.rs
    fingerprint.rs
    events.rs
    engagement.rs
    funnel.rs
    overview.rs
    requests.rs
    costs.rs
    conversations.rs
    content_analytics.rs
    agents.rs
    tools.rs
    core_stats.rs
    traffic.rs
    cli_sessions.rs
    queries.rs
```

**MCP** -- create:
```
crates/tests/integration/mcp/
  repository/
    mod.rs
    session.rs
    tool_usage.rs
    artifact.rs
```

**AI** -- create:
```
crates/tests/integration/ai/
  repository/
    mod.rs
    ai_requests.rs
```

**Content** -- add to existing content directory:
```
crates/tests/integration/content/
  repository/
    mod.rs
    content.rs
    link.rs
    link_analytics.rs
    search.rs
```

**Scheduler** -- add to existing scheduler directory:
```
crates/tests/integration/scheduler/
  repository/
    mod.rs
    jobs.rs
    security.rs
    scheduler.rs
    analytics.rs
```

### Cargo.toml Updates

Each domain may need new integration test crate members in `crates/tests/Cargo.toml`. Domains that already have integration crates (`integration/oauth`, `integration/extension`) just need module additions. New crates need `[workspace.members]` entries and their own `Cargo.toml` with appropriate `systemprompt-*` dependencies.

---

## Expected Outcomes

### Test Count Impact

| Domain | Repositories | New Tests | Existing Repo Tests |
|--------|-------------|-----------|-------------------|
| OAuth | OAuthRepository, ClientRepository | ~89 | 0 |
| Agent | ContextRepo, TaskRepo, MessageRepo, AgentServiceRepo, ExecutionStepRepo, AgentRepo, SkillRepo, ArtifactRepo, PushNotificationConfigRepo | ~110 | 0 |
| MCP | McpSessionRepo, ToolUsageRepo, McpArtifactRepo | ~28 | 0 |
| AI | AiRequestRepository | ~22 | 0 |
| Content | ContentRepo, LinkRepo, LinkAnalyticsRepo, SearchRepo | ~37 | 0 |
| Analytics | SessionRepo, FingerprintRepo, EventsRepo, EngagementRepo, FunnelRepo, + 11 read-only repos | ~130 | 0 |
| Files | FileRepository (extensions) | ~8 | 19 |
| Users | (already tested) | 0 | 38 |
| Scheduler | JobRepo, SecurityRepo, SchedulerRepo, AnalyticsRepo | ~20 | 0 |
| **Total** | **55+ repositories** | **~444** | **57** |

### Coverage Impact

Repository tests exercise the hottest code paths -- every SQL query, every error branch in data mapping, every transaction boundary. Expected impact:

- **Domain layer line coverage:** significant increase (estimated 15-25 percentage points) since repository code is currently 0% covered by integration tests for most domains
- **SQL query coverage:** from ~12% (3/55 repos) to ~100% of repositories having at least basic CRUD tests
- **Regression safety:** any schema migration that breaks a query will be caught by these tests

### Risk Mitigation

- **Database state:** All tests use unique IDs (UUID-based) and clean up after themselves
- **Parallel safety:** Tests do not share state; each creates its own data
- **CI compatibility:** Tests skip gracefully when DATABASE_URL is not set
- **FK ordering:** Factory and cleanup utilities respect migration weight order
