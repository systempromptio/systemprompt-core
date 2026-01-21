# systemprompt-oauth Compliance

**Layer:** Domain
**Reviewed:** 2026-01-21
**Verdict:** COMPLIANT (pending upstream dependency fixes)

---

## Checklist

| Category | Status |
|----------|--------|
| Boundary Rules | ✅ |
| Required Structure | ✅ |
| Code Quality | ✅ |

---

## Verification Results

| Check | Status |
|-------|--------|
| Inline comments (`//`) | ✅ 0 instances |
| Doc comments (`///`) | ✅ 0 instances |
| Files over 300 lines | ✅ 0 files |
| `unwrap()` usage | ✅ None |
| `panic!()` / `todo!()` | ✅ None |
| `unsafe` blocks | ✅ None |
| `#[cfg(test)]` in source | ✅ None |
| Direct `env::var()` | ✅ None |
| `unwrap_or_default()` | ✅ None |
| `cargo fmt --check` | ✅ Pass |
| `cargo clippy` | ⚠️ Blocked by upstream deps |

---

## Commands Run

```
cargo fmt -p systemprompt-oauth -- --check    # PASS
rg '^\s*//[^!/]' --type rust src/             # 0 matches
rg '///' --type rust src/                     # 0 matches
rg '\.unwrap\(\)' --type rust src/            # 0 matches
rg 'panic!\(' --type rust src/                # 0 matches
rg 'unsafe' --type rust src/                  # 0 matches
rg '#\[cfg\(test\)\]' --type rust src/        # 0 matches
rg 'unwrap_or_default\(\)' --type rust src/   # 0 matches
rg 'env::var\(' --type rust src/              # 0 matches
```

---

## File Structure

All source files are under 300 lines. Key module sizes:

| Module | File | Lines |
|--------|------|-------|
| api/routes/oauth/token | handler.rs | 281 |
| api/routes/oauth | anonymous.rs | 271 |
| models/oauth | mod.rs | 290 |
| repository/oauth | mod.rs | 280 |
| api/routes/oauth/token | generation.rs | 271 |
| api/routes/oauth | webauthn_complete.rs | 236 |

---

## Changes Made (This Review)

### Compilation Fixes
1. Removed invalid `.await` on non-async functions in:
   - `authorize/validation.rs`
   - `consent.rs`
   - `introspect.rs`
   - `register.rs`

### Scope Function Refactoring
1. Made `validate_scopes()` a static function (no `&self`)
2. Made `get_default_roles()` return `Vec<String>` directly (no Result wrapper)
3. Made `get_available_scopes()` return `Vec<...>` directly (no Result wrapper)
4. Made `scope_exists()` return `bool` directly (no Result wrapper)
5. Updated all callers to use `OAuthRepository::function_name()` syntax

### Silent Error Pattern Fixes
1. Added logging before `.ok()` in filter_map patterns:
   - `token/generation.rs:130` - client scope parsing
   - `token/generation.rs:217` - client scope parsing
   - `callback.rs:203` - user role parsing
   - `authorization.rs:92` - JWT audience parsing

### Magic Number Extraction
1. Created `src/constants.rs` with named constants for:
   - PKCE: `CODE_CHALLENGE_MIN_LENGTH` (43), `CODE_CHALLENGE_MAX_LENGTH` (128)
   - Token: `COOKIE_MAX_AGE_SECONDS`, `SECONDS_PER_DAY`, `REFRESH_TOKEN_EXPIRY_DAYS`, `ANONYMOUS_TOKEN_EXPIRY_SECONDS`
   - WebAuthn: `CHALLENGE_EXPIRY_SECONDS`, `CLEANUP_INTERVAL_SECONDS`
   - Validation: `MIN_SEQUENTIAL_RUN`, `DIVERSITY_THRESHOLD`, `MIN_UNIQUE_CHARS`
2. Updated all usages in:
   - `authorize/validation.rs` - PKCE length checks, entropy validation
   - `anonymous.rs` - token expiry
   - `callback.rs` - cookie max-age, refresh token expiry
   - `webauthn/config.rs` - challenge expiry
   - `webauthn/manager.rs` - cleanup interval

### Dead Code Removal
1. Removed unused `validate_pkce()` function from `services/validation/oauth_params.rs`
2. Removed unused `PkceChallenge` struct and `PkceMethod` import

### Hardcoded URL Removal
1. Removed hardcoded localhost callback URLs from `client_config/update.rs`
   - Previously auto-added `http://localhost:6274/oauth/callback` URIs
   - Clients must now explicitly register their callback URIs

---

## Notes

The crate passes all internal compliance checks. However, `cargo clippy` cannot be run because upstream workspace dependencies have issues:

1. `systemprompt-cloud` - duplicate module file conflict
2. `systemprompt-security` - dead code warnings
3. `systemprompt-logging` - missing module

These issues are unrelated to the OAuth crate and need to be fixed separately.

Once upstream dependencies are fixed, run `cargo clippy -p systemprompt-oauth -- -D warnings` to complete verification.
