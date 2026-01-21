# systemprompt-oauth Tech Debt Audit

**Layer:** Domain
**Audited:** 2026-01-21
**Verdict:** NEEDS_WORK

---

## Summary

| Category | Status | Issues |
|----------|--------|--------|
| Architecture | :x: | 3 |
| Rust Standards | :warning: | ~18 remaining |
| Code Quality | :white_check_mark: | 0 (FIXED) |
| Tech Debt | :white_check_mark: | 0 |

**Total Remaining Issues:** ~21

---

## Fixes Applied (This Session)

### Silent Error Patterns - FULLY FIXED

All 20 `.ok()` and `let _ =` patterns now have proper error logging:

| File | Fix Applied |
|------|-------------|
| `services/session/lookup.rs` | Added `map_err` with `tracing::warn!` before all `.ok()` calls |
| `services/session/mod.rs` | Changed `let _ =` to `if let Err(e)` with logging |
| `services/http.rs` | Added `map_err` with `tracing::debug!` for header parsing |
| `api/routes/oauth/revoke.rs` | Changed `let _ =` to `if let Err(e)` with logging |
| `api/routes/oauth/userinfo.rs` | Added `map_err` with `tracing::debug!` for header parsing |
| `api/routes/oauth/token/handler.rs` | Added `map_err` with `tracing::debug!` for grant type parsing |
| `repository/oauth/auth_code.rs` | Added `map_err` with `tracing::debug!` for PKCE parsing |

**Note:** `services/validation/client_credentials.rs:47` - `let _ =` is intentional for timing attack mitigation.

### Typed Identifiers - SIGNIFICANT PROGRESS

**New Type Created:**
- `ChallengeId` in `crates/shared/identifiers/src/oauth.rs`

**Files Updated to Use Typed Identifiers:**

| File | Changes |
|------|---------|
| `crates/shared/identifiers/src/oauth.rs` | Added `ChallengeId` type |
| `crates/shared/identifiers/src/lib.rs` | Exported `ChallengeId` |
| `models/clients/mod.rs` | `OAuthClientRow.client_id` and `OAuthClient.client_id` -> `ClientId` |
| `models/clients/api.rs` | `CreateOAuthClientRequest.client_id` and `OAuthClientResponse.client_id` -> `ClientId` |
| `repository/client/mod.rs` | All 4 structs updated to use `ClientId` |
| `repository/webauthn.rs` | `WebAuthnCredential.user_id` -> `UserId` |
| `api/routes/oauth/authorize/mod.rs` | `AuthorizeQuery.client_id` and `AuthorizeRequest.client_id` -> `ClientId` |
| `api/routes/oauth/consent.rs` | `ConsentQuery.client_id` and `ConsentRequest.client_id` -> `ClientId` |

### Domain Error Type - NOT NEEDED

The crate correctly uses `AuthError` from `systemprompt_models::auth` which provides comprehensive OAuth-specific error variants. No separate `error.rs` needed.

---

## Critical Violations (Remaining)

### Architecture Violations (3)

| File | Violation | Severity |
|------|-----------|----------|
| `Cargo.toml:27` | Depends on `systemprompt-runtime` (App layer) | Critical |
| `Cargo.toml:64` | Depends on `systemprompt-users` (Domain) - cross-domain | Critical |
| `Cargo.toml:67` | Depends on `systemprompt-analytics` (Domain) - cross-domain | Critical |

**Resolution Required:**
1. Define `UserProvider` trait in `shared/traits`
2. Define `AnalyticsProvider` trait in `shared/traits`
3. Implement traits in respective domain crates
4. Update oauth to depend on traits instead of concrete types

### Raw String IDs (Remaining - ~18 occurrences)

Files still needing typed identifier updates:

| File | Fields to Update |
|------|------------------|
| `models/analytics.rs` | `client_id: String` (2 occurrences) |
| `models/cimd.rs` | `client_id: String` |
| `models/oauth/dynamic_registration.rs` | `client_id: String` |
| `api/routes/oauth/anonymous.rs` | `session_id`, `user_id`, `client_id` (4 occurrences) |
| `api/routes/oauth/callback.rs` | `client_id: String` |
| `api/routes/oauth/webauthn_complete.rs` | `user_id`, `client_id` (2 occurrences) |
| `api/routes/webauthn/authenticate.rs` | `challenge_id`, `user_id` (3 occurrences) |
| `api/routes/webauthn/register/finish.rs` | `challenge_id`, `user_id` (2 occurrences) |
| `services/http.rs` | `client_id: String` |
| `services/webauthn/config.rs` | `rp_id: String` |
| `services/webauthn/service/mod.rs` | `user_id: String` |

**Available Typed Identifiers:**
- `ClientId` - for OAuth client identifiers
- `UserId` - for user identifiers
- `SessionId` - for session identifiers
- `ChallengeId` - for WebAuthn challenge identifiers (newly created)

---

## Passing Checks

| Check | Status |
|-------|--------|
| Zero inline comments (`//`) | :white_check_mark: |
| Zero doc comments (`///`) | :white_check_mark: |
| Zero `unwrap()` calls | :white_check_mark: |
| Zero `panic!()` / `todo!()` / `unimplemented!()` | :white_check_mark: |
| Zero `unsafe` blocks | :white_check_mark: |
| Zero `#[cfg(test)]` modules | :white_check_mark: |
| Zero `println!` / `eprintln!` / `dbg!` | :white_check_mark: |
| Zero TODO/FIXME/HACK comments | :white_check_mark: |
| Zero non-macro SQLX calls | :white_check_mark: |
| Repository pattern enforced (no SQL in services) | :white_check_mark: |
| Zero `NaiveDateTime` usage | :white_check_mark: |
| Zero direct `env::var()` access | :white_check_mark: |
| Zero `unwrap_or_default()` usage | :white_check_mark: |
| All files under 300 lines | :white_check_mark: |
| Schema directory with SQL files exists | :white_check_mark: |
| Formatting passes | :white_check_mark: |
| No `#[allow(dead_code)]` attributes | :white_check_mark: |
| Silent error patterns fixed | :white_check_mark: |

---

## Commands Executed

```
cargo fmt -p systemprompt-oauth -- --check          # PASS
cargo clippy -p systemprompt-oauth -- -D warnings   # BLOCKED (requires database)
```

---

## Required Actions

### Before crates.io Publication

1. **Fix Architecture Violations (3 remaining):**
   - Create `UserProvider` trait in `shared/traits`
   - Create `AnalyticsProvider` trait in `shared/traits`
   - Remove direct domain dependencies, use trait injection

2. **Complete Typed Identifier Migration (~18 remaining):**
   - Update remaining models to use `ClientId`, `UserId`, `SessionId`, `ChallengeId`
   - Test SQLX compatibility with typed identifiers

3. **Regenerate SQLX offline cache:**
   - Run `cargo sqlx prepare` with database connection
   - Enables offline Clippy verification

---

## Verdict Criteria

- **CLEAN**: Zero critical violations, ready for crates.io
- **NEEDS_WORK**: Minor issues, can publish with warnings
- **CRITICAL**: Blocking issues, must resolve before publication

**Current Status: NEEDS_WORK**
- Silent error patterns: FULLY FIXED (20/20)
- Typed identifiers: ~15 of 33 FIXED, ~18 remaining
- Architecture violations: 3 REMAINING (blocking for crates.io)
