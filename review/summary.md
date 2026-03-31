# Review Summary & Remediation Roadmap

Review date: 2026-03-31
Codebase version: 0.1.18 (commit 38dcc3f50)
Internal software — no GDPR/consent requirements

---

## Deferred / Removed

| ID | Finding | Reason |
|----|---------|--------|
| ARCH-001 | Split agent domain into 3 sub-crates | Deferred - architectural refactor |
| ARCH-002 | Extract CLI command logic into service modules | Deferred - architectural refactor |
| GDPR-001 | Consent audit trail | N/A - internal software |
| GDPR-004 | Excessive data collection | N/A - internal software |

---

## Active Remediation: 4 Parallel Workstreams

### Bucket A: OAuth & WebAuthn Security (worktree agent)

**Files:** `crates/domain/oauth/src/services/validation/redirect_uri.rs`, `crates/entry/api/src/routes/oauth/endpoints/webauthn_complete.rs`, `crates/entry/api/src/routes/oauth/endpoints/authorize/validation.rs`, `crates/domain/oauth/src/services/webauthn/service/registration.rs`, `crates/domain/oauth/src/services/webauthn/service/authentication.rs`, `crates/domain/oauth/src/repository/oauth/auth_code.rs`

| Priority | ID | Action |
|----------|----|--------|
| P0 | CRITICAL-001 | Fix redirect URI validation - reject full URLs against relative registrations |
| P0 | CRITICAL-002 | Use WebAuthn-authenticated user_id, not query param |
| P1 | HIGH-005/008 | Remove CORS wildcard on WebAuthn complete |
| P1 | HIGH-009 | Check challenge timestamp on retrieval |
| P1 | HIGH-004 | Strengthen PKCE entropy validation |
| P1 | HIGH-007 | Block internal/private IPs in resource URI validation |
| P2 | MEDIUM-002 | Validate OAuth state user matches WebAuthn-authenticated user |
| P2 | MEDIUM-004 | Unify auth code error messages to prevent enumeration |

### Bucket B: Sync & Input Security (worktree agent)

**Files:** `crates/entry/api/src/routes/sync/files.rs`, `crates/entry/api/src/routes/sync/auth.rs`, `Cargo.toml`, `crates/entry/api/Cargo.toml`

| Priority | ID | Action |
|----------|----|--------|
| P0 | HIGH-001 | Block symlinks/hardlinks and validate canonical paths in tarball extraction |
| P0 | HIGH-003 | Add `subtle` crate; use constant-time comparison for sync token |

### Bucket C: Process Security (worktree agent)

**Files:** `crates/domain/agent/src/services/agent_orchestration/process.rs`

| Priority | ID | Action |
|----------|----|--------|
| P0 | HIGH-002 | Replace `.envs(std::env::vars())` with explicit env var allowlist |
| P1 | CONCURRENCY-002 | Add FD cleanup in pre_exec for spawned agents |

### Bucket D: Infrastructure & Concurrency (worktree agent)

**Files:** `crates/infra/config/src/services/manager.rs`, `crates/domain/ai/src/services/providers/gemini/provider.rs`

| Priority | ID | Action |
|----------|----|--------|
| P1 | CONCURRENCY-001 | Replace `std::sync::Mutex` with `tokio::sync::Mutex` in Gemini provider |
| P2 | MEDIUM-001 | Replace unsafe `set_var` with safe alternative |

### Future Work

| ID | Action |
|----|--------|
| TEST-001 | Add unit tests to agent domain |
| RESOURCE-001 | Agent process registry and coordinated shutdown |
| ERROR-001 | Unified ApiError type |
| GDPR-003 | Automated data retention/cleanup jobs |
| AUDIT-001 | Security event audit logging |
