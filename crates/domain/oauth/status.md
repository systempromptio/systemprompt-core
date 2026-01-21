# systemprompt-oauth Compliance

**Layer:** Domain
**Reviewed:** 2026-01-21
**Verdict:** NON-COMPLIANT

---

## Checklist

| Category | Status |
|----------|--------|
| Boundary Rules | ✅ |
| Required Structure | ✅ |
| Code Quality | ❌ |

---

## Violations

### File Length Violations (Limit: 300 lines)

| File | Lines | Over By |
|------|-------|---------|
| `src/services/session/mod.rs` | 364 | 64 |
| `src/repository/client/mutations.rs` | 323 | 23 |
| `src/api/routes/webauthn/authenticate.rs` | 309 | 9 |

### Inline Comments (ZERO TOLERANCE - 44 instances)

| File:Line | Comment |
|-----------|---------|
| `src/lib.rs:1` | `// Minimal clippy allows...` |
| `src/services/validation/client_credentials.rs:24-25` | `// Always perform...` |
| `src/services/validation/client_credentials.rs:31` | `// Do a dummy verification...` |
| `src/services/validation/client_credentials.rs:36` | `// Do a dummy verification...` |
| `src/services/validation/client_credentials.rs:41` | `// Do a dummy verification...` |
| `src/services/validation/client_credentials.rs:60-61` | `// Note: Full tests...` |
| `src/services/validation/client_credentials.rs:65` | `// Ensure our dummy hash...` |
| `src/services/validation/client_credentials.rs:67` | `// This should not panic` |
| `src/services/validation/oauth_params.rs:24` | `// Enforce minimum length...` |
| `src/repository/client/queries.rs:98` | `// Use indexed join...` |
| `src/repository/client/queries.rs:127` | `// Find client by redirect_uri...` |
| `src/repository/client/queries.rs:130` | `// Then filter by required scopes` |
| `src/api/routes/oauth/anonymous.rs:111` | `// Check if this is a TUI session...` |
| `src/api/routes/oauth/anonymous.rs:122` | `// Generate admin JWT for TUI` |
| `src/api/routes/oauth/callback.rs:111` | `// Use indexed query...` |
| `src/api/routes/oauth/authorize/validation.rs:172-242` | Multiple entropy check comments |
| `src/api/routes/oauth/revoke.rs:126-143` | Multiple revocation comments |
| `src/api/routes/oauth/token/validation.rs:22` | `// Delegate to the shared...` |
| `src/api/routes/oauth/token/generation.rs:122,134,155` | Multiple scope validation comments |

### Doc Comments (ZERO TOLERANCE)

| File:Line | Issue |
|-----------|-------|
| `src/services/validation/client_credentials.rs:10-15` | `/// Validates client credentials...` |
| `src/api/routes/oauth/authorize/validation.rs:160-170` | `/// Validates that a PKCE code_challenge...` |
| `src/models/oauth/mod.rs:87` | `/// Error type for OAuth...` |

### Tests in Source Files

| File:Line | Issue |
|-----------|-------|
| `src/services/validation/client_credentials.rs:57` | `#[cfg(test)]` module in source |

### Anti-Patterns

| File:Line | Violation | Category |
|-----------|-----------|----------|
| `src/api/routes/webauthn/authenticate.rs:3` | `std::env::var("DANGEROUSLY_BYPASS_OAUTH")` | Direct env::var |
| `src/models/analytics.rs:55` | `unwrap_or_default()` | Silent Default |
| `src/models/oauth/dynamic_registration.rs:46` | `unwrap_or_default()` | Silent Default |
| `src/api/routes/oauth/consent.rs:28,29` | `unwrap_or_default()` | Silent Default |
| `src/api/routes/oauth/authorize/response_builder.rs:22,52-55` | `unwrap_or_default()` | Silent Default |
| `src/api/routes/oauth/webauthn_complete.rs:87,88` | `unwrap_or_default()` | Silent Default |

### Silent Error Handling

| File:Line | Pattern | Issue |
|-----------|---------|-------|
| `src/services/session/mod.rs:multiple` | `.ok()` | Swallowing errors |
| `src/services/validation/client_credentials.rs:31,36,41` | `let _ =` | Ignoring results |
| `src/api/routes/oauth/revoke.rs:142` | `let _ =` | Ignoring results |

### Formatting

| Command | Status |
|---------|--------|
| `cargo fmt -p systemprompt-oauth -- --check` | FAIL |

---

## Commands Run

```
cargo fmt -p systemprompt-oauth -- --check    # FAIL
cargo clippy -p systemprompt-oauth -- -D warnings  # BLOCKED (upstream dependency errors)
```

---

## Actions Required

### Critical (Must Fix Before Publish)

1. **Remove ALL 44 inline comments** - ZERO TOLERANCE per rust.md
2. **Remove ALL doc comments (`///`)** - ZERO TOLERANCE per rust.md
3. **Remove `#[cfg(test)]` module** from `client_credentials.rs` - move to `tests/`
4. **Replace `env::var()`** in `authenticate.rs` with `Config` access
5. **Run `cargo fmt`** to fix formatting issues

### High Priority

6. **Split `services/session/mod.rs`** (364 → ≤300 lines)
7. **Split `repository/client/mutations.rs`** (323 → ≤300 lines)
8. **Split `api/routes/webauthn/authenticate.rs`** (309 → ≤300 lines)

### Medium Priority

9. **Replace `unwrap_or_default()`** with explicit error handling (11 instances)
10. **Replace `let _ =`** with proper error handling or logging (5 instances)
11. **Review `.ok()` usage** for silent error swallowing (8 instances in session/mod.rs)

---

## Violation Count Summary

| Category | Count |
|----------|-------|
| Inline Comments | 44 |
| Doc Comments | 3 |
| File Length | 3 |
| Tests in Source | 1 |
| Direct env::var | 1 |
| unwrap_or_default | 11 |
| let _ = | 5 |
| .ok() swallowing | 8 |
| Formatting | FAIL |
| **Total Issues** | **76+** |
