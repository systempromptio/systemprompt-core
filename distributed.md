# Distributed deployments: what core must change

Written 2026-09-02 from a Docker reproduction of a customer topology against
the published astound image on core 0.43.0: three stateless nodes sharing one
PostgreSQL 16 primary behind a TLS load balancer, with a streaming replica. The
harness is `scripts/e2e-published-release.sh` in the astound repo. Every item
below was either observed to break, or is a single-process assumption that a
second node violates. Ordered by launch impact.

## Assumptions the code makes today

| assumption | where | what breaks with N > 1 nodes |
|---|---|---|
| One process owns the MCP service registry | `services` table, `find_service_by_name`; `entry/api/.../lifecycle/reconciliation.rs` | rows are keyed by server name; a node treats another node's live PID as stale and rewrites the row; two nodes booting together race and one fails boot with "MCP services running but not properly registered" |
| Scheduler outputs are cluster-global | `app/scheduler` advisory lock (`pg_try_advisory_lock(hashtext(job))`) | `publish_pipeline` writes node-local files (`web/dist`, bundled CSS, copied assets); the lock lets one node render, the others serve 404 for the whole public site |
| Profile authoring runs once, on the only node | `admin setup` generates `oauth_at_rest_pepper`, `manifest_signing_secret_seed`, RSA key | every node mints a different identity; JWTs, PATs and manifests from one node are rejected by the next |
| The signing key is a file beside the binary | `keys/authority.rs` file fallback, entrypoints that `keys generate` | regenerated per container start, invalidating all sessions; the `signing_key_pem` secret path exists but nothing documents or enforces it |
| `storage/data` is the instance's storage | uploads, generated images, `storage/data/**` | node-local volumes diverge: a file uploaded through node A is 404 on node B |
| Health means "the process is up" | `/api/v1/health` returns `200 {"status":"starting"}` during boot | any load balancer that checks status code only admits a node that is still migrating; there is no `/readyz` |
| Shutdown is instantaneous | `axum::serve` without a graceful-shutdown future | rolling restarts drop in-flight requests and sever SSE streams; readiness is never flipped off, so the balancer keeps sending traffic until the socket closes |
| Rate limits are per process | `rate_limits` buckets in memory | a quota of N/s becomes N×nodes/s; unidentified callers are bucketed per node |
| `instance_id` is optional and unused | `server.instance_id` in the profile | there is no per-replica identity to key registrations, locks, logs, or metrics on |

## Work items

### 1. Instance-scoped MCP service registry (launch blocker for concurrent boot)

- Add `instance_id` to the `services` rows and to every read/write in
  `domain/mcp` repositories. Derive it from `server.instance_id`, defaulting to a
  stable per-host value (hostname + boot id) when unset.
- `verify_database_registration` and stale-row reconciliation consider only rows
  with this instance's id. Rows from other instances are never judged by local
  PID liveness.
- Add a periodic heartbeat column so an instance that disappears without cleanup
  can be garbage-collected by a scheduler job instead of by the next booting node.
- Until this lands, operators must boot nodes strictly one at a time.

### 2. Scheduler: per-job lock scope

- `distributed_lock` is a single scheduler-wide flag. Add a per-`JobConfig`
  `scope: cluster | node` (default `cluster`), and run `node`-scoped jobs on every
  replica without the advisory lock. `publish_pipeline`, `bundle_admin_css`,
  `copy_extension_assets`, `content_prerender` are `node`; DB cleanups stay `cluster`.
- Bootstrap jobs must respect the same scope.
- Alternative that removes the problem: ship `web/dist` in the image (build-time
  prerender needs content from the DB, so this requires a content snapshot step)
  or serve the site from object storage. Not the short path.

### 3. Identity secrets are inputs, never generated at boot

- `admin setup` / entrypoint paths stop generating `oauth_at_rest_pepper`,
  `manifest_signing_secret_seed` and the signing key when a profile is supplied;
  boot fails loudly if any is missing. The env-mode loader already requires the
  pepper; extend the same requirement to the seed and `SIGNING_KEY_PEM`.
- Document the exact encodings in `reference/configuration.md`: seed is standard
  base64 of 32 bytes; `signing_key_pem` is standard base64 of a PKCS#8 PEM. Both
  were discovered by boot failures.
- Provide `systemprompt admin identity generate` that emits all three in the
  right encoding as a JSON fragment, so operators do not hand-roll them.

### 4. Shared storage abstraction

- Route `storage/data` writes (uploads, generated images, artifacts) through a
  storage backend trait with a filesystem default and an S3-compatible
  implementation. Multi-node deployments configure the object store; single node
  keeps the volume.
- Until then, document `storage/data` as single-node only.

### 5. Readiness and graceful shutdown

- Split liveness and readiness: `/livez` (process up) and `/readyz` (migrations
  applied, MCP children registered, warm). `/readyz` returns 503 during boot and
  after a drain signal.
- Wire `axum::serve(...).with_graceful_shutdown(...)` on SIGTERM: flip readiness
  off, stop accepting, drain in-flight requests and SSE streams with a bounded
  timeout, then exit. Document the balancer drain interval accordingly.

### 6. Rate limiting and quotas across nodes

- Move quota buckets that must be global (per-user AI quota, per-tenant) to the
  database or a shared counter; keep abuse throttles per node and document that
  they scale with node count.
- Unidentified callers behind a balancer whose CIDR is not in `trusted_proxies`
  are refused rather than admitted. Boot should fail, not warn, when the profile
  is `cloud`/multi-node and `trusted_proxies` is empty.

### 7. Read replicas

- `database_url` (reads) + `database_write_url` (primary) exists and the boot
  check refuses a standby as write target. What is missing for regional replicas
  is read-after-write consistency: session creation, PAT issue, bridge exchange
  codes and device links read back rows they just wrote. Either route those
  paths to the write pool explicitly (a `ReadAfterWrite` marker on the
  repository calls) or gate replica reads on a replica-lag bound. Until this is
  done, regional replica reads are not supportable; replicas are DR only.

### 8. Observability per replica

- Stamp `instance_id` on every log line, `ai_requests` row, trace span and
  Prometheus series so a multi-node incident can be attributed to a node.
- `/metrics` is unauthenticated on the public router; add a scrape token or bind
  it to a separate port so balancers do not have to filter it.

### 9. Documentation and tooling

- `documentation/guides/deploy-production.md` describes N replicas but none of
  the above caveats; add a "what is not yet replica-safe" section that this file
  retires item by item.
- Ship a `systemprompt admin doctor --distributed` that checks: identity secrets
  present and identical across nodes (by hash), `trusted_proxies` set, `instance_id`
  set, storage backend not local when N > 1, readiness endpoint answering.

## Verified working across nodes (no core change needed)

JWT/PAT verification with a shared `signing_key_pem`; sessions and PATs in the
database; migrations run once and are idempotent on later nodes; `admin bootstrap`
idempotent; MCP children per node on fixed ports; governance audit rows and costs
from every node; downloads served from each node's image copy; a node restart
keeps existing tokens valid when the key comes from the secret store.
