# Deployment Reference Architecture

This document describes how to operate systemprompt.io in production to meet enterprise availability, durability, and recoverability expectations. It is a reference, not a prescription — customers adapt it to their platform (Kubernetes, VMs, bare metal) and compliance programme.

## 1. Minimum Production Topology

A production deployment consists of:

1. **systemprompt binary** — stateless Rust process, horizontally scalable. Target two or more replicas behind a load balancer for availability.
2. **PostgreSQL 15+** — stateful backing store. Primary + at least one synchronous streaming replica for HA.
3. **TLS-terminating reverse proxy** — customer choice (Envoy, NGINX, Traefik, cloud LB). Terminates TLS before forwarding to the binary.
4. **Secrets store** — customer-managed. KMS, HSM, sealed file, or Kubernetes Secret with envelope encryption.
5. **Observability sink** — Prometheus scrape target plus log forwarder to the customer's SIEM.

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

## 2. Configuration and Secrets

Configuration is loaded from a profile directory (see `crates/shared/models/src/config.rs` for the `Config` shape). Bootstrap order:

1. `ProfileBootstrap` — load YAML profile
2. `SecretsBootstrap` — load secrets from a profile-referenced JSON file or from environment variables into process memory (see `crates/shared/models/src/secrets_bootstrap.rs`)
3. `CredentialsBootstrap` — materialise provider credentials into in-memory handles
4. `Config` — construct validated config
5. `AppContext` — assemble service graph (see `crates/app/runtime/src/context.rs`)

The binary does **not** itself perform symmetric at-rest encryption of the secrets file. The deployment model is that the customer owns the master-key lifecycle end-to-end using their existing key-management programme, and the binary receives plaintext secrets only after the customer's tooling has opened the envelope. The master key never enters the binary. Supported patterns, in order of preference for regulated production:

- **KMS / HSM envelope** (AWS KMS, GCP Cloud KMS, Azure Key Vault, on-prem HSM) — the secrets file is ciphertext at rest; a short-lived decryption grant produces plaintext that is either written to a tmpfs-mounted file the binary reads, or exported into the binary's environment. Preferred for regulated workloads.
- **HashiCorp Vault** — Vault Agent sidecar renders secrets to a file or environment; lease renewal and revocation are Vault's responsibility.
- **sops + age / sops + KMS** — secrets file encrypted in place; CI / deploy pipeline decrypts at launch, materialising plaintext to tmpfs.
- **Kubernetes Secret with envelope encryption enabled** — acceptable when the cluster has `--encryption-provider-config` with a KMS provider configured. Plain Kubernetes Secrets without KMS envelope are not acceptable for PHI workloads.
- **Environment variable** (no envelope) — acceptable for non-regulated deployments only; not for PHI.

Plain JSON secrets files on disk should carry `0600` permissions and be owned by the dedicated service account. Never check secrets into git.

## 3. High Availability

### 3.1 Application Tier

The systemprompt binary is stateless — all durable state lives in Postgres. Run N ≥ 2 replicas behind a load balancer. Liveness and readiness probes:

| Probe | Endpoint | Purpose |
|-------|----------|---------|
| Liveness | `GET /health/live` | Process is responsive; container orchestrator restarts if failing |
| Readiness | `GET /health/ready` | DB reachable, config loaded; LB removes from pool if failing |

Rolling deploys are safe — draining a replica only requires completing in-flight requests (bounded by request timeout). There is no session affinity requirement.

### 3.2 Database Tier

Target recovery objectives for a regulated deployment:

| Objective | Target |
|-----------|--------|
| RPO (data loss window) | ≤ 60 seconds |
| RTO (recovery time) | ≤ 15 minutes |
| Availability | 99.95% (single-region, HA primary + standby) |
| Availability | 99.99% (multi-region with async replica) |

Recommended configuration:

- Primary + one synchronous streaming replica (same AZ or adjacent AZ, low-latency link)
- One asynchronous replica for DR (different region, accepting higher RPO)
- Continuous WAL archiving to an object store (S3, GCS, Azure Blob) via `pg_receivewal` or `wal-g`
- `pg_basebackup` or `wal-g backup-push` nightly, retained per compliance programme

Patroni, Stolon, or a cloud-managed Postgres (RDS Multi-AZ, Cloud SQL HA, Azure Flexible Server) are all acceptable for orchestrating failover.

## 4. Backup and Restore

### 4.1 Backup Cadence

| Artifact | Cadence | Retention | Tooling |
|----------|---------|-----------|---------|
| Full base backup | Daily | 35 days rolling + monthly for 7 years (or per programme) | `wal-g backup-push` |
| WAL segments | Continuous | 35 days rolling | `wal-g wal-push` |
| Secrets file (encrypted) | On change | Versioned in customer secrets-management system | KMS / Vault |
| Configuration profile | On change | Git-versioned | customer's IaC repo |

### 4.1.1 Optional Hardening: Schema-Level Append-Only on Audit Tables

By default, the systemprompt binary uses a DB role with `INSERT, SELECT` only on audit tables (`logs`, `analytics_events`) — no `UPDATE` or `DELETE`. This is the primary append-only control. Customers whose compliance programme requires defense-in-depth at the schema level can install the following trigger under a database superuser role that systemprompt does not use day-to-day:

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

Once installed, even a compromised systemprompt role (or any non-superuser) cannot mutate audit history. Rotate the superuser credentials afterwards and keep them offline. This trigger does not interfere with normal operation because systemprompt never issues UPDATE / DELETE on these tables.

### 4.2 Restore Procedure

Point-in-time recovery example using `wal-g`:

```bash
# 1. Provision a fresh Postgres 15 instance
# 2. Fetch the most recent base backup
wal-g backup-fetch $PGDATA LATEST

# 3. Configure recovery target
cat >> $PGDATA/postgresql.auto.conf <<EOF
restore_command = 'wal-g wal-fetch "%f" "%p"'
recovery_target_time = '2026-04-23 14:30:00 UTC'
EOF
touch $PGDATA/recovery.signal

# 4. Start Postgres; it will replay WAL to the target and promote
pg_ctl start

# 5. Reconfigure systemprompt profile to point at the restored instance
# 6. Restart systemprompt replicas; readiness probes confirm recovery
```

Test restore quarterly. A backup that has not been restored is a hope, not a recovery strategy.

## 5. Disaster Recovery

Maintain a documented DR runbook. At minimum:

1. **Trigger criteria** — what incidents escalate to DR (primary region loss, corruption detected, compliance event)
2. **Communication tree** — who is notified and in what order
3. **Cutover steps** — promote DR replica, update DNS / service registry, re-key if suspected compromise
4. **Validation** — post-cutover health checks (auth flows, audit event round-trip, sample governance request)
5. **Rollback criteria** — when to abort DR cutover and attempt primary-region recovery instead

Run a DR drill annually. Capture the timing of each step and update the runbook.

## 6. Key Rotation

| Key | Rotation cadence | Procedure |
|-----|------------------|-----------|
| Secrets-file envelope key (customer KMS / Vault / sops) | Per customer key-management programme | Rotate inside the customer's existing KMS rotation workflow. Re-wrap the secrets file with the new key; the binary sees plaintext in both cases and does not participate directly in rotation. |
| JWT signing secret (HS256) | Annual, or on suspicion of compromise | Generate a new HMAC secret; update the secrets source (KMS/Vault/sops); rolling restart of replicas. Outstanding JWTs signed with the old secret are invalidated on rotation — plan a short maintenance window or issue shorter-lived tokens in the week leading up to rotation. |
| OAuth signing keys (when systemprompt issues tokens to MCP servers via the code-flow authoriser) | 90 days recommended | Rotate through the `jwt_secret` rotation above; the OAuth issuer shares the same signing material in the current implementation. |
| Database credentials | Per customer policy | Create new role, grant minimum privileges, update secrets source, rolling restart, revoke old role. |
| TLS certificates | Per PKI policy | Reverse-proxy responsibility; no binary action required. |
| MCP manifest-signing secret | On compromise, otherwise annual | Re-sign the manifest with the new secret; deploy the secret atomically alongside the new manifest (verification uses the same HMAC). |

## 7. Monitoring

### 7.1 Prometheus Metrics

The binary exposes `/metrics` on a configurable port (default disabled in production — opt-in). Key series:

| Metric | Type | Use |
|--------|------|-----|
| `systemprompt_http_requests_total{route,status}` | counter | request rate, error rate |
| `systemprompt_http_request_duration_seconds{route}` | histogram | p50/p95/p99 latency |
| `systemprompt_governance_rule_evaluations_total{outcome}` | counter | allow/deny breakdown |
| `systemprompt_audit_events_written_total` | counter | audit throughput |
| `systemprompt_audit_events_failed_total` | counter | durability alert signal |
| `systemprompt_provider_requests_total{provider,outcome}` | counter | egress per provider |
| `systemprompt_provider_request_duration_seconds{provider}` | histogram | upstream provider health |
| `systemprompt_db_pool_available` | gauge | pool saturation |

Recommended alerts:

- Audit write failure rate > 0% for 5 minutes — page immediately (durability breach)
- p99 governance latency > 50ms for 10 minutes — page (SLA risk)
- Provider error rate > 10% for 5 minutes — warn (upstream incident)
- DB pool exhaustion — page (saturation)

### 7.2 SIEM Integration

Audit events are written to Postgres and can be forwarded to a SIEM via:

1. **Logical replication** — preferred for high-volume environments; SIEM subscribes to the `audit_events` table
2. **Structured log egress** — `systemprompt_tracing` emits audit events to stdout as JSON lines when configured; forward via Fluent Bit, Vector, or similar to Splunk / Elastic / Datadog
3. **Pull-based export** — periodic query by SIEM against a read-only replica

Event schema is documented in `crates/infra/events/src/` and stable per the [stability-contract.md](stability-contract.md).

Log format is structured JSON with these standard fields:

```json
{
  "ts": "2026-04-23T14:30:00.123Z",
  "level": "INFO",
  "event": "governance.request.evaluated",
  "request_id": "req_01HXYZ...",
  "user_id": "usr_...",
  "tenant_id": "tnt_...",
  "rule_decision": "allow",
  "provider": "anthropic",
  "duration_ms": 3,
  "trace_id": "...",
  "span_id": "..."
}
```

Fields tagged `secret` (tokens, API keys, prompt bodies by default) are redacted before emission.

## 8. Air-Gap Deployment

Zero outbound network calls are required for governance operation. In an air-gapped environment:

1. **Binary distribution** — download signed release on a connected machine, verify with cosign (see SECURITY.md), transfer to the air-gapped network
2. **Dependency mirror** — Postgres and OS packages mirrored locally; systemprompt itself has no runtime network dependencies beyond Postgres
3. **Provider endpoints** — when providers are reachable only via an approved egress proxy, configure per-provider `base_url` in the profile to point at that proxy
4. **Update channel** — scheduled import of verified releases through the customer's software-import process

Air-gap is a first-class deployment mode, not an afterthought. The binary does not phone home, check for updates, or emit telemetry.

## 9. Update and Rollback

### 9.1 Update

1. Pull new release; verify cosign signature and SBOM
2. Review the [CHANGELOG.md](../../CHANGELOG.md) for breaking changes — in particular, scan for any entry tagged `BREAKING` under the target version
3. Apply database migrations: `systemprompt infra db migrate --dry-run` then `--apply`. Migrations are forward-compatible for one minor version — see [stability-contract.md](stability-contract.md)
4. Rolling restart: replace one replica at a time; wait for readiness before proceeding

### 9.2 Rollback

Because migrations are forward-compatible across one minor version, rolling back by one minor is supported:

1. Replace binary with previous version
2. Rolling restart replicas
3. If data was written using schema additions from the newer version, those columns are ignored by the older binary (additive-only migration policy)

Rolling back across more than one minor version requires a point-in-time restore from backup.

## 10. Reference Sizing

Starting points; benchmark against your own workload:

| Deployment | Replicas | vCPU / replica | Memory / replica | Postgres |
|------------|----------|----------------|------------------|----------|
| Pilot (≤100 AI users) | 2 | 1 | 512 MB | 2 vCPU / 4 GB / 50 GB SSD |
| Team (≤1,000 AI users) | 2–3 | 2 | 1 GB | 4 vCPU / 16 GB / 200 GB SSD |
| Enterprise (≤10,000 AI users) | 4–8 | 4 | 2 GB | 16 vCPU / 64 GB / 1 TB SSD + replica |

Governance overhead is sub-5ms p50 at 200 concurrent requests per replica (measured). The hot path is CPU-bound; Postgres becomes the capacity bottleneck before the binary does.
