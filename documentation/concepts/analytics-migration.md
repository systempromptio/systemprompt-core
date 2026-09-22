# Analytics ownership and durable reporting

Analytics reports read analytics-owned `analytics_report_*` tables. Source owners
publish explicit reporting columns through `reporting_source_*` views and
transactional PostgreSQL triggers. The existing `event_outbox` carries these
facts; no additional broker or event-log table is required.

## Ownership and interfaces

Users owns session creation, revocation, lifecycle, usage and behavioral session
data through the shared `SessionProvider` and `SessionStore` contracts. Logging
owns analytics-event ingestion and behavioral event lookups through
`AnalyticsEventStore`. Content supplies the authoritative public-page count
through `ContentCatalogStats`. Runtime and entry composition inject these owners
into analytics. Analytics owns request-signal extraction, behavioral analysis,
engagement, fingerprint reputation, funnels and reporting projections.

Authentication and behavioral decisions use primary owner stores. Reports are
eventually consistent and may also incur read-replica lag. Analytics ingestion
preserves caller-visible event IDs, batch atomicity and `event_data` contents.
The Rust constructors require the corresponding owner handles; callers must
update their composition when upgrading.

## Capture and processing

Users, agent, AI, MCP, content and logging publish version-one `reporting.row`
facts for the `analytics_reporting` consumer. Facts contain source identity,
entity key, revision and an explicit column snapshot or deletion marker. Updates
to unreported columns produce no fact. A key change emits a tombstone for the old
key and a snapshot for the new key. Source changes and facts commit or roll back
together.

The runtime worker polls once per second and processes up to 256 deliveries per
pass. A full pass schedules another immediately. Each delivery is claimed with a
transactional row lock; projection changes and acknowledgement commit together.
The projector ignores revisions already included in the baseline and older or
duplicate entity revisions. Individual pending rows remain discoverable even
when transactions commit in a different order from their revision allocation.
Unsupported versions and malformed facts remain pending and generate a processing
error. Outbox cleanup retains unacknowledged deliveries.
A failing delivery stops its processing pass and is retried; it can hold up the
queue until the producer, decoder, or stored fact is repaired. Rebuild does not
discard unsupported or malformed deliveries.

Reporting rows use a separate `reporting` channel inside the existing outbox.
The SSE bridge skips that channel. Existing SSE channels, payloads, local fan-out,
context fan-out and origin suppression retain their delivery contracts.
PostgreSQL notifications continue serving SSE delivery; reporting polling does
not depend on receiving them.

## Initialization and rebuild

`systemprompt_runtime::reporting::initialize` builds a baseline when none exists;
`rebuild` refreshes it explicitly. Both use the primary database. The server
does not wait for the baseline: it binds and serves while its reporting task
builds it, and reports refuse with an in-progress message until it lands.

A rebuild runs in fenced, committed phases rather than one transaction:

1. **Fence.** Under the projector advisory lock and shared locks on every source
   table, mint the cutoff revision and open a new generation. Source writes
   wait only for this step, which takes milliseconds.
2. **Clear.** Truncate the projection tables in their own transaction.
3. **Snapshot.** For each source, write the retained rows of its owner view in
   keyset pages of 10 000, one transaction and one set-based statement per
   page, heartbeating the state row after each. Pages read live committed
   rows under the projector lock, so a privacy delivery made between pages is
   never undone by a stale snapshot. A change captured after the fence also
   has a fact above the cutoff, which the worker re-applies afterwards.
4. **Finish.** Flip `initialized` if the generation is still the live one.

`analytics projection status` shows the phase (`rebuild_source`,
`rebuild_rows`, `rebuild_heartbeat_at`). A node that finds a fresh heartbeat
leaves the running rebuild alone; a heartbeat older than two minutes is taken
over from the fence. A failed or interrupted rebuild leaves the projection
uninitialized (and its tables empty) until the retry succeeds — the projection
is a derived cache, and the server retries automatically. The generation
counter records every fence but does not keep a historical generation for
rollback.

Rebuild needs the source tables and views. The outbox is a delivery queue, not
permanent reporting history. Upgrade all SSE relays to understand durable
retention and the reporting channel before enabling capture. Apply schema
installation before running reporting initialization or starting workers.
Runtime initialization enables capture automatically. For an upgrade from a
release without durable retention, stop the old replicas, apply migrations,
then start the upgraded replicas; mixed-version rolling operation is not
supported because an old cleanup worker can remove pending facts.
Initial capture installation and explicit rebuild require privileges to create
views and triggers on source tables. Normal initialized startup skips this DDL.
Direct source `TRUNCATE` or writes with triggers disabled require a rebuild;
normal row inserts, updates and deletes are captured transactionally.
Per-entity revisions, including deletion tombstones, remain until rebuild so late
delivery cannot resurrect removed rows. Rebuild clears that revision history and
establishes a new cutoff. Future reporting-contract upgrades must reinstall
capture and rebuild explicitly, or invalidate the baseline in their migration;
initialized startup does not refresh capture definitions automatically.

## Operations and verification

The CLI exposes `analytics projection status`, `analytics projection sync --limit
10000`, and `analytics projection rebuild`. Status is read-only; sync applies a
bounded number of pending facts. Report commands do not implicitly rebuild or
drain the queue.
Report commands reject an uninitialized baseline with a rebuild instruction
instead of returning empty reports.

`reporting::status` exposes initialization, generation, rebuild time, pending
count, oldest pending time and the latest retained acknowledgement time. Worker
metrics include `analytics_projection_processed_total`,
`analytics_projection_failures_total`, `analytics_projection_pending` and
`analytics_projection_oldest_pending_seconds`. Processing failures are retried;
the pending count and oldest age distinguish a quiet queue from stalled work.

The events integration regression exercises transactional capture using logging
schemas, unreported-column updates, nulls, key changes, deletions, late commits
and reporting-channel isolation while preserving the existing four SSE channels:

```bash
cargo test --manifest-path crates/tests/integration/events/Cargo.toml -- --test-threads=1
```

The command requires `DATABASE_URL` for a test PostgreSQL database.
