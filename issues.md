# issues.md — Auth / Identity / Governance Tech Debt

Surfaced 2026-05-21 while reconciling the `systemprompt-template`
preflight demo (`demo/00-preflight.sh`) against the real behaviour of
`systemprompt-core` v0.11.0. Demo narrative previously claimed a
hardened, service-typed plugin principal; the code actually mints an
admin-typed token with a few related gaps. Demos have been corrected to
reflect reality — the items below are the *real* fixes that need to
land in core.

Each issue cites file:line and links to the demo line the fix would
re-enable.

---

## 1. `admin keys issue-plugin-token` mints an Admin-typed token, not a Service principal

**Severity:** high — privilege scope smell. A 365-day token labelled
"plugin" carries `Permission::Admin` and therefore `UserType::Admin`.

**Where:** `crates/entry/cli/src/commands/admin/keys/issue_plugin_token.rs:91-109`

```rust
let authenticated = AuthenticatedUser::new_with_roles(
    user_uuid,
    user.name.clone(),
    user.email.clone(),
    vec![Permission::Admin],   // hardcoded
    user.roles,
);
// …
permissions: vec![Permission::Admin],
audience:    vec![JwtAudience::Api],
resource:    Some("plugin".to_string()),
```

`AuthenticatedUser::user_type()` (`crates/shared/models/src/auth/types.rs:88`)
short-circuits on `Permission::Admin`, so the resulting JWT decodes as
`user_type=admin` regardless of intent.

**Fix options:**
1. Parameterise the handler with a `--user-type` / `--permission` flag,
   default to `Permission::Service`.
2. Create (or look up) a dedicated service user for each plugin and
   mint the token against *that* user, not the calling admin.
3. Either way, surface `UserType::Service` end-to-end so governance
   policies can route on type rather than peeking at `plugin_id`.

**Demo impact:** preflight Step 3 currently has to caveat the token
shape; once fixed, the table in Step 4 collapses back to
`user_type=service`.

---

## 2. `validate_aud = false` — audience claim is decorative

**Severity:** high — the `aud` array is set, logged, and shown in
demos, but the JWT extractor explicitly disables audience validation.

**Where:** `crates/entry/api/src/services/middleware/jwt/token.rs:32-36`

```rust
fn build_validation() -> Validation {
    let mut validation = Validation::new(Algorithm::RS256);
    validation.validate_exp = true;
    validation.validate_aud = false;   // ← decorative aud
    validation
}
```

A token minted with `aud=[api]` is accepted by routes that nominally
require `aud=[web]` or `aud=[mcp]`. Audience-based isolation between
gateway / dashboard / MCP / A2A surfaces is therefore not enforced.

**Fix:**
1. Flip to `validate_aud = true`.
2. Have each route extractor declare its `required_audience` and
   reject tokens whose `aud` set does not include it (Axum extractor,
   one line per route).
3. Add regression tests covering each surface — `aud=[api]` must not
   open `/mcp/*` and vice versa.

**Demo impact:** lets us tell the truth in the architecture diagrams
("audience is checked") instead of qualifying every claim.

---

## 3. Plugin token's `session_id` is `sess_<uuid>`, not `plugin_<id>`

**Severity:** medium — semantic drift. Downstream code that wants to
identify a token as "this came from a plugin hook" is doing
`session_id.starts_with("plugin_")` and silently falling back when it
fails.

**Where:**
- Mint side: `crates/entry/cli/src/commands/admin/keys/issue_plugin_token.rs:103` calls `SessionId::generate()` which produces `sess_<uuid>`.
- Consumer:  `systemprompt-template/extensions/web/admin/src/handlers/hooks_track/auth.rs:50-55` strips `plugin_` prefix and `unwrap_or("")` — silent failure today.

**Fix:** either
1. Mint `SessionId::from(format!("plugin_{plugin_id}"))` in the
   handler, OR
2. Stop overloading `session_id` and route via the existing
   `plugin_id` claim, then delete the prefix-stripping dead code in
   downstream consumers.

Option 2 is the cleaner doctrine — `session_id` should always be a
session, not a discriminator.

---

## 4. No `SYSTEM` / platform `UserType`

**Severity:** medium — the demos and external docs reach for a
"SYSTEM user" concept that does not exist. Internal callers (job
scheduler, publish_pipeline, hook attribution) currently borrow a real
`users` row via `SystemAdmin` and the `PLATFORM_OWNER` `OnceLock`.

**Where:**
- Enum: `crates/shared/models/src/auth/enums.rs:66` — variants are
  `Admin, User, A2a, Mcp, Service, Anon, Unknown`.
- Bootstrap: `crates/shared/models/src/services/system_admin.rs:7-15`
  and `crates/infra/logging/src/attribution.rs:13-21`.

**Fix options:**
1. Add `UserType::System` and bootstrap a reserved `users` row whose
   token (if any) only the platform itself can sign and which is
   rejected on any externally-reachable route.
2. Or keep the `SystemAdmin` handle but document loudly that there is
   no `SYSTEM` user_type — only an attribution alias. Update README
   and demo wording accordingly.

The status quo (a real admin row that *acts* as system) means
governance audit rows for internal jobs are indistinguishable from
human admin actions. That's a real incident-response gap.

---

## 5. `UserType::Anon` exists but "every request is forced to a user" is the stated invariant

**Severity:** low — documentation/code mismatch. The README and several
demos claim there is no anonymous path; the enum has `Anon`, and no
top-level grep proves it is unreachable.

**Where:** `crates/shared/models/src/auth/enums.rs:66`.

**Fix:**
1. Audit every route extractor and confirm `UserType::Anon` cannot
   reach any handler that writes to the DB or calls a model.
2. If confirmed dead, delete the variant.
3. If not, document the surfaces that admit `Anon` explicitly (e.g.
   public OAuth landing pages) and reject it everywhere else with a
   typed extractor.

---

## 6. `user_type` is derived at mint, never re-derived on validation

**Severity:** low — defence-in-depth. `AuthenticatedUser::user_type()`
(`crates/shared/models/src/auth/types.rs:88`) maps permissions → type
at JWT creation. The validator (`jwt/token.rs:74-110`) trusts the
`user_type` claim straight from the token.

If a future bug lets `Permission::Admin` ride with `user_type=user`
into a token, no server-side check will catch it.

**Fix:** in `validate_jwt_token`, derive `user_type` from `permissions`
and assert it equals the claim. Cheap, compile-time-checked, and
catches token-forging shortcuts.

---

## 7. `act_chain` cap and `nbf` are landed but not exercised by demos

**Severity:** none (capability) — context only. Commits `db917b58` and
`7036d3a3` added `act` claim chaining with depth cap + `nbf`, but no
demo or integration test in the template currently exercises an actor
chain. If you want to *prove* the hardening works publicly, add a
demo that mints a token-on-behalf-of and shows the chain decoding +
depth limit refusal.

**Where:** `crates/shared/models/src/auth/claims.rs:116-118` (`act`
field), `crates/domain/oauth/src/services/generation.rs:62` (chain
build).

**Fix:** new demo under `demo/governance/` — "delegation depth"
scenario. Not blocking, but cheap.

---

## 8. `x-forwarded-for` is trusted without a proxy allowlist

**Severity:** medium — IP-based controls are bypassable. The rate
limiter, IP-ban middleware, and bot detector all derive the client IP
from the `x-forwarded-for` header without verifying the request arrived
through a trusted proxy. A client connecting directly can set the header
to any value and evade all IP-keyed protections.

**Where:** `crates/entry/api/src/services/middleware/rate_limit.rs`,
`crates/entry/api/src/services/middleware/ip_ban.rs`, and the bot
detector.

**Fix:** a `TrustedProxies` layer that only honours `x-forwarded-for`
when the peer address is in a configured proxy allowlist (profile-driven),
applied ahead of the IP-keyed middleware.

---

## 9. No JWT revocation path

**Severity:** medium — a leaked or stale token stays valid until `exp`.
There is no denylist and no revocation endpoint, so logout and
permission-change events cannot terminate an outstanding token
server-side.

**Where:** JWT validation in `crates/infra/security/src/auth/validation.rs`;
OAuth session lifecycle in `crates/domain/oauth/src/services/`.

**Fix:** a short-lived `jti` denylist (in-memory LRU, optionally backed by
a shared store) consulted during validation, populated on logout /
permission change; keep access-token TTLs short with refresh rotation.

---

## Triage suggestion

| # | Area | Effort | Blast radius |
|---|------|--------|--------------|
| 1 | Plugin token user_type | M | High (privilege scope) |
| 2 | validate_aud           | S | High (audience isolation) |
| 3 | session_id format      | S | Medium (silent dead code) |
| 4 | SYSTEM user_type       | M | Medium (audit clarity) |
| 5 | Anon variant           | S | Low |
| 6 | user_type re-derivation| XS| Low (defence-in-depth) |
| 7 | Delegation demo        | S | None (capability proof) |
| 8 | x-forwarded-for trust  | M | Medium (IP-control bypass) |
| 9 | JWT revocation         | M | Medium (stale-token window) |

Recommend bundling **#2 + #6** into one auth-extractor hardening PR
(both touch `jwt/token.rs`, both add to the validation pass) and
**#1 + #3** into a separate "plugin principal" PR (both touch
`issue_plugin_token.rs` and the template's consumer).

---

## Cross-references

- Demo updates that motivated this file:
  - `systemprompt-template/demo/00-preflight.sh` (header, Step 3, Step 4 table)
  - `systemprompt-template/demo/performance/01-request-tracing.sh:263`
- Source-of-truth audit captured 2026-05-21; re-validate before
  implementing — core moves fast.
