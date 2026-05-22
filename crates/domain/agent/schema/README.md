<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://systemprompt.io/files/images/logo.svg">
  <source media="(prefers-color-scheme: light)" srcset="https://systemprompt.io/files/images/logo-dark.svg">
  <img src="https://systemprompt.io/files/images/logo.svg" alt="systemprompt.io" width="180">
</picture>

### Production infrastructure for AI agents

[**Website**](https://systemprompt.io) · [**Documentation**](https://systemprompt.io/documentation/) · [**Guides**](https://systemprompt.io/guides) · [**Core**](https://github.com/systempromptio/systemprompt-core) · [**Template**](https://github.com/systempromptio/systemprompt-template) · [**Discord**](https://discord.gg/wkAbSuPWpr)

</div>

---

# Agent Schema

PostgreSQL schema for the `systemprompt-agent` crate. The tables persist A2A (agent-to-agent) protocol state: contexts, tasks, messages, artifacts, execution steps, and push-notification configuration. Each table lives in its own `.sql` file and is embedded at compile time via `include_str!` in `extension.rs`; versioned migrations live under `schema/migrations/` and are discovered by `build.rs`.

## Tables

| File | Table | Purpose |
|------|-------|---------|
| `user_contexts.sql` | `user_contexts` | Conversation context owned by a user, optionally tied to a session. |
| `agent_tasks.sql` | `agent_tasks` | A2A task state, status, timing, and per-task metadata. |
| `task_messages.sql` | `task_messages` | Ordered messages within a task (`role` is `user` or `agent`). |
| `message_parts.sql` | `message_parts` | Message content parts (`text`, `file`, `data`). |
| `task_artifacts.sql` | `task_artifacts` | Artifacts produced by a task. |
| `artifact_parts.sql` | `artifact_parts` | Artifact content parts (`text`, `file`, `data`). |
| `task_execution_steps.sql` | `task_execution_steps` | Per-step execution trace for a task. |
| `task_push_notification_configs.sql` | `task_push_notification_configs` | Webhook push-notification endpoints per task. |
| `context_agents.sql` | `context_agents` | Agents that have participated in a context. |
| `context_notifications.sql` | `context_notifications` | Queued A2A notifications for a context. |
| `services.sql` | `services` | Service-process registry (name, module, pid, port, status). |
| `user_session_analytics.sql` | — | Reporting views over sessions, contexts, and messages. |

## Conventions

- **Dialect**: PostgreSQL. JSON columns use `JSONB`; timestamps use `TIMESTAMPTZ`; surrogate keys use `SERIAL`; protocol identifiers use `TEXT`.
- **Timestamps**: `created_at` / `updated_at` default to `CURRENT_TIMESTAMP`. `updated_at` is maintained by the shared `update_timestamp_trigger()` function via `CREATE OR REPLACE TRIGGER`.
- **Referential integrity**: child tables reference `user_contexts(context_id)` or `agent_tasks(task_id)` with `ON DELETE CASCADE`.
- **Enumerations**: enforced with `CHECK` constraints rather than native enum types.

## TaskState values

`agent_tasks.status` is constrained to the A2A task-state strings:

```sql
status TEXT NOT NULL DEFAULT 'TASK_STATE_SUBMITTED' CHECK (
    status IN (
        'TASK_STATE_PENDING', 'TASK_STATE_SUBMITTED', 'TASK_STATE_WORKING',
        'TASK_STATE_INPUT_REQUIRED', 'TASK_STATE_COMPLETED', 'TASK_STATE_CANCELED',
        'TASK_STATE_FAILED', 'TASK_STATE_REJECTED', 'TASK_STATE_AUTH_REQUIRED',
        'TASK_STATE_UNKNOWN'
    )
)
```

## Part kinds

`message_parts.part_kind` and `artifact_parts.part_kind` are constrained to `text`, `file`, or `data`. Text content lands in `text_content`, file references in the `file_*` columns, and structured payloads in `data_content` (`JSONB`).

## Migrations

Schema changes are applied as numbered migrations:

```
schema/
├── *.sql                 # current table definitions (embedded via include_str!)
└── migrations/
    └── NNN_<name>.sql     # ordered, discovered by build.rs
```

The crate's `build.rs` calls `systemprompt_extension::build::emit_migrations()`, and the migrations are returned through the `extension_migrations!()` macro. SQL string constants and hand-written `Migration::new(...)` lists are not used.

## References

- [Agent2Agent Protocol Specification](https://a2aprotocol.ai)
- [PostgreSQL JSON functions](https://www.postgresql.org/docs/current/functions-json.html)

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Agent schema reference · Own how your organization uses AI.</sub>

</div>
</content>
</invoke>
