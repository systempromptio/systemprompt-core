# Entry: API Crate Coverage

## Current State

The API crate is the largest entry point in the project with 152 source files comprising approximately 4,400 lines of code in services alone. It defines 69 route handler files across 12 modules, 17 middleware files, and service implementations for server, proxy, static content, and health.

Test coverage stands at 6.6% of source files (10 out of 152), with all 83 tests running synchronously in `crates/tests/unit/entry/api/src/`.

### What IS Tested

- **Routes**: 1 of 69 route files (`sync_types.rs`) has tests.
- **Middleware**: `trailing_slash` and `bot_detector` have 3 tests total. The remaining 15 middleware files (auth, JWT, CORS, rate limiting, analytics, context extraction, session, site auth, throttle, trace) have zero coverage.
- **Services**: Only `health.rs` is tested.
- **Models**: 1 test file covers model logic.
- **Static content**: 1 test file exists.

### What is NOT Tested

Every route module has zero test coverage:

| Module | Untested Files |
|--------|---------------|
| admin | cli.rs |
| agent | artifacts, contexts operations, tasks, registry, responses, webhook (8 files) |
| analytics | events, stream (2 files) |
| content | blog, links, query (4 files) |
| engagement | handlers (1 file) |
| marketplace | 1 file |
| mcp | registry (1 file) |
| oauth | ALL 21+ endpoints: token, authorize, callback, webauthn, client config, consent, introspect, revoke, register, register/dynamic, userinfo, anonymous session, discovery, wellknown, health |
| proxy | agents, mcp (2 files) |
| stream | contexts (1 file) |
| sync | auth, files (2 files) |
| wellknown | 1 file |

Critical middleware with zero coverage:

- Auth middleware (JWT extraction and validation)
- Rate limiting middleware
- CORS middleware
- Session management middleware
- Analytics tracking middleware
- Context extraction middleware
- Site auth middleware
- Throttling middleware
- Trace middleware

### Risk Assessment

The API is the public-facing surface of the application. Zero endpoint tests means regressions in request handling, parameter validation, and response formatting will go undetected. Zero auth middleware tests means security regressions are possible. The OAuth module alone has 21+ endpoints with no test coverage, representing a significant compliance and security risk.

## Desired State

- All 17 middleware files have unit tests covering happy path, error path, and edge cases.
- Auth and JWT middleware have dedicated security-focused tests verifying token validation, expiry, malformed input, and permission checking.
- Every route module has at least one test per handler verifying request parsing, response shape, and error responses.
- OAuth endpoints have comprehensive tests covering the full authorization flow, token lifecycle, and error conditions.
- Rate limiting and throttling middleware have tests verifying enforcement behavior.
- Overall API crate coverage reaches 60%+ of source files with meaningful assertions.

## How to Get There

### Phase 1: Security Middleware (Highest Priority)

1. Write unit tests for the auth middleware covering valid tokens, expired tokens, missing tokens, and malformed tokens.
2. Write unit tests for JWT extraction verifying header parsing, claim validation, and error propagation.
3. Write tests for rate limiting middleware verifying that limits are enforced and that responses include appropriate headers.
4. Write tests for CORS middleware verifying allowed origins, methods, and headers.

### Phase 2: OAuth Endpoints

1. Test the token endpoint for all grant types (authorization_code, refresh_token, client_credentials).
2. Test the authorize endpoint for valid and invalid redirect URIs, scopes, and response types.
3. Test introspection and revocation endpoints.
4. Test discovery and wellknown endpoints for spec compliance.
5. Test WebAuthn registration and authentication flows.

### Phase 3: Core Route Modules

1. Test agent routes (artifacts, contexts, tasks, registry, responses, webhook) with mock service layers.
2. Test content routes (blog, links, query) for request parsing and response formatting.
3. Test analytics routes for event ingestion and stream handling.
4. Test proxy routes for request forwarding behavior.

### Phase 4: Remaining Middleware and Services

1. Test session management middleware for session creation, retrieval, and expiry.
2. Test analytics tracking middleware for event capture.
3. Test context extraction middleware for request context building.
4. Test server and proxy services for lifecycle management.

## Incremental Improvement Strategy

### Week 1-2: Auth and Security Middleware

Target: 8 new test files covering auth, JWT, rate limiting, and CORS middleware. This addresses the highest-risk gap immediately. Expected result: security-critical code paths verified, coverage rises to approximately 12%.

### Week 3-4: OAuth Endpoints

Target: 10 new test files covering the core OAuth flow (token, authorize, callback, introspect, revoke, discovery). OAuth is the second-highest risk area due to its security implications and protocol compliance requirements. Expected result: coverage rises to approximately 20%.

### Week 5-6: Agent and Content Routes

Target: 12 new test files covering agent operations (artifacts, contexts, tasks) and content routes (blog, links, query). These are the most frequently used API surfaces. Expected result: coverage rises to approximately 28%.

### Week 7-8: Remaining Routes and Services

Target: 10 new test files covering remaining route modules (analytics, engagement, marketplace, mcp, proxy, stream, sync, wellknown) and service files. Expected result: coverage rises to approximately 35%.

### Ongoing

Add tests for new routes and middleware as they are developed. Enforce a policy that new API endpoints ship with at least one test per handler. Review and expand existing tests to cover error paths and edge cases. Target 60% coverage by end of quarter.
