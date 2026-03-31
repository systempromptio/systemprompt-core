# Compliance, Operations & Data Governance Review

Review date: 2026-03-31
Codebase version: 0.1.18 (commit 38dcc3f50)
Reviewer: Automated deep audit

---

## GDPR Readiness: ~40%

The platform has core infrastructure for data handling but critical gaps in consent management, data retention, and right to erasure.

---

## GDPR-001: No Consent Audit Trail

**Severity:** CRITICAL
**Category:** GDPR Article 7 / Consent
**Impact:** Legal non-compliance

### Description

The platform collects extensive user data (PII, behavioral analytics, geolocation, device fingerprinting) but has no consent management infrastructure. No consent table exists in the database schema. No mechanism exists to record when consent was given, what it covered, or when it was withdrawn.

### Evidence

- No `consent` or `user_consent` table found in any schema file
- OAuth client consent is stored in `oauth_auth_codes` but no user-level consent audit trail exists
- Session creation captures 30+ data points with no consent documentation
- Analytics events record behavioral data without opt-in mechanism

### GDPR Requirements Not Met

- **Article 7(1):** Controller must be able to demonstrate that the data subject has consented
- **Article 7(3):** Data subject must be able to withdraw consent at any time
- **Article 6(1)(a):** Processing based on consent requires freely given, specific, informed, and unambiguous indication

### Remediation

1. Create a `user_consents` table:
   ```sql
   CREATE TABLE user_consents (
       id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
       user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
       consent_type VARCHAR(100) NOT NULL,  -- 'analytics', 'marketing', 'geolocation', etc.
       granted_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
       withdrawn_at TIMESTAMPTZ,
       ip_address INET,
       user_agent TEXT,
       consent_version VARCHAR(20) NOT NULL  -- Track which privacy policy version
   );
   ```
2. Gate analytics/geolocation collection on consent status
3. Provide API endpoints for consent withdrawal
4. Add consent checks to middleware before recording behavioral data

---

## GDPR-002: Right to Erasure Broken

**Severity:** CRITICAL
**Category:** GDPR Article 17 / Right to Erasure
**Impact:** Legal non-compliance

### Description

User deletion cascades to OAuth, WebAuthn, and agent data via foreign key constraints. However, it does NOT cascade to logs or analytics events. After a user is deleted, their PII (user_id, IP address, user agent, geolocation) persists indefinitely in the `logs` and `analytics_events` tables.

### Evidence

**Tables that CASCADE on user deletion (correct):**
- `webauthn_credentials` — `ON DELETE CASCADE`
- `webauthn_setup_tokens` — `ON DELETE CASCADE`
- `webauthn_challenges` — `ON DELETE CASCADE`
- `oauth_auth_codes` — `ON DELETE CASCADE`
- `oauth_refresh_tokens` — `ON DELETE CASCADE`
- `user_contexts` — `ON DELETE CASCADE`

**Tables that RETAIN data after user deletion (broken):**
- `logs` table — Contains `user_id`, `session_id`, `context_id`, `client_id`
  - File: `crates/infra/logging/schema/log.sql:8-12`
- `analytics_events` table — Contains `user_id`, `session_id`, full event metadata
  - File: `crates/infra/logging/schema/analytics.sql:1-35`
- `user_sessions` — Uses `ON DELETE SET NULL` for `user_id`, leaving orphaned session records with IP addresses, geolocation, and behavioral data
  - File: `crates/domain/users/schema/user_sessions.sql:49`

### Impact

When a user exercises their right to erasure, their PII remains in logging and analytics tables. This is a direct violation of GDPR Article 17. The orphaned session data (IP, geolocation, device fingerprint) can still be correlated to identify the deleted user.

### Remediation

1. Add `ON DELETE CASCADE` or `ON DELETE SET NULL` to log/analytics foreign keys
2. Implement an application-level erasure orchestrator that:
   - Deletes or anonymizes log entries containing the user_id
   - Deletes or anonymizes analytics events for the user
   - Nullifies IP addresses and user agents in orphaned session records
   - Generates an erasure receipt for compliance records
3. Ensure backup purge policies also handle deleted user data within 30 days

---

## GDPR-003: No Data Retention Policies

**Severity:** CRITICAL
**Category:** GDPR Article 5(1)(e) / Storage Limitation
**Impact:** Legal non-compliance, operational risk

### Description

The `logs` and `analytics_events` tables have no TTL, expiration, or automatic purge mechanism. They grow indefinitely. Cleanup operations exist in the codebase but require manual invocation — no scheduled automation.

### Evidence

**Cleanup operations that exist but are not automated:**

```rust
// crates/infra/database/src/repository/cleanup.rs:55-69
pub async fn delete_old_logs(&self, days: i32) -> Result<u64> {
    // Manual invocation only - no automatic scheduling
}
pub async fn delete_expired_oauth_tokens(&self) -> Result<u64> {
    // No automatic invocation
}
pub async fn delete_expired_oauth_codes(&self) -> Result<u64> {
    // No automatic invocation
}
```

**Tables with no retention policy:**
- `logs` — No TTL, no partition pruning (File: `crates/infra/logging/schema/log.sql`)
- `analytics_events` — No TTL, no partition pruning (File: `crates/infra/logging/schema/analytics.sql`)

**Tables with retention (correct):**
- `user_sessions` — 7-day expiration via `expires_at` default (File: `crates/domain/users/schema/user_sessions.sql:10`)

### Impact

- Unbounded table growth degrades database performance over time
- PII retained beyond legitimate business need violates GDPR storage limitation principle
- Backup sizes grow continuously, increasing storage costs and recovery times

### Remediation

1. Add scheduled jobs (via `systemprompt-scheduler`) to run cleanup operations daily:
   - `delete_old_logs(90)` — 90-day retention for operational logs
   - `delete_expired_oauth_tokens()` — daily cleanup
   - `delete_expired_oauth_codes()` — daily cleanup
   - Anonymize analytics events older than 365 days (retain aggregate data, remove PII)
2. Consider PostgreSQL table partitioning by month for logs and analytics, enabling fast partition drops
3. Document retention periods in a data processing agreement

---

## GDPR-004: Excessive Data Collection

**Severity:** HIGH
**Category:** GDPR Article 5(1)(c) / Data Minimisation
**Impact:** Legal risk, increased breach impact

### Description

The `user_sessions` table captures 30+ columns of tracking data, much of which is not necessary for the platform's core functionality.

### Evidence

File: `crates/domain/users/schema/user_sessions.sql:22-47`

**Data collected that exceeds functional necessity:**

| Column | Type | Necessity |
|--------|------|-----------|
| `country`, `region`, `city` | Geolocation | Not required for auth or API delivery |
| `fingerprint_hash` | Device fingerprinting | Not required; privacy-invasive |
| `utm_source`, `utm_medium`, `utm_campaign` | Marketing attribution | Not required for platform function |
| `referrer_source`, `referrer_url` | Referrer tracking | Not required for platform function |
| `landing_page`, `entry_url` | Page tracking | Not required for API platform |
| `behavioral_bot_score`, `behavioral_bot_reason` | Bot detection detail | Aggregate score sufficient |
| `total_tokens_used`, `total_ai_cost_microdollars` | Per-session AI costs | Aggregate metrics sufficient |
| `throttle_level`, `throttle_escalated_at` | Rate limit state | Could be ephemeral, not persisted |

### Impact

- Each field increases the PII surface area in case of a data breach
- Users cannot meaningfully consent to 30+ distinct data points
- Regulatory scrutiny increases with data volume
- Storage and processing costs scale with unnecessary data

### Remediation

1. Remove marketing attribution columns (UTM, referrer) unless the user explicitly consents to marketing analytics
2. Make geolocation opt-in (behind GDPR-001 consent mechanism)
3. Remove `fingerprint_hash` entirely — device fingerprinting is a GDPR red flag
4. Move per-session cost tracking to an aggregate analytics table (no PII association)
5. Make `behavioral_bot_score` ephemeral (in-memory only, not persisted)

---

## TENANT-001: No Database-Level Multi-Tenant Isolation

**Severity:** CRITICAL
**Category:** Multi-Tenancy / Data Isolation
**Impact:** Data breach risk

### Description

Multi-tenant data isolation relies entirely on application-layer `WHERE user_id = $1` filtering. No PostgreSQL Row-Level Security (RLS) policies exist. No schema-per-tenant isolation. No `tenant_id` column for organizational grouping. All users share the same tables with no database-enforced access boundaries.

### Evidence

- No `CREATE POLICY` statements found in any schema file
- No `ALTER TABLE ... ENABLE ROW LEVEL SECURITY` found
- No `tenant_id` column exists on any table
- All queries use `WHERE user_id = $1` for scoping — correct but fragile

Example of application-layer scoping (correct but without database enforcement):
```sql
-- crates/domain/agent/schema/user_contexts.sql
FROM user_contexts WHERE context_id = $1 AND user_id = $2
```

### Impact

A single SQL injection vulnerability, middleware bypass, or developer error in a WHERE clause exposes ALL user data across ALL tenants. There is no defense-in-depth at the database layer. The entire security model depends on every query in every code path correctly including the user_id filter.

### Remediation

**Phase 1 (Immediate):** Add PostgreSQL RLS policies to all user-facing tables:

```sql
ALTER TABLE user_contexts ENABLE ROW LEVEL SECURITY;

CREATE POLICY user_contexts_isolation ON user_contexts
    USING (user_id = current_setting('app.current_user_id')::uuid);
```

Set the session variable at connection checkout:
```rust
sqlx::query("SET app.current_user_id = $1")
    .bind(user_id)
    .execute(&pool)
    .await?;
```

**Phase 2 (Medium-term):** Add a `tenant_id` column for organizational isolation (multi-user teams). Apply RLS at the tenant level for shared resources.

---

## TENANT-002: No Process Isolation for Agents

**Severity:** CRITICAL
**Category:** Multi-Tenancy / Process Isolation
**Impact:** Cross-tenant data access

### Description

Agent processes spawn as regular OS processes with no containerization, namespace isolation, seccomp filtering, or resource limits. All agents share the parent's database connection pool, environment variables (including all API keys), and filesystem.

### Evidence

File: `crates/domain/agent/src/services/agent_orchestration/process.rs:86-148`

- Agents inherit full parent environment via `.envs(std::env::vars())`
- No cgroup resource limits applied
- No filesystem namespace isolation
- No network namespace isolation
- Shared database connection credentials
- Shared log directory

### Impact

A compromised or malicious agent can:
- Read all environment variables including API keys for every provider
- Access the database with full credentials (not scoped to its tenant)
- Read/write files belonging to other agents
- Interfere with other agent processes via shared resources
- Exfiltrate data from the host filesystem

### Remediation

**Short-term:** Whitelist environment variables per agent (see security review HIGH-002). Apply resource limits via `setrlimit`.

**Medium-term:** Run agents in isolated containers or namespaces with:
- Dedicated database credentials scoped to their tenant
- Filesystem mounts limited to their working directory
- Network policies restricting internal access
- Seccomp profiles blocking unnecessary syscalls

---

## TENANT-003: Application-Layer Query Scoping Has No Compile-Time Verification

**Severity:** HIGH
**Category:** Multi-Tenancy / Code Quality
**Impact:** Silent data leakage on developer error

### Description

Every database query must manually include `user_id` in its WHERE clause. There is no compile-time or runtime verification that all queries are properly scoped. A single missed filter in any new query exposes all users' data.

### Evidence

The pattern relies on correct parameter passing in every handler:
```rust
// crates/entry/api/src/routes/analytics/events.rs:78-80
let created = state.events.create_event(
    req_ctx.session_id().as_str(),
    req_ctx.user_id().as_str(),
    &input,
).await
```

If a developer writes a new query that omits `user_id`, no compiler warning, no runtime check, and no test catches it.

### Remediation

1. Create a `TenantScopedQuery` wrapper type that enforces `user_id` inclusion at the type level
2. Add integration tests that verify every public repository method requires a `user_id` parameter
3. Consider a custom clippy lint that flags queries without tenant scoping

---

## AUDIT-001: Security-Relevant Events Not Logged

**Severity:** HIGH
**Category:** Audit Trail / SOC2
**Impact:** Incident response capability gap

### Description

The logging infrastructure is well-designed (structured tracing, correlation IDs, indexed database storage), but security-relevant events are not explicitly captured. The system logs HTTP requests/responses but not security-specific events.

### Missing Audit Events

| Event | Status | Impact |
|-------|--------|--------|
| Successful login | Not logged | Cannot detect account compromise |
| Failed login attempt | Not logged | Cannot detect brute force attacks |
| OAuth grant authorization | Not logged | Cannot audit consent |
| OAuth token revocation | Not logged | Cannot verify revocation compliance |
| WebAuthn credential registration | Not logged | Cannot detect unauthorized credential addition |
| WebAuthn credential deletion | Not logged | Cannot detect credential tampering |
| Permission/role changes | Not logged | Cannot detect privilege escalation |
| API key creation/revocation | Not logged | Cannot audit API access grants |
| Admin actions | Not logged | Cannot verify admin accountability |
| User data export/deletion | Not logged | Cannot prove GDPR compliance |

### Evidence

File: `crates/entry/api/src/services/middleware/analytics/events.rs:52-70`
- Logs HTTP method, URI, status code, response time, user_id, session_id
- Does not log security event type, authentication method, or authorization decisions

### Remediation

Create a dedicated security audit logger:

```rust
pub enum SecurityEvent {
    LoginSuccess { user_id: UserId, method: AuthMethod, ip: IpAddr },
    LoginFailure { email: String, method: AuthMethod, ip: IpAddr, reason: String },
    TokenIssued { user_id: UserId, client_id: ClientId, scopes: Vec<String> },
    TokenRevoked { user_id: UserId, client_id: ClientId },
    CredentialRegistered { user_id: UserId, credential_type: String },
    PermissionChanged { user_id: UserId, changed_by: UserId, old_role: String, new_role: String },
    DataExported { user_id: UserId, requested_by: UserId },
    DataDeleted { user_id: UserId, requested_by: UserId },
}
```

Store in a dedicated `security_audit_log` table with immutable semantics (no UPDATE/DELETE allowed).

---

## OPS-001: No Automated Backup Mechanism

**Severity:** CRITICAL
**Category:** Operations / Disaster Recovery
**Impact:** Data loss risk

### Description

No automated database backup mechanism is visible in the codebase. No backup scheduling, no backup verification, no recovery procedure documentation, no encryption-at-rest for backups.

### Evidence

- No backup-related schema, configuration, or job definitions found
- No `pg_dump` or backup utility invocations in CLI commands
- No backup verification tests
- No documented Recovery Time Objective (RTO) or Recovery Point Objective (RPO)

### Remediation

1. Implement automated daily backups using `pg_dump` or cloud-native backup (RDS snapshots, Cloud SQL backups)
2. Encrypt backups at rest using AES-256
3. Store backups in a separate region/account from production
4. Implement weekly backup restoration tests
5. Document RTO/RPO targets and verify they are achievable
6. Add a CLI command: `systemprompt infra db backup` and `systemprompt infra db restore`

---

## OPS-002: One-Way Database Migrations

**Severity:** HIGH
**Category:** Operations / Database
**File:** `crates/infra/database/src/lifecycle/migrations.rs`

### Description

Database migrations are applied forward only. No down migrations are defined. No rollback mechanism exists. If a migration causes data corruption or performance degradation, the only recovery path is restoring from backup.

### Evidence

```rust
// crates/infra/database/src/lifecycle/migrations.rs:45-76
// Migrations are tracked with version, name, and checksum
// Checksum validation warns if SQL has changed but doesn't prevent execution
// No rollback logic exists
```

### Impact

- A bad migration in production requires backup restoration (potentially hours of downtime)
- No way to test rollback procedures
- Encourages "fix-forward" culture which increases pressure during incidents

### Remediation

1. Add down migration support (optional per migration, required for destructive changes)
2. Wrap migrations in transactions where possible (DDL is transactional in PostgreSQL)
3. Add pre-migration backup step
4. Test migrations against a production-like dataset before applying

---

## OPS-003: Orphaned Agent Processes Survive Server Shutdown

**Severity:** HIGH
**Category:** Operations / Process Management
**File:** `crates/domain/agent/src/services/agent_orchestration/process.rs:148`

### Description

Agent processes are spawned with `std::mem::forget(child)`, which detaches them from the parent. On server shutdown (graceful or crash), agents continue running as orphans consuming resources, holding database connections, and potentially serving stale state.

### Impact

- Resource leak across server restarts
- Port conflicts when restarting (agents still bound to ports)
- Database connection exhaustion from orphaned processes
- Stale agents may serve outdated responses to users

### Remediation

1. Maintain a PID registry of spawned agents
2. On graceful shutdown, send SIGTERM to all agents, wait 10s, then SIGKILL
3. On startup, check for stale agent processes from previous runs and terminate them
4. Store PID registry in a file or database table for crash recovery

---

## OPS-004: No Metrics or Alerting Infrastructure

**Severity:** MEDIUM
**Category:** Operations / Monitoring
**Impact:** Incident detection delay

### Description

The platform has structured logging with trace correlation but no metrics emission (Prometheus, StatsD, etc.) and no alerting configuration. Incident detection requires manual log analysis.

### Missing Metrics

- Request rate, latency percentiles, error rate (RED metrics)
- Database connection pool utilization
- Agent process count and health status
- Memory and CPU usage trends
- OAuth flow success/failure rates
- WebAuthn authentication success/failure rates
- Event broadcast queue depth

### Remediation

1. Add `metrics` crate with Prometheus exporter
2. Instrument critical paths: request handler, database queries, agent lifecycle
3. Expose `/metrics` endpoint for scraping
4. Define alert rules for: error rate > 5%, p99 latency > 5s, DB pool > 80%, memory > 80%

---

## OPS-005: HTTPS Not Enforced at Application Level

**Severity:** MEDIUM
**Category:** Operations / Security
**File:** `crates/infra/config/src/services/validator.rs:20-24`

### Description

The configuration validator warns if `USE_HTTPS` is not enabled in production, but does not enforce it. No HTTP-to-HTTPS redirect middleware exists. The platform relies entirely on the reverse proxy or deployment configuration for TLS termination.

### Impact

If deployed without a reverse proxy (development, staging, misconfigured production), all traffic including OAuth tokens, JWT secrets, and user credentials transmits in plaintext.

### Remediation

1. Change the HTTPS warning to a hard error in production mode
2. Add HTTP-to-HTTPS redirect middleware as a default
3. Validate HSTS max-age is set to at least 31536000 (1 year)

---

## LICENSE-001: Business Source License Compliance

**Severity:** LOW
**Category:** License Compliance
**File:** `Cargo.toml:52`

### Description

The codebase uses BUSL-1.1 (Business Source License 1.1). No GPL or AGPL contamination was detected in the dependency tree. All transitive dependencies appear to use permissive licenses (MIT, Apache-2.0, BSD).

### Evidence

- License field: `license = "BUSL-1.1"`
- Dependencies audited via `Cargo.toml` workspace dependencies — all MIT/Apache-2.0/BSD
- `libc` crate: MIT/Apache-2.0
- `tokio` crate: MIT
- `axum` crate: MIT
- `sqlx` crate: MIT/Apache-2.0

### Gaps

- No SBOM (Software Bill of Materials) generated
- No third-party license attribution file (e.g., `THIRD_PARTY_LICENSES.md`)
- `cargo deny` configuration may be overly restrictive (rejecting valid licenses like 0BSD, Unlicense)

### Remediation

1. Generate SBOM: `cargo tree --depth 1 --format "{p} {l}"` > `THIRD_PARTY_LICENSES.md`
2. Run `cargo deny check licenses` and fix any false-positive rejections
3. Add license check to CI pipeline

---

## SOC2 Readiness: ~50%

### Controls Met
- Access control via JWT + WebAuthn (Type I)
- Structured audit logging infrastructure (Type I)
- Rate limiting and bot detection (Type I)
- Encrypted transport (TLS via rustls) (Type I)
- Security headers (HSTS, CSP, X-Frame-Options) (Type I)

### Controls Not Met
- No automated backup/recovery (CC6.1)
- Insufficient security event logging (CC7.2)
- No incident response procedure (CC7.3)
- No change management audit trail (CC8.1)
- No vulnerability scanning in CI (CC7.1)
- No penetration testing schedule (CC7.1)
- No data retention enforcement (CC6.5)
- No consent management (CC2.1 if handling EU data)
