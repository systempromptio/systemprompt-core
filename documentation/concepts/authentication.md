# Authentication and authorization

How systemprompt-core establishes who a caller is (authentication), how it decides what they may do (authorization), and the design choices that make both fail closed.

The platform separates two concerns. Authentication proves identity and is carried by a JWT minted through an OAuth2/OIDC or WebAuthn flow. Authorization is a separate, fail-closed decision made by an authorization hook on every governed request. A valid token is necessary but not sufficient: the hook can still deny.

## Token issuance: OAuth2/OIDC and WebAuthn

Identity is established through standard OAuth 2.x / OIDC flows served under `/api/v1/core/oauth` (with `authorize`, `token`, `callback`, `consent`, `register`, and `clients` sub-routes), advertised through the usual discovery documents (`/.well-known/openid-configuration`, `oauth-authorization-server`, `oauth-protected-resource`, `jwks.json`). WebAuthn completes through `/api/v1/core/oauth/webauthn/complete`.

The authorization-code flow is hardened against code interception and replay:

- **PKCE with S256.** The stored code challenge is verified at token exchange using a constant-time comparison (`subtle::ConstantTimeEq`).
- **Single-use codes.** A code is consumed on first use; replay of an already-used code actively revokes the entire refresh-token family it belongs to.
- **At-rest hardening.** Authorization codes and refresh-token identifiers are HMAC-SHA-256 peppered before storage, so a database snapshot yields opaque digests rather than usable secrets.
- **Exact-match redirect URIs.** Redirect URIs are checked against an exact-match allowlist.

> **Open item — PKCE is verified but not mandated.** A challenge is only verified when one was stored. An authorize request from a public client that omits `code_challenge` stores no challenge, and the subsequent exchange skips PKCE. There is no server-side requirement that public clients present a challenge. Treat PKCE enforcement for public clients as a hardening item, not a guarantee.

## JWT validation

Every JWT is validated by `AuthValidationService` (`crates/infra/security/src/auth/validation.rs`). The validation is deliberately narrow:

| Check | Behaviour | Source |
|-------|-----------|--------|
| Algorithm | **RS256 only.** The header `alg` is read and any value other than `RS256` is rejected before verification — `none` and algorithm-confusion attempts are refused. There is no ES/EdDSA acceptance path. | `validation.rs:92` |
| Key identifier | `kid` is required; the matching decoding key is resolved by `kid`. A missing or unknown `kid` is rejected. | `validation.rs:95` |
| Time claims | `exp`, `nbf`, and `iat` are validated with a pinned 30-second leeway (set explicitly in code rather than inherited from the library default). | `validation.rs:100` |
| Delegation chain | The `act` (actor) delegation chain depth is capped; a token whose chain exceeds the maximum is rejected. | `validation.rs:114` |
| User type | The principal's `user_type` is re-derived from the permission set and a disagreeing claim is rejected, catching forged or mis-minted tokens that signature checks alone would pass. | `crates/entry/api/src/services/middleware/jwt/token.rs:91` |
| Revocation | A JTI revocation check backed by the database (with a negative cache) runs on each request and fails closed: a lookup error returns 401, not an allow. | `crates/entry/api/src/services/middleware/jti_revocation.rs:67` |

The RS256 pin is the mitigation for the RUSTSEC-2023-0071 RSA timing class: the validator never enters a non-RS256 verification path, and there is no algorithm flexibility to exploit.

### Open item: audience is not enforced at the primary extractor

The `aud` claim is set when a token is minted and is shown in operator tooling, but the primary API JWT extractor sets `validate_aud = false` (`crates/entry/api/src/services/middleware/jwt/token.rs:35`; the agent JWT path does the same). A token minted for one surface (for example `aud=[api]`) is accepted on routes nominally scoped to other surfaces. The MCP RBAC path does perform an audience check, so MCP is partially defended, but the main extractor is not.

Do not describe audience-based isolation between the gateway, dashboard, MCP, and A2A surfaces as enforced. It is a known open item.

## Scopes and RBAC

Permissions travel in the token's `scope`/permission set and map to a user type. The principal types are `Admin`, `User`, `A2a`, `Mcp`, `Service`, and `Anon` (with an `Unknown` fallback). The user type is derived from permissions at validation time rather than trusted from the claim, so a permission set that includes the admin permission yields an `Admin` principal regardless of what the token asserts.

There is currently no distinct `System`/platform principal. Internal callers (the scheduler, the publish pipeline, hook attribution) borrow a real admin user row, so audit rows for internal jobs are not distinguishable from human-admin actions. Note this where attribution matters.

## The authorization hook

Authentication answers "who"; the authorization hook answers "may they". It is constructed once at startup by `build_authz_hook` (`crates/infra/security/src/authz/runtime.rs:36`) from the profile's `governance.authz` block, and the resulting `Arc<dyn AuthzDecisionHook>` is stored on the `AppContext` and threaded to every consumer. It is fail-closed by construction.

```
governance.authz.hook.mode
   │
   ├─ webhook       → WebhookHook   (url must pass the SSRF guard at boot;
   │                                 a non-2xx, transport error, or decode
   │                                 failure at decision time → DENY)
   │
   ├─ disabled      → DenyAllHook   (every request denied)
   │
   ├─ (block absent)→ DenyAllHook   (every request denied)
   │
   └─ unrestricted  → AllowAllHook  (every request allowed) — ONLY if
                                     `acknowledgement` exactly equals the
                                     required string, else bootstrap fails
```

Three properties make this trustworthy:

1. **Absent or `disabled` config denies everything.** A missing `governance.authz` block does not open the gate; it closes it.
2. **Webhook faults deny.** In `webhook` mode the decision is delegated to an external endpoint. Any fault — non-2xx, transport error, malformed response — results in a denial, not an allow.
3. **The escape hatch is gated.** `unrestricted` (allow-all) mode requires an exact acknowledgement string in the profile; without it, bootstrap fails. An error-level warning is always logged. Refusing this mode in production is the operator's responsibility.

The webhook URL is validated by the shared outbound-URL guard (`validate_outbound_url`) at bootstrap, so a hook pointing at loopback over `http`, at `169.254.169.254`, or into an RFC1918 range fails to start — the same guard that closes SSRF on the outbound webhook delivery path.

Every decision is written to the `governance_decisions` audit table when a database pool is available; the hook is built after the pool exists precisely so its audit sink can persist.

## See also

- [The threat model](../security/threat-model.md) and [compliance control matrix](../security/compliance-control-matrix.md) for the full security treatment.
- [a2a-protocol.md](a2a-protocol.md) and [mcp.md](mcp.md) for the surfaces these tokens authenticate against.
- [The configuration guide](../guides/configure.md) for the `governance.authz` profile block.
