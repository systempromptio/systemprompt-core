# Analytics ownership and reporting

Analytics reports read `report_*` views that analytics declares in
`crates/domain/analytics/schema/report_views.sql`, one per source table its
owners write: `ai_requests`, `agent_tasks`, `task_messages`, `user_contexts`,
`mcp_tool_executions`, `markdown_content`, `analytics_events`, `users` and
`user_sessions` (plus `v_clean_traffic`, `v_engaged_traffic` and
`v_bot_sessions`). Analytics therefore queries only relations it declares,
while nothing is copied: no capture trigger, no outbox consumer, no worker and
no baseline to rebuild, so every report is current as of its query.

## Ownership and interfaces

Users owns session creation, revocation, lifecycle, usage and behavioral session
data through the shared `SessionProvider` and `SessionStore` contracts. Logging
owns analytics-event ingestion and behavioral event lookups through
`AnalyticsEventStore`. Content supplies the authoritative public-page count
through `ContentCatalogStats`. Runtime and entry composition inject these owners
into analytics. Analytics owns request-signal extraction, behavioral analysis,
engagement and fingerprint reputation, and the read-only report queries.

Authentication, behavioral decisions and reports all use owner tables. Reports
may incur read-replica lag. Analytics ingestion preserves caller-visible event
IDs, batch atomicity and `event_data` contents.

## Privacy and retention

Privacy is enforced where the data lives. Deleting a user removes their rows
from every table registered with `user_purge_tables!` in the same transaction as
the `users` row. The report views also hide every row of a user whose `status`
is `deleted`, so a soft-deleted user vanishes from every report at once. Age limits are enforced by retention deleting source
rows (`database_cleanup`, and `expire_user_sessions` for installations that
expire idle sessions from their own retention job).

## History

Up to 0.59 the reports read `analytics_report_*` copies maintained by statement
triggers, an `analytics_reporting` outbox consumer and a one-second projection
worker, with a boot-time baseline rebuild. The copies had one reader, the
`analytics` CLI, and cost a capture write on every source statement. Migration
`analytics/015` and the owner `retire_reporting_capture` migrations remove all of
it; `systemprompt analytics projection` no longer exists.
