# systemprompt-brain

Organisational memory for [systemprompt.io](https://systemprompt.io):
revisioned, scoped facts captured from what happens in the platform and
recalled into model requests through the gateway.

Producers append a `BrainEvent` to the durable outbox through the injected
`BrainSink`. This crate owns the `brain_*` tables, their migrations and the
repositories over them: declared kinds, topics, interests and workflow
profiles; subjects and their aliases; facts with an append-only revision
history; the capture ledgers; and the recall, outcome and proposal records.

## Requirements

The schema uses the `pg_trgm` Postgres extension for alias matching and
creates it with `CREATE EXTENSION IF NOT EXISTS pg_trgm;`. `pg_trgm` is a
trusted extension on PostgreSQL 13 and later, so a database owner can create
it. If the migration role cannot, an administrator runs once:

```sql
CREATE EXTENSION pg_trgm;
```

Licensed under the Business Source License 1.1.
