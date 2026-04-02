# Domain: OAuth Crate Coverage

## Current State

**Source code:** 60 source files across 9 service modules in `crates/domain/oauth/src/`
**Test code:** 27 test files in `crates/tests/unit/domain/oauth/src/` (563 tests)
**Integration tests:** 14 tests in `crates/tests/integration/oauth/` (client lifecycle, tokens, webauthn)
**Coverage:** 42.8% line coverage (up from 34.1% baseline)

### Phase 3 Completion (2026-04-02)

Added 139 new tests across 8 new test files covering previously untested OAuth services:

| New Test File | Tests | What's Covered |
|--------------|-------|----------------|
| `services/auth_provider.rs` | 28 | JwtAuthProvider, JwtAuthorizationProvider, TraitBasedAuthService, JwtValidationProviderImpl — token validation, audience matching, permission parsing, token generation |
| `services/session.rs` | 21 | SessionCreationService construction, AnonymousSessionInfo, AuthenticatedSessionInfo, SessionCreationError, client secret hash/verify roundtrip |
| `services/cimd.rs` | 33 | CimdFetcher URL validation, ClientId type dispatch, ClientValidation accessors, CimdMetadata edge cases |
| `services/webauthn/config.rs` | 10 | WebAuthnConfig construction, all builder methods, chrono duration conversion |
| `services/webauthn/jwt.rs` | 10 | JwtTokenValidator — valid/expired/invalid tokens, UUID extraction, username/email extraction |
| `services/webauthn/token.rs` | 8 | generate_setup_token, hash_token determinism, validate_token_format |
| `services/webauthn/user_service.rs` | 15 | UserCreationService — find/create user, email/username uniqueness, role assignment |
| `services/webauthn/service_types.rs` | 14 | VerifiedAuthentication, LinkUserInfo, create_link_states, WebAuthnManager |

### What IS Tested

| Area | What is Covered |
|------|----------------|
| auth_provider | JwtAuthProvider, JwtAuthorizationProvider, TraitBasedAuthService — token validation, audience matching, error handling |
| providers | JwtValidationProviderImpl — validate_token, generate_token, generate_secure_token |
| generation | Token generation, hashing, verification |
| jwt | JWT validation, signing, extraction |
| validation | Token/claim validation, redirect URI, OAuth params, MCP OAuth flow, client credentials |
| session | Service construction, data types, error types, builder methods |
| webauthn | Config + builders, JWT validator, token generation/validation, user service, service types |
| cimd | Fetcher URL validation, client type dispatch, metadata edge cases |
| templating | HTML template rendering |
| http | Basic HTTP utilities |

### What is NOT Tested

| Area | Gap Description |
|------|-----------------|
| Session creation/lookup flows | Requires full runtime with Config singleton + DB providers |
| WebAuthn ceremonies | Registration/authentication need webauthn-rs + OAuthRepository (DB) |
| CIMD validator (full) | ClientValidator.validate_client needs DB for DCR lookup |
| client_credentials validation | validate_client_credentials needs OAuthRepository |
| OAuth consent flows | Consent screen logic depends on DB state |
| Complete OAuth flows | authorize -> token -> userinfo (integration test scope) |

### Risk Assessment

The security-critical service layer is now well-tested. Auth providers, token validation, JWT handling, and WebAuthn config/token/user services have comprehensive unit tests. Remaining gaps are primarily in areas requiring database integration tests (session flows, credential validation, full WebAuthn ceremonies).

---

## Desired State

- The `auth_provider` abstraction has tests for: provider registration, provider selection, configuration validation, and error handling
- Session management has tests for: session creation, expiration, invalidation, fixation prevention, and concurrent session limits
- WebAuthn has tests for: registration ceremony (challenge generation, attestation verification, credential storage), authentication ceremony (challenge generation, assertion verification), and error handling for malformed/replayed credentials
- Complete OAuth flows have integration tests for: authorization code flow, token refresh, token revocation, and provider-specific behavior
- All security-critical edge cases have explicit tests: algorithm confusion, key rotation, token replay, session fixation, CSRF validation
- Target: 60%+ source file coverage with security-focused test cases

---

## How to Get There

### Phase 1: Security-Critical Tests (Highest Priority)

1. **Token validation edge cases:**
   - Test algorithm confusion attacks (e.g., RS256 token validated as HS256)
   - Test expired tokens at boundary conditions (exact expiry, 1 second before/after, clock skew)
   - Test malformed tokens: missing claims, extra claims, wrong types, truncated signatures
   - Test key rotation: token signed with old key, token signed with unknown key

2. **Session security:**
   - Test session fixation: verify session ID changes after login
   - Test session expiration: verify expired sessions are rejected
   - Test concurrent sessions: verify limits are enforced if configured

3. **CSRF protection:**
   - Test state parameter validation in OAuth callback
   - Test CSRF token presence and correctness on state-changing endpoints

### Phase 2: WebAuthn Tests

1. Test registration ceremony:
   - Challenge generation: uniqueness, expiration, binding to session
   - Attestation verification: valid attestation accepted, invalid rejected
   - Credential storage: credential ID and public key persisted correctly
2. Test authentication ceremony:
   - Challenge generation and binding
   - Assertion verification: valid assertion accepted, replayed assertion rejected
   - Counter verification: counter must increment, rollback detected
3. Test error handling:
   - Malformed CBOR/attestation objects
   - Unsupported attestation formats
   - Timeout on ceremony completion

### Phase 3: OAuth Flow Integration Tests

1. Test authorization code flow end-to-end: authorize redirect, callback handling, token exchange, userinfo retrieval
2. Test token refresh: valid refresh, expired refresh token, revoked refresh token
3. Test token revocation: access token revocation, refresh token revocation, cascade behavior
4. Test multi-provider: switching providers, linking accounts, provider-specific claim mapping

### Phase 4: Auth Provider and Consent

1. Test `auth_provider` abstraction: provider registration, lookup, configuration validation
2. Test consent flows: consent screen generation, consent recording, consent revocation
3. Test CIMD integration: connection lifecycle, error handling

---

## Incremental Improvement Strategy

### Week 1-2: Security Hardening (Non-Negotiable First Step)
- Write token validation edge case tests (algorithm confusion, expiration boundaries, malformed tokens)
- Write session fixation and CSRF tests
- Target: 30 new security-focused tests. These are the highest-priority tests in the entire codebase.

### Week 3-4: WebAuthn Coverage
- Write registration ceremony tests with test fixtures for attestation objects
- Write authentication ceremony tests with assertion verification
- Write replay and counter tests
- Target: 25 new tests covering the WebAuthn protocol

### Week 5-6: OAuth Flow Integration
- Write end-to-end OAuth flow integration tests (authorize, callback, token, userinfo)
- Write token refresh and revocation tests
- Target: 15 new integration tests

### Week 7-8: Provider Abstraction and Consent
- Write `auth_provider` unit tests
- Write consent flow tests
- Write multi-provider scenario tests
- Target: 20 new tests, coverage moves to 55%+

### Ongoing
- Every security-related change must include corresponding test updates (enforce via PR review checklist)
- Quarterly security audit: review OAuth test coverage against OWASP OAuth security guidelines
- WebAuthn specification updates must be accompanied by updated test fixtures
- Token validation tests must be run against multiple JWT libraries to detect implementation-specific vulnerabilities
