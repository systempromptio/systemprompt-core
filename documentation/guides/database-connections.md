# Budget database connections and run behind a pooler

How many Postgres connections a deployment opens, how to size the pool per node role, and how to put a transaction-mode pooler such as PgBouncer (or the CloudNativePG `Pooler`) in front of the main traffic without breaking the features that need a Postgres session of their own. For the deployment topology see [deploy-production.md](deploy-production.md); for every profile key see [reference/configuration.md](../reference/configuration.md).

## 1. What each node opens

Every node keeps up to `max_connections` connections per pool. Pools open lazily (`min_connections` is 0) and close idle connections after `idle_timeout_secs`.

| Pool | When it exists | Used for |
|------|----------------|----------|
| Read | Always (`database_url`) | Reads |
| Write | `database_write_url` is set | Writes, transactions, `NOTIFY`, the outbox |
| Session | `database_session_url` is set | `LISTEN`, session advisory locks, the migration runner |

Without `database_write_url` the write pool is the read pool. Without `database_session_url` the session features use the write pool, which is the behaviour of a deployment that connects straight to Postgres.

Each node also holds two long-lived sessions whatever its pool size: the replica-identity claim (a session advisory lock held for the process lifetime) and the cross-replica event listener (`LISTEN systemprompt_events`). Admin nodes additionally hold one session per running scheduler job for the job's advisory lock, and a migration run holds one for the migration lock.

## 2. Size the pool per role

`database.pool.max_connections` sets the ceiling for every node. `database.pool.max_connections_by_role` overrides it for the node's `server.role`, so one profile can keep a horizontally scaled gateway small and the single admin node larger:

```yaml
database:
  type: postgres
  pool:
    max_connections: 30
    max_connections_by_role:
      gateway: 15
      admin: 30
```

Each value must be between 1 and 500; the profile is rejected at bootstrap otherwise. A role that is not listed uses `max_connections`, and the engine default is 50.

The gateway commits its admission row before it calls the provider, so a streaming response does not hold a connection while it streams. A gateway node therefore needs far fewer connections than its concurrent request count; 10 to 20 is a reasonable starting point.

## 3. Budget the primary

Postgres refuses connections beyond its `max_connections` (100 by default, also in CloudNativePG). With an autoscaled gateway the ceiling is reached by the replica count at peak, not by any one pod:

```
max_connections(Postgres) >= gateway_max_replicas × (pools × gateway_pool + 2)
                           + admin_replicas      × (pools × admin_pool   + 2 + concurrent_jobs)
                           + migration job + operator sessions + headroom
```

`pools` is 1 with a single URL and 2 when `database_write_url` points somewhere other than `database_url` (each pool takes the full `max_connections`). Example: 12 gateway replicas at 15, one admin at 30, a single URL, a few concurrent jobs: `12 × 17 + 1 × 35 + 10 ≈ 250`.

## 4. Put a transaction-mode pooler in front

A transaction-mode pooler hands a server connection to a client for one transaction and then gives it to someone else. Ordinary statements, transactions, `SET LOCAL`, `NOTIFY` and transaction advisory locks all work through it. Three things do not, and they are exactly what the session pool is for:

- `LISTEN` — notifications arrive on a server session the listener no longer owns.
- Session advisory locks — the replica-identity claim and the scheduler job locks would be held by a backend another client then uses.
- The migration lock — the migration runner holds it for the whole run.

Point `database_url` (and `database_write_url`, if used) at the pooler and set the secret `database_session_url` (`DATABASE_SESSION_URL` with the `env` secrets source) to a direct connection to the primary. The session pool, the event listener, every advisory lock and the migration runner then use the direct connection, and everything else goes through the pooler. Budget the direct connections as `2 + concurrent_jobs` per admin node and `2` per gateway node, plus one for a migration run. The session pool is capped by `database.pool.session_max_connections` (default 10), independent of `max_connections`; raise it on an admin node that runs more concurrent cluster jobs than that.

Configure the pooler as follows:

- **Prepared statements.** systemprompt prepares named statements and caches up to `statement_cache_capacity` of them per connection. PgBouncer 1.21 or later with `max_prepared_statements` above 0 tracks them across server connections. Do not set `statement_cache_capacity: 0` to work around an older pooler: without a cache the statements are never closed and every backend grows until Postgres runs out of memory.
- **Startup parameters.** Each connection sends the startup parameter `options` (with `idle_in_transaction_session_timeout` and `client_min_messages`). PgBouncer refuses unknown startup parameters unless they are listed in `ignore_startup_parameters`; add `options` there, and set the equivalent server default with `ALTER ROLE <role> SET idle_in_transaction_session_timeout = '10min'` so the guard against stuck transactions still applies.
- **Pool mode.** `pool_mode = transaction`. Session mode also works without `database_session_url`, but it does not reduce the number of server connections.

## 5. Verify

- `GET /health` reports the event relay as listening on every replica; a relay stuck reconnecting means `LISTEN` is not reaching a session-capable connection.
- Two replicas never share an identity: a second process with the same `server.instance_id` must be refused at boot.
- `pg_stat_activity` on the primary shows the pooler's server connections plus the direct sessions from step 4, and the count does not grow with the number of gateway replicas beyond the per-replica direct sessions.
