# Domain: OAuth Crate Coverage

## Current State

**Source code:** 60 source files across 9 service modules in `crates/domain/oauth/src/`
**Test code:** 19 test files in `crates/tests/unit/domain/oauth/src/` (424 tests -- all sync)
**Integration tests:** 14 tests in `crates/tests/integration/oauth/` (client lifecycle, tokens, webauthn)
**Coverage:** 24% of source files have corresponding tests

### What IS Tested

| Area | What is Covered |
|------|----------------|
| generation | Token generation (partial) |
| jwt | 2 test files -- JWT validation, signing |
| validation | Token/claim validation, redirect URI validation (recently fixed), MCP OAuth flow |
| templating | HTML template rendering |
| http | Basic HTTP utilities |

### What is NOT Tested

| Area | Gap Description |
|------|-----------------|
| auth_provider | Main OAuth abstraction -- ZERO tests |
| Session management | Session service -- ZERO tests |
| WebAuthn flows | Registration, authentication -- 6 files, ZERO tests |
| CIMD integration | CIMD connection -- ZERO tests |
| OAuth consent flows | Consent screen logic -- ZERO tests |
| Token generation errors | Invalid claims, expired tokens -- ZERO tests |
| Multi-provider scenarios | Multiple OAuth providers -- ZERO tests |
| Complete OAuth flows | authorize -> token -> userinfo -- ZERO integration tests |

### Security-Critical Gaps

| Gap | Risk |
|-----|------|
| Token validation edge cases | Malformed tokens, algorithm confusion, key rotation mid-flight -- untested |
| Replay attack prevention | No tests verify that tokens cannot be reused after consumption |
| Session fixation prevention | No tests verify session ID regeneration after authentication |
| CSRF token handling | No tests verify CSRF tokens are validated on state-changing requests |
| Token expiration boundaries | No tests for behavior at exact expiration time, clock skew, or grace periods |

### Risk Assessment

OAuth is the authentication layer. Untested auth code is a security liability. The WebAuthn implementation (passwordless authentication) has zero tests despite being a complex cryptographic protocol with multiple round-trips, challenge verification, and credential storage. A bug in any of these areas could allow unauthorized access or lock out legitimate users.

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
