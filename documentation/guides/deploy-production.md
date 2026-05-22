# Deploy in production

How to operate systemprompt in production for availability, durability, and recoverability. This is a reference architecture — adapt it to your platform (Kubernetes, VMs, bare metal) and compliance programme.

## Prerequisites

- A built or downloaded systemprompt release binary.
- PostgreSQL 18+ with a primary you can configure for replication and WAL archiving.
- A TLS-terminating reverse proxy.
- A secrets-management system (KMS, HSM, Vault, or sops) you already operate.
- A configured `profile.yaml` — see [configure.md](configure.md).

## 1. Minimum production topology

A production deployment consists of:

1. **systemprompt binary** — stateless Rust process, horizontally scalable. Run two or more replicas behind a load balancer.
2. **PostgreSQL 18+** — the only durable state. Primary plus at least one synchronous streaming replica for HA.
3. **TLS-terminating reverse proxy** — Envoy, NGINX, Traefik, or a cloud load balancer. Terminates TLS before forwarding to the binary; set `server.use_https: false` and configure `server.trusted_proxies` (see [configure.md](configure.md)).
4. **Secrets store** — customer-managed. KMS, HSM, Vault, sealed file, or a Kubernetes Secret with envelope encryption.
5. **Observability sink** — a Prometheus scrape target plus a log forwarder to your SIEM.

```
                            ┌─────────────────┐
                            │  Reverse proxy  │
                            │   (TLS term.)   │
                            └────────┬────────┘
                                     │
               ┌─────────────────────┼─────────────────────┐
               ▼                     ▼                     ▼
      ┌─────────────┐        ┌─────────────┐        ┌─────────────┐
      │ systemprompt│        │ systemprompt│        │ systemprompt│
      │   replica 1 │        │   replica 2 │        │   replica N │
      └──────┬──────┘        └──────┬──────┘        └──────┬──────┘
             │ libpq/TLS            │                      │
             └──────────────────────┼──────────────────────┘
                                    ▼
                        ┌───────────────────────┐
                        │   Postgres primary    │─── sync replication ──► standby
                        │                       │─── WAL archiving   ──► object store
                        └───────────────────────┘
                                    │
                        ┌───────────────────────┐
                        │ Prometheus + SIEM     │
                        │ (scrape + syslog)     │
                        └───────────────────────┘
```

## 2. Configuration and secrets

Configuration is loaded from a profile directory. The `Config` shape is defined in `crates/shared/models/src/config/mod.rs`; the profile structs in `crates/shared/models/src/profile/`. Bootstrap order:

1. `ProfileBootstrap` — load and interpolate the YAML profile.
2. `SecretsBootstrap` — load secrets from the profile-referenced JSON file or from environment variables into process memory (`crates/infra/config/src/bootstrap/secrets/`).
3. `CredentialsBootstrap` — materialise provider credentials into in-memory handles.
4. `Config` — construct validated config.
5. `AppContext` — assemble the service graph (`crates/app/runtime/src/context.rs`).

The binary does not perform symmetric at-rest encryption of the secrets file. The customer owns the master-key lifecycle end-to-end; the binary receives plaintext only after the customer's tooling opens the envelope. The master key never enters the binary. Patterns, in order of preference for regulated production:

- **KMS / HSM envelope** (AWS KMS, GCP Cloud KMS, Azure Key Vault, on-prem HSM) — secrets file is ciphertext at rest; a short-lived decryption grant produces plaintext written to a tmpfs file the binary reads, or exported into its environment. Preferred for regulated workloads.
- **HashiCorp Vault** — Vault Agent sidecar renders secrets to a file or environment; lease renewal and revocation are Vault's responsibility.
- **sops + age / sops + KMS** — secrets file encrypted in place; the deploy pipeline decrypts at launch to tmpfs.
- **Kubernetes Secret with envelope encryption** — acceptable when the cluster has `--encryption-provider-config` with a KMS provider. Plain Kubernetes Secrets without a KMS envelope are not acceptable for PHI workloads.
- **Environment variable** (no envelope) — non-regulated deployments only; not for PHI.

Plain JSON secrets files carry `0600` permissions, owned by the dedicated service account. Never commit secrets to git. Secret types are in `crates/shared/models/src/secrets.rs`.

## 3. High availability

### 3.1 Application tier

The binary is stateless — all durable state lives in Postgres. Run N ≥ 2 replicas behind a load balancer. There is no session affinity requirement.

**Health endpoints.** The binary mounts these probes (`crates/entry/api/src/services/server/discovery.rs:166-177`):

| Endpoint | Auth | Cost | Returns |
|----------|------|------|---------|
| `GET /health` | none | `SELECT 1` round-trip | `200` healthy, `503` if the DB is unreachable |
| `GET /api/v1/health` | none | `SELECT 1` round-trip | same as `/health` |
| `GET /api/v1/health/detail` | authenticated | DB latency, service counts, memory, disk, table sizes | rich JSON |

There are **no** `/livez`, `/readyz`, `/healthz`, or `/health/live`/`/health/ready` aliases. Wire orchestrator probes against the real endpoints:

```yaml
# Kubernetes example — both probes target /health
livenessProbe:
  httpGet: { path: /health, port: 8080 }
  periodSeconds: 10
readinessProbe:
  httpGet: { path: /health, port: 8080 }
  periodSeconds: 5
```

`/health` performs a `SELECT 1` against the database on every call (`crates/entry/api/src/services/server/health.rs:179`), so it serves as a combined liveness-and-readiness signal: a `200` means the process is up and the DB is reachable. `/api/v1/health/detail` requires authentication and is not usable as an unauthenticated probe; use it for operator dashboards and deeper checks, not for the load balancer.

> **Operational caveat — no graceful shutdown on the main API server.** The main HTTP API server currently calls `axum::serve` without a graceful-shutdown future; on `SIGTERM` (container stop, rolling deploy, `systemctl restart`) the process is terminated mid-flight (`crates/entry/api/src/services/server/builder.rs`). In-flight requests are dropped and SSE connections are severed without notice. Until graceful shutdown is wired, mitigate at the orchestration layer:
>
> - Set a `preStop` hook (or equivalent drain delay) that removes the replica from the load balancer and waits for in-flight requests to complete before sending `SIGTERM`. A delay covering your longest expected non-stream request (plus margin) bounds request loss.
> - Keep the readiness probe pointed at `/health`; combined with a `preStop` drain, the LB stops sending new traffic before the process exits.
> - Avoid long-lived SSE assumptions across deploys: clients re-fetch canonical state on reconnect.
>
> The A2A agent server does wire graceful shutdown; this caveat is specific to the main API surface.

Rolling deploys are otherwise safe — draining a replica only requires completing in-flight requests, bounded by the request timeout.

### 3.2 Database tier

Target recovery objectives for a regulated deployment:

| Objective | Target |
|-----------|--------|
| RPO (data-loss window) | ≤ 60 seconds |
| RTO (recovery time) | ≤ 15 minutes |
| Availability | 99.95% (single-region, HA primary + standby) |
| Availability | 99.99% (multi-region with async replica) |

Recommended configuration:

- Primary plus one synchronous streaming replica (same or adjacent AZ, low-latency link).
- One asynchronous replica for DR (different region, higher RPO accepted).
- Continuous WAL archiving to an object store (S3, GCS, Azure Blob) via `pg_receivewal` or `wal-g`.
- `pg_basebackup` or `wal-g backup-push` nightly, retained per compliance programme.

Patroni, Stolon, or a cloud-managed Postgres (RDS Multi-AZ, Cloud SQL HA, Azure Flexible Server) are all acceptable for orchestrating failover.

### 3.3 Database role and grants

systemprompt connects with the role in `database_url`. Provision a least-privilege role for it. The platform issues only `INSERT` and `SELECT` against audit tables (`logs`, `analytics_events`, defined in `crates/infra/logging/schema/`) in normal operation — it never `UPDATE`s or `DELETE`s them.

Grants are an **operator-provisioned control**, not shipped DDL: no migration or schema file in the codebase emits `GRANT`/`REVOKE`. Provision them yourself, for example:

```sql
-- Run as a database superuser the application does not use day-to-day
REVOKE UPDATE, DELETE ON logs, analytics_events FROM systemprompt;
GRANT INSERT, SELECT ON logs, analytics_events TO systemprompt;
```

For defense-in-depth, install append-only triggers under a superuser role the application never uses, then take that role offline:

```sql
CREATE OR REPLACE FUNCTION audit_tables_deny_update_delete()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'Audit tables are append-only: % on % denied',
        TG_OP, TG_TABLE_NAME;
END;
$$;

CREATE TRIGGER logs_append_only
    BEFORE UPDATE OR DELETE ON logs
    FOR EACH ROW EXECUTE FUNCTION audit_tables_deny_update_delete();

CREATE TRIGGER analytics_events_append_only
    BEFORE UPDATE OR DELETE ON analytics_events
    FOR EACH ROW EXECUTE FUNCTION audit_tables_deny_update_delete();
```

These triggers do not interfere with normal operation because the platform never issues `UPDATE`/`DELETE` on these tables.

## 4. Backup and restore

### 4.1 Backup cadence

| Artifact | Cadence | Retention | Tooling |
|----------|---------|-----------|---------|
| Full base backup | Daily | 35 days rolling + monthly for 7 years (or per programme) | `wal-g backup-push` |
| WAL segments | Continuous | 35 days rolling | `wal-g wal-push` |
| Secrets file (encrypted) | On change | Versioned in your secrets system | KMS / Vault |
| Configuration profile | On change | Git-versioned | your IaC repo |

### 4.2 Restore procedure

Point-in-time recovery with `wal-g`:

```bash
# 1. Provision a fresh Postgres 18 instance
# 2. Fetch the most recent base backup
wal-g backup-fetch $PGDATA LATEST

# 3. Configure the recovery target
cat >> $PGDATA/postgresql.auto.conf <<EOF
restore_command = 'wal-g wal-fetch "%f" "%p"'
recovery_target_time = '2026-05-22 14:30:00 UTC'
EOF
touch $PGDATA/recovery.signal

# 4. Start Postgres; it replays WAL to the target and promotes
pg_ctl start

# 5. Repoint the systemprompt profile's database_url at the restored instance
# 6. Restart replicas; confirm GET /health returns 200
```

Test restore quarterly. A backup that has not been restored is a hope, not a recovery strategy.

## 5. Disaster recovery

Maintain a documented DR runbook covering at minimum:

1. **Trigger criteria** — what incidents escalate to DR (primary-region loss, corruption, compliance event).
2. **Communication tree** — who is notified and in what order.
3. **Cutover steps** — promote the DR replica, update DNS / service registry, re-key if compromise is suspected.
4. **Validation** — post-cutover checks: `GET /health` returns 200, an auth flow succeeds, an audit event round-trips, a sample governed request is evaluated.
5. **Rollback criteria** — when to abort DR cutover and attempt primary-region recovery instead.

Run a DR drill annually. Capture per-step timing and update the runbook.

## 6. Key rotation

| Key | Cadence | Procedure |
|-----|---------|-----------|
| Secrets-file envelope key (KMS / Vault / sops) | Per your key-management programme | Rotate in your KMS workflow; re-wrap the secrets file. The binary sees plaintext either way and does not participate in rotation. |
| JWT signing key (RS256) | Annual, or on suspected compromise | Mint a new RSA-2048 keypair with `systemprompt admin keys generate`. New tokens carry the new `kid` and verify against the public set republished at `/.well-known/jwks.json`. Tokens under the previous `kid` validate until natural expiry while the JWKS retains the prior public key — no maintenance window for the rotation itself. |
| OAuth at-rest pepper (`oauth_at_rest_pepper`) | Annual, or on suspected compromise | The pepper is the HMAC-SHA-256 key under which refresh-token ids and authorization codes are stored as digests. Rotating it invalidates every outstanding refresh token and pending authorization code; plan a maintenance window and force re-authentication, or stagger by issuing short-lived tokens before cutover. |
| Trusted-issuer JWKS (federated subjects) | Tracked by the issuer, not this deployment | RFC 8693 subject tokens verify against the JWKS at each `security.trusted_issuers[*].jwks_uri`. The client refreshes entries on a bounded LRU; no local key material is held. |
| MCP manifest-signing seed (`manifest_signing_secret_seed`) | On compromise, otherwise annual | Manifest signing is **Ed25519**. Rotate with `systemprompt admin bridge rotate-signing-key`, which writes a new base64 seed to the secrets file; re-sign and redeploy the manifest. Verification uses the corresponding Ed25519 public key, not the seed. This seed is distinct from the JWT signing key. |
| Database credentials | Per your policy | Create a new role, grant the minimum privileges (§3.3), update the secrets source, rolling-restart, then revoke the old role. |
| TLS certificates | Per your PKI policy | Reverse-proxy responsibility; no binary action required. |

## 7. Monitoring

### 7.1 Prometheus metrics

The binary serves `GET /metrics` (Prometheus exposition, `text/plain; version=0.0.4`). The recorder is installed and the route mounted **unconditionally** — `/metrics` is always exposed on the API port (`crates/entry/api/src/services/server/metrics.rs`, `discovery.rs:160-172`). It carries no scrape authentication and sits on the public discovery router. **Restrict it at the reverse-proxy or network layer** — allow only your scrape mesh to reach `/metrics`, or front it with a proxy that requires a scrape token. Do not expose it to untrusted networks; route labels and traffic volume are visible to anyone who can reach the port.

Recorded series (`metrics.rs:14-79`):

| Metric | Type | Labels | Use |
|--------|------|--------|-----|
| `http_requests_total` | counter | `method`, `path`, `status` | request and error rate |
| `http_request_duration_seconds` | histogram | `method`, `path`, `status` | p50/p95/p99 latency |
| `http_requests_in_flight` | gauge | — | concurrency / saturation |
| `sse_active_connections` | gauge | `channel` (`context`, `agui`, `a2a`, `analytics`) | live SSE stream counts |

Recommended alerts:

- `http_requests_total` 5xx rate climbing for 5 minutes — warn.
- p99 of `http_request_duration_seconds` over your SLO for 10 minutes — page.
- `http_requests_in_flight` near the replica's concurrency ceiling — warn (saturation).

### 7.2 SIEM integration

Structured logs and audit events are written to Postgres (the `logs` table) and emitted to stdout as JSON. Forward to a SIEM via:

1. **Logical replication** — preferred for high volume; the SIEM subscribes to the audit tables.
2. **Structured log egress** — forward stdout JSON via Fluent Bit, Vector, or similar to Splunk / Elastic / Datadog.
3. **Pull-based export** — periodic query by the SIEM against a read-only replica.

Log lines are structured JSON carrying a `trace_id` for correlation; secret-bearing fields (tokens, API keys, authorization headers) are redacted before emission. See [operate.md](operate.md) for the log fields and the OTLP ingest endpoint.

## 8. Air-gap deployment

No outbound network calls are required for governance operation. The binary does not phone home, check for updates, or emit telemetry. In an air-gapped environment:

1. **Binary distribution** — obtain and verify the release on a connected machine, then transfer it to the air-gapped network through your software-import process.
2. **Dependency mirror** — Postgres and OS packages mirrored locally; the binary has no runtime network dependency beyond Postgres.
3. **Provider endpoints** — when AI providers are reachable only via an approved egress proxy, set each provider's `base_url` in the profile to that proxy.
4. **Update channel** — scheduled import of verified releases through the same import process.

## 9. Update and rollback

### 9.1 Update

1. Obtain and verify the new release through your supply-chain process.
2. Review the [CHANGELOG.md](../../CHANGELOG.md) for the target version; scan for breaking-change entries.
3. Preview migrations: `systemprompt infra db migrate-plan` (no DB writes). Apply with `systemprompt infra db migrate`.
4. Rolling restart: replace one replica at a time; wait for `GET /health` to return 200 before proceeding. Use a `preStop` drain (§3.1) to bound request loss during each replacement.

### 9.2 Rollback

Migrations are additive-only within a minor version, so rolling back by one minor is supported:

1. Replace the binary with the previous version.
2. Rolling restart, draining each replica first.
3. Columns added by the newer version are ignored by the older binary.

Rolling back across more than one minor version requires a point-in-time restore from backup (§4.2).

## 10. Reference sizing

Starting points; benchmark against your own workload.

| Deployment | Replicas | vCPU / replica | Memory / replica | Postgres |
|------------|----------|----------------|------------------|----------|
| Pilot (≤100 AI users) | 2 | 1 | 512 MB | 2 vCPU / 4 GB / 50 GB SSD |
| Team (≤1,000 AI users) | 2–3 | 2 | 1 GB | 4 vCPU / 16 GB / 200 GB SSD |
| Enterprise (≤10,000 AI users) | 4–8 | 4 | 2 GB | 16 vCPU / 64 GB / 1 TB SSD + replica |

The request hot path is CPU-bound; Postgres becomes the capacity bottleneck before the binary does.

## Next steps

- [configure.md](configure.md) — write and manage the profile this deployment runs on.
- [operate.md](operate.md) — day-2 health checks, metrics scrape, logging, and troubleshooting.
