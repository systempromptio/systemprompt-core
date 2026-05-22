# Operate in production

How to run a deployed systemprompt instance day to day: probe its health, scrape its metrics, read its logs, ingest OpenTelemetry (OTLP) data, troubleshoot common failures, and upgrade or roll back. For the deployment topology this guide assumes — replicas, Postgres, reverse proxy, secrets store — see [deploy-production.md](deploy-production.md).

## Prerequisites

- A running deployment configured per [configure.md](configure.md).
- Network reachability to the API port (default `8080`) from your probe, scrape, and operator tooling.
- A Prometheus-compatible scraper and a log forwarder (Fluent Bit, Vector, or equivalent) if you forward logs off-host.

## 1. Wire liveness and readiness probes

The binary exposes three health surfaces. There are no `/livez`, `/readyz`, `/healthz`, or `/health/live`/`/health/ready` aliases — wire orchestrator probes against the real endpoints (`crates/entry/api/src/services/server/discovery.rs:166-167,177`).

| Endpoint | Auth | Cost | Returns |
|----------|------|------|---------|
| `GET /health` | none | `SELECT 1` round-trip | `200` with `{"status":"healthy"}`, or `503` with `{"status":"unhealthy"}` if the DB is unreachable |
| `GET /api/v1/health` | none | `SELECT 1` round-trip | same as `/health` |
| `GET /api/v1/health/detail` | authenticated | DB size and latency, top-15 table sizes, process memory, disk usage | rich JSON |

`/health` runs a `SELECT 1` against the database on every call (`crates/entry/api/src/services/server/health.rs:185-198`). A `200` therefore means the process is up *and* the database is reachable, so the same endpoint serves as both the liveness and the readiness signal.

```bash
curl -fsS http://127.0.0.1:8080/health
# {"status":"healthy"}
```

Kubernetes probes target `/health` for both checks:

```yaml
livenessProbe:
  httpGet: { path: /health, port: 8080 }
  periodSeconds: 10
readinessProbe:
  httpGet: { path: /health, port: 8080 }
  periodSeconds: 5
```

`/api/v1/health/detail` requires authentication and is not usable as an unauthenticated probe. Use it for operator dashboards and deeper checks, not for the load balancer:

```bash
curl -fsS -H "Authorization: Bearer $TOKEN" \
  http://127.0.0.1:8080/api/v1/health/detail
```

The detail handler reports `degraded` when the static-content files (`index.html`, `sitemap.xml`) are absent from the web dist directory. A headless, API-only deployment that serves no static site therefore reports `degraded` while being fully functional; treat `degraded` from a headless deployment as expected, and alert on the `/health` `503` instead.

## 2. Account for the absence of graceful shutdown

The main HTTP API server calls `axum::serve` without a graceful-shutdown future (`crates/entry/api/src/services/server/builder.rs`). On `SIGTERM` — container stop, rolling deploy, `systemctl restart` — the process terminates mid-flight: in-flight requests are dropped and Server-Sent Events (SSE) connections are severed without notice. The readiness flag is never flipped, so a load balancer relying on an in-process draining signal does not get one.

Mitigate at the orchestration layer until graceful shutdown is wired:

1. Set a `preStop` hook (or an equivalent drain delay) that removes the replica from the load balancer and waits before `SIGTERM` is sent. Size the delay to cover your longest expected non-stream request, plus margin; this bounds request loss.
2. Keep the readiness probe pointed at `/health`. Combined with the `preStop` drain, the load balancer stops sending new traffic before the process exits.
3. Treat SSE streams as transient across deploys: clients re-fetch canonical state on reconnect (see §6 on SSE replay).

The A2A (agent-to-agent) agent server does wire graceful shutdown; this caveat is specific to the main API surface.

## 3. Scrape Prometheus metrics

The binary serves `GET /metrics` in Prometheus exposition format (`text/plain; version=0.0.4; charset=utf-8`). The recorder is installed and the route mounted unconditionally — `/metrics` is always exposed on the API port (`crates/entry/api/src/services/server/metrics.rs:19-31`, `discovery.rs:160-161`).

The endpoint carries no scrape authentication and sits on the public discovery router. Restrict it at the reverse-proxy or network layer: allow only your scrape mesh to reach `/metrics`, or front it with a proxy that requires a scrape token. Route labels and traffic volume are visible to anyone who can reach the port.

```bash
curl -fsS http://127.0.0.1:8080/metrics | head
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

## 4. Read structured logs

Logs are emitted as structured JSON to stdout and persisted to the `logs` table in Postgres (`crates/infra/logging/src/layer/mod.rs`). The verbosity is set by `runtime.log_level` in the profile (`quiet`→`error`, `normal`→`info`, `verbose`→`debug`, `debug`→`trace`); set `runtime.output_format: json` for machine ingestion.

Each log line carries a `trace_id` for correlation. The request middleware threads the request's `trace_id` through the span chain into every log line and echoes it back to the client as the `x-trace-id` response header (`crates/entry/api/src/services/middleware/trace.rs`). To trace one request end to end, read `x-trace-id` from the response and filter logs or the `logs.trace_id` column on that value. Span context also propagates `user_id`, `session_id`, `task_id`, `context_id`, and `client_id` into dedicated `logs` columns (`crates/infra/logging/src/layer/visitor.rs`).

Secret-bearing fields are redacted before emission. The log field visitor matches an allow-list — `password`, `token`, `*_token`, `authorization`, `cookie`, `client_secret`, `private_key`, and similar — across string, debug, integer, and boolean field types, and strips ANSI escapes from messages (`crates/infra/logging/src/layer/visitor.rs:15-37`).

Forward logs off-host by one of:

- **Structured log egress** — forward the stdout JSON via Fluent Bit, Vector, or similar to your SIEM.
- **Logical replication** — subscribe a downstream system to the `logs` table for high volume.
- **Pull-based export** — periodic query against a read-only replica.

## 5. Ingest OpenTelemetry (OTLP)

The gateway exposes an OTLP ingest endpoint at `POST /otel` (and `POST /otel/{*rest}`) that decodes OTLP trace, log, and metric envelopes and persists spans and logs as rows in the `logs` table (`crates/entry/api/src/routes/gateway/otel.rs`). It accepts protobuf envelopes up to 4 MiB and auto-detects the envelope type. The endpoint is unauthenticated by design: it is gated to a loopback origin by the bridge proxy, which is the only intended client. Do not expose `/otel` to untrusted networks.

Two limits to account for when planning telemetry:

- **The server ingests OTLP but does not emit its own distributed traces.** There is no OTLP exporter in the binary. Cross-service correlation uses the `logs.trace_id` column and the ingested span rows, not an external collector (Jaeger/Tempo) fed by the server.
- **OTLP-ingested metrics are not recorded.** The metrics path of the ingest endpoint counts and logs metric names only; values are discarded and never appear on `/metrics` or in the database. Only ingested traces and logs are persisted.

## 6. Account for SSE delivery semantics

Cross-replica event delivery is durable: routed events are written to an `event_outbox` table and announced via Postgres `NOTIFY`, and peer replicas re-inject them locally (`crates/infra/events/src/services/repository.rs`). The final hop to a connected SSE client is best-effort: delivery uses a non-blocking send on a bounded per-connection channel, and on a full or closed channel the event is dropped and the connection evicted (`crates/infra/events/src/services/broadcaster.rs:136-154`). There is no per-connection replay-from-offset.

A slow or briefly disconnected SSE client silently misses events. SSE alone is not an at-least-once channel. Where a client must not miss state, have it re-fetch canonical state on reconnect rather than relying on the stream for catch-up.

## 7. Troubleshoot common failures

### Authentication failures (401 / 403)

The JWT plane is RS256-only. Tokens signed with any other algorithm, or presenting `alg: none`, are rejected (`crates/infra/security/src/auth/validation.rs`). Common causes:

- **Wrong or missing `kid`.** The verifier requires a `kid` header and matches it against the published JWKS at `/.well-known/jwks.json`. After a signing-key rotation, a token minted under the old `kid` validates only while the JWKS still carries the prior public key.
- **Clock skew.** `exp`/`nbf`/`iat` are checked with a 30-second leeway. A client clock more than 30 seconds off produces spurious 401s; sync NTP.
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

Migrations are additive-only within a minor version. A failed migration leaves the schema at the last successful step; re-running `migrate` resumes from there. Confirm connectivity and the role's privileges first with `systemprompt infra db status` — a migration that cannot create or alter a table usually indicates a least-privilege role missing DDL grants on its schema.

### Provider / gateway errors (502 / 429)

Gateway requests to `/v1/messages` map upstream failures as follows (`crates/entry/api/src/routes/gateway/messages/dispatch.rs`):

| Symptom | Cause | Action |
|---------|-------|--------|
| `404 Gateway not enabled` | `gateway.enabled` is `false` or absent | Enable the gateway in the profile (see [configure-providers.md](configure-providers.md)). |
| `404 No gateway route matches model` | No `routes[*].model_pattern` matches the requested model | Add or widen a route pattern. |
| `403` policy denied | The requested model is not in the gateway policy's allowed list | Adjust the gateway policy. |
| `429` quota exceeded | A per-user quota window is exhausted; a `retry-after` header is set | Back off until the window resets, or raise the quota. |
| `502 Bad Gateway` | The upstream provider returned a non-2xx or the connection failed | Inspect the gateway access log for the upstream status and body; verify `api_key_secret` and `endpoint`. |
| `503 Profile not ready` / API key secret not configured | The named `api_key_secret` is missing from the secrets document | Add the secret and reload. |

Every gateway request is access-logged with method, path, status, and elapsed time to both stdout and the `logs` table (`crates/entry/api/src/routes/gateway/mod.rs:29-85`), keyed by an `x-systemprompt-request-id` response header for correlation.

## 8. Upgrade and roll back

### Upgrade

1. Obtain and verify the new release through your supply-chain process.
2. Review [CHANGELOG.md](../../CHANGELOG.md) for the target version; scan for breaking-change entries.
3. Preview migrations with `systemprompt infra db migrate-plan`, then apply with `systemprompt infra db migrate`.
4. Replace one replica at a time. Wait for `GET /health` to return `200` before proceeding to the next, and use a `preStop` drain (§2) to bound request loss during each replacement.

### Roll back

Migrations are additive-only within a minor version, so rolling back by one minor is supported:

1. Replace the binary with the previous version.
2. Rolling-restart, draining each replica first.
3. Columns added by the newer version are ignored by the older binary.

Rolling back across more than one minor version requires a point-in-time restore from backup; see [deploy-production.md](deploy-production.md).

## Next steps

- [deploy-production.md](deploy-production.md) — topology, HA, backup/restore, key rotation.
- [configure.md](configure.md) — the profile that drives this deployment.
- [configure-providers.md](configure-providers.md) — configure AI providers through the gateway.
