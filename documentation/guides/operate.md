# Operate in production

How to run a deployed systemprompt instance day to day: probe its health, scrape its metrics, read its logs, ingest and export OpenTelemetry (OTLP) data, troubleshoot common failures, and upgrade or roll back. For the deployment topology this guide assumes — replicas, Postgres, reverse proxy, secrets store — see [deploy-production.md](deploy-production.md).

## Prerequisites

- A running deployment configured per [configure.md](configure.md).
- Network reachability to the API port (default `8080`) from your probe, scrape, and operator tooling.
- A Prometheus-compatible scraper and a log forwarder (Fluent Bit, Vector, or equivalent) if you forward logs off-host.

## 1. Configure liveness and readiness probes

| Endpoint | Authentication | Behavior |
|----------|----------------|----------|
| `GET /livez` | None | Returns 200 once the listener is bound, independently of database availability |
| `GET /readyz` | None | Returns 503 during bootstrap, shutdown or database-probe failure; returns 200 when ready |
| `GET /health`, `GET /api/v1/health` | None | Operational health summary; returns a starting response during bootstrap |
| `GET /api/v1/health/detail` | Required | Database, storage and process diagnostics |

Use dedicated probes so a database outage removes a replica from service without causing
liveness-driven restart loops. See `crates/entry/api/src/services/server/probes.rs` and
`startup.rs`.

Once booted, `/livez`, `/readyz` and the health endpoints carry `version` (the core
release) and `build`: the name, version and optional source revision of the binary that is
serving. A downstream binary sets `build` by starting through `systemprompt::cli::run_with`;
one that starts through `run` reports core itself. Use `build` to confirm which image a
replica is running without asking the orchestrator.

```yaml
livenessProbe:
  httpGet: { path: /livez, port: 8080 }
  periodSeconds: 10
readinessProbe:
  httpGet: { path: /readyz, port: 8080 }
  periodSeconds: 5
```

```bash
curl -fsS http://127.0.0.1:8080/readyz
curl -fsS -H "Authorization: Bearer $TOKEN" \
  http://127.0.0.1:8080/api/v1/health/detail
```

Interpret detailed health fields against the enabled deployment features. For example,
missing static-site artifacts require investigation only when the deployment serves them.

## 2. Configure shutdown and draining

Ctrl-C or Unix SIGTERM marks the API unready (`/readyz` answers `503 draining`) at once.
The listener keeps accepting for `server.shutdown.pre_stop_delay_secs`, so the balancer
can deregister the replica first, then stops accepting and drains in-flight requests and
SSE streams for the full `drain_grace_secs`. Teardown then stops the scheduler,
terminates registered children (`child_grace_secs`) and drains owned background work
(`background_grace_secs`), which includes a final settlement pass over the gateway
accounting journal. `forced_exit_grace_secs` bounds that teardown. A second signal
exits immediately.

```yaml
server:
  shutdown:
    pre_stop_delay_secs: 0      # accept while endpoints update
    drain_grace_secs: 10        # longest request or stream to wait for
    child_grace_secs: 5         # MCP and agent children
    background_grace_secs: 5    # owned background work and journal flush
    forced_exit_grace_secs: 10  # backstop for the teardown; must exceed both graces above
```

The values above are the defaults. At boot the server logs the total budget
(`pre_stop_delay_secs + drain_grace_secs + 2 + forced_exit_grace_secs`); set
`terminationGracePeriodSeconds` above it. With `pre_stop_delay_secs` set, no
`preStop` `sleep` hook is needed. Set `drain_grace_secs` to the longest stream you
are prepared to wait for. A gateway stream that outlasts it is ended with a
terminal error event in its own wire format telling the client to retry, and gets
2 seconds to flush before the connection closes;
`gateway_streams_aborted_on_shutdown_total` counts them. The client must reconnect
and fetch canonical state.

A replica that drains in order leaves its gateway journal empty. The
`gateway_journal_receipts` gauge reports the receipts still on disk;
`gateway_journal_settle_failures_total` and `gateway_journal_recovery_failures_total`
count receipts kept because settlement failed and recovery passes that failed.


## 3. Scrape Prometheus metrics

The Prometheus endpoint is served at `GET /metrics` on the separate listener configured by `server.metrics_port`. When that setting is unset, no metrics listener is started. The endpoint has no scrape authentication; restrict access to the configured port using network policy or an authenticated proxy.

```bash
curl -fsS http://127.0.0.1:9100/metrics | head
```

Recorded series (`metrics.rs:14-79`):

| Metric | Type | Labels | Use |
|--------|------|--------|-----|
| `http_requests_total` | counter | `method`, `path`, `status` | request and error rate |
| `http_request_duration_seconds` | histogram | `method`, `path`, `status` | p50/p95/p99 latency |
| `http_requests_in_flight` | gauge | — | concurrency / saturation |
| `sse_active_connections` | gauge | `channel` (`context`, `agui`, `a2a`, `analytics`) | live SSE stream counts |

The `path` label is the matched route template (for example `/api/v1/agents/{agent_id}`), not the raw URI, so per-ID cardinality stays bounded for matched routes. The four `sse_active_connections` gauges are refreshed live from the event broadcasters on each scrape (`metrics.rs:33-43`).

A minimal scrape job:

```yaml
scrape_configs:
  - job_name: systemprompt
    metrics_path: /metrics
    static_configs:
      - targets: ["systemprompt-1:8080", "systemprompt-2:8080"]
```

Suggested alerts:

- 5xx fraction of `http_requests_total` climbing for 5 minutes — warn.
- p99 of `http_request_duration_seconds` over your latency SLO for 10 minutes — page.
- `http_requests_in_flight` approaching the replica's concurrency ceiling — warn (saturation).

Gateway spend controls add two counters. `systemprompt_quota_denials_total{subject_kind, dimension, mode}` counts every quota-window breach (`mode` is `enforce` or `warn`). `systemprompt_usage_anomaly_total{kind, subject_kind}` is raised by the `usage_anomaly_scan` job (enable it under that name in the scheduler config; it runs at two minutes past each hour): for the previous closed hour it compares each user's completed-request cost and count with their trailing seven-day hourly average and flags `kind="spend"` at three times the baseline and at least 1,000,000 microdollars, `kind="request_rate"` at three times the baseline and at least 50 requests. Each flag also logs a `usage_anomaly` warning (target `usage_anomaly`) carrying `user_id`, `observed`, `baseline`, `ratio` and `window_start`. Alert on any increase of either counter.

## 4. Read structured logs

Logs are written to **stderr** and persisted to the `logs` table in Postgres (`crates/infra/logging/src/layer/mod.rs`). The verbosity is set by `runtime.log_level` in the profile (`quiet`→`error`, `normal`→`info`, `verbose`→`debug`, `debug`→`trace`). The stderr format is `observability.log_format`: `text` (the default, `key=value` fields) or `json`, one object per line with `timestamp`, `level`, `target`, `fields`, `spans` and `instance` — set `json` for machine ingestion. `runtime.output_format` affects CLI command output only.

Each log line carries a `trace_id` for correlation. The request middleware threads the request's `trace_id` through the span chain into every log line and echoes it back to the client as the `x-trace-id` response header (`crates/entry/api/src/services/middleware/trace.rs`). A request that carries no `x-trace-id` but a W3C `traceparent` header takes its trace id from `traceparent`, and the gateway sends a `traceparent` with the same trace id on its provider calls, so a caller's distributed trace, the gateway's audit rows and the provider's request logs share one id. To trace one request end to end, read `x-trace-id` from the response and filter logs or the `logs.trace_id` column on that value. Span context also propagates `user_id`, `session_id`, `task_id`, `context_id`, and `client_id` into dedicated `logs` columns (`crates/infra/logging/src/layer/visitor.rs`).

Secret-bearing fields are redacted before emission. The log field visitor matches an allow-list — `password`, `token`, `*_token`, `authorization`, `cookie`, `client_secret`, `private_key`, and similar — across string, debug, integer, and boolean field types, and strips ANSI escapes from messages (`crates/infra/logging/src/layer/visitor.rs:15-37`).

Forward logs off-host by one of:

- **Structured log egress** — set `observability.log_format: json` and forward the stderr stream via Fluent Bit, Vector, or similar to your SIEM.
- **Logical replication** — subscribe a downstream system to the `logs` table for high volume.
- **Pull-based export** — periodic query against a read-only replica.

## 5. Ingest and export OpenTelemetry (OTLP)

The gateway exposes an OTLP ingest endpoint at `POST /otel` (and `POST /otel/{*rest}`) that decodes OTLP trace, log, and metric envelopes and persists spans and logs as rows in the `logs` table (`crates/entry/api/src/routes/gateway/otel/`). It accepts protobuf envelopes up to 4 MiB and auto-detects the envelope type. Review the gateway route authentication and restrict telemetry ingress to intended clients. The bridge enforces its local proxy origin before forwarding; that control does not restrict direct network access to the API.

### Export to a collector

The instance can also *emit* OTLP. With a `observability.otlp` block in the profile ([configuration.md §observability](../reference/configuration.md#observability)) and a server built with the `otel` feature, the server exports its own `tracing` pipeline live as OTLP/HTTP protobuf envelopes (`crates/infra/logging/src/otlp/`):

- **Logs** — one record per log entry the server persists (level → severity, module, message, metadata, user/session/task/context/client/instance ids), carrying the OTLP trace id derived from the platform trace id.
- **Traces** — one span per `tracing` span that runs under a platform trace, with its start and end time, its parent span and the attribution it inherits (user, session, task, context, client). A platform trace id that is a 32-hex W3C id is kept, so client-side spans correlate; every other id is digested to 16 bytes, so every span and record of one trace share one OTLP trace id.
- **Audit spans** — the audit facts, each handed to the exporter the moment it is recorded:
  - `ai_request <model>` (kind `SERVER`), one per settled gateway request: `systemprompt.request.id`, `systemprompt.request.kind`, `systemprompt.request.status`, `enduser.id`, `systemprompt.actor.kind`, `systemprompt.actor.id`, `session.id`, `systemprompt.context.id`, `systemprompt.trace.id`, `gen_ai.system`, `systemprompt.served_provider`, `gen_ai.request.model`, `gen_ai.response.model`, `systemprompt.route`, `gen_ai.usage.input_tokens`, `gen_ai.usage.output_tokens`, `gen_ai.usage.cache_read_tokens`, `gen_ai.usage.cache_creation_tokens`, `systemprompt.cost.usd`, `systemprompt.cost.microdollars`, `systemprompt.latency_ms`, `systemprompt.upstream_latency_ms`, `gen_ai.response.finish_reason`, `systemprompt.client.kind`, `systemprompt.wire_protocol`, `systemprompt.instance.id`, `systemprompt.api_key.id` (the key's id, never the key), and per attributed scope `systemprompt.scope.<dimension>` with `systemprompt.scope.<dimension>.source`. A failed request has status `ERROR` with its error message.
  - `tool_call <tool>` (kind `INTERNAL`), one per MCP execution whose end is recorded (in-process, proxied, or reported by a client hook): `gen_ai.tool.name`, `gen_ai.tool.call.id`, `systemprompt.mcp.server`, `systemprompt.mcp.execution_id`, `systemprompt.tool.state`, `systemprompt.tool.source`, `systemprompt.tool.status`, `systemprompt.tool.execution_ms`, `systemprompt.artifact.type`, `systemprompt.artifact.bytes`, `systemprompt.artifact.secret_redactions`, `systemprompt.artifact.is_error`.
  - `governance_decision <policy>` (kind `INTERNAL`, zero duration), one per recorded authorization decision: `systemprompt.governance.decision`, `systemprompt.governance.policy`, `systemprompt.governance.reason`, `gen_ai.tool.name`, `gen_ai.tool.call.id`, `systemprompt.plugin.id`, `systemprompt.actor.kind`, `systemprompt.actor.id`. A deny has status `ERROR` with its reason.

  All three share the request's OTLP trace id. Absent values are omitted rather than sent empty. A child span names the `ai_request` span as its parent when the request is known where the fact is recorded; a tool execution or a decision is recorded without it, so those spans join the trace as roots and correlate to the request through `gen_ai.tool.call.id` and the trace id. Artifact attributes are set only where the execution record carries them. A request settled by crash recovery rather than by the request that produced it emits no span. No attribute carries secret material.
- **Metrics** are not exported. They stay on `/metrics`; a collector that wants them scrapes it (`prometheusreceiver`).

Any collector that speaks OTLP/HTTP works — the OpenTelemetry Collector fronting Datadog, Splunk, Grafana Tempo and Loki, Honeycomb, or the vendor agent directly. `protocol: grpc` is not yet supported and is refused at validation.

Export is live and at-most-once. Items wait in a bounded queue (8,192 items) and ship every `batch_seconds` or every 512 items; a full queue drops the item rather than slow the code that emitted it. A batch is retried three times with increasing backoff on transport errors and 429/502/503/504 (never on other 4xx), and is then dropped. Every drop is counted in `otlp_export_dropped_total{signal}`, and every post in `otlp_export_batches_total{signal,status}` (`ok` or `error`), both on `/metrics`. What is still queued at shutdown is flushed before the process exits. The audit tables remain the durable record; the export is the live view of it.

One limit to account for when planning telemetry: **OTLP-ingested metrics are not recorded.** The metrics path of the ingest endpoint counts and logs metric names only; values are discarded and never appear on `/metrics` or in the database. Only ingested traces and logs are persisted.

## 6. Account for SSE delivery semantics

Cross-replica event delivery is durable: routed events are written to an `event_outbox` table and announced via Postgres `NOTIFY`, and peer replicas re-inject them locally (`crates/infra/events/src/services/repository.rs`). The final hop to a connected SSE client is best-effort: delivery uses a non-blocking send on a bounded per-connection channel, and on a full or closed channel the event is dropped and the connection evicted (`crates/infra/events/src/services/broadcaster.rs:136-154`). There is no per-connection replay-from-offset.

A slow or briefly disconnected SSE client silently misses events. SSE alone is not an at-least-once channel. Where a client must not miss state, have it re-fetch canonical state on reconnect rather than relying on the stream for catch-up.

## 7. Troubleshoot common failures

### Authentication failures (401 / 403)

The JWT plane is RS256-only. Tokens signed with any other algorithm, or presenting `alg: none`, are rejected (`crates/infra/security/src/auth/validation.rs`). Common causes:

- **Wrong or missing `kid`.** The verifier requires a `kid` header and matches it against the published JWKS at `/.well-known/jwks.json`. The in-process authority accepts one active signing key. Tokens with the previous `kid` require reauthentication after key replacement.
- **Clock skew.** `exp`/`nbf` are checked with a 30-second leeway. A client clock more than 30 seconds off produces spurious 401s; sync NTP.
- **Authorization denied (403).** Authorization is fail-closed. If the `governance.authz` hook is in `webhook` mode and the policy endpoint returns a transport error, a non-2xx, or an undecodable body, the request is denied. Check the authz hook's reachability and the audit log for the decision.
- **Open registration disabled.** If `security.allow_registration` is `false`, registration attempts are rejected by design.

### Migration issues

Preview pending migrations without writing to the database:

```bash
systemprompt infra db migrate-plan
```

Apply them:

```bash
systemprompt infra db migrate
```

Migrations follow the expand/contract contract (see [deploy-production.md](deploy-production.md#93-schema-compatibility-during-a-rolling-update)). A failed migration leaves the schema at the last successful step; re-running `migrate` resumes from there. Confirm connectivity and the role's privileges first with `systemprompt infra db status` — a migration that cannot create or alter a table usually indicates a least-privilege role missing DDL grants on its schema.

### Provider / gateway errors (502 / 429)

Gateway requests to `/v1/messages` map upstream failures as follows (`crates/entry/api/src/routes/gateway/messages/dispatch/`):

| Symptom | Cause | Action |
|---------|-------|--------|
| `404 Gateway not enabled` | `gateway.enabled` is `false` or absent | Enable the gateway in services configuration (see [configure-providers.md](configure-providers.md)). |
| `404 No gateway route matches model` | No `routes[*].model_pattern` matches the requested model | Add or widen a route pattern. |
| `403` policy denied | The requested model is not in the gateway policy's allowed list | Adjust the gateway policy. |
| `429` quota exceeded | A quota window (per user, API key or scope dimension) is exhausted; `retry-after` is the time to the window reset and `error.quota` names the window, subject, dimension, limit and usage | Back off until `error.quota.resets_at`, or raise the quota. |
| `502 Bad Gateway` | The upstream provider returned a non-2xx or the connection failed | Inspect the gateway access log for the upstream status and body; verify `api_key_secret` and `endpoint`. |
| `503 Profile not ready` / API key secret not configured | The named `api_key_secret` is missing from the secrets document | Add the secret and restart the affected process. |

Every gateway request is access-logged with method, path, status, and elapsed time to both stdout and the `logs` table (`crates/entry/api/src/routes/gateway/mod.rs:29-85`), keyed by an `x-systemprompt-request-id` response header for correlation.

## 8. Upgrade and roll back

### Upgrade

1. Obtain and verify the new release through your supply-chain process.
2. Review [CHANGELOG.md](../../CHANGELOG.md) for the target version; scan for breaking-change entries.
3. Preview migrations with `systemprompt infra db migrate-plan`, then apply with `systemprompt infra db migrate`.
4. Replace one replica at a time. Wait for `GET /readyz` to return `200` before proceeding to the next, and use a `preStop` drain (§2) to bound request loss during each replacement.

### Roll back

Check the target release’s migration and rollback requirements before changing binaries:

1. Replace the binary with the previous version.
2. Rolling-restart, draining each replica first.
3. Confirm that the older binary accepts the resulting schema and configuration; apply documented down migrations or restore a backup when required.

Rolling back across more than one minor version requires a point-in-time restore from backup; see [deploy-production.md](deploy-production.md).

## Next steps

- [deploy-production.md](deploy-production.md) — topology, HA, backup/restore, key rotation.
- [configure.md](configure.md) — the profile that drives this deployment.
- [configure-providers.md](configure-providers.md) — configure AI providers through the gateway.
