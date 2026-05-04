# systemprompt-api Tech Debt Audit

**Layer:** entry
**Audited:** 2026-05-04 (Wave E1 sweep)
**Verdict:** CLEAN

---

## Entry-Binary Exemption

This crate is the HTTP entry binary. Per `instructions/audits/INDEX.md`
"Entry Layer" rule, the §3a Public-API Hygiene checks are explicitly waived:

- `anyhow::Error` may stay at the HTTP boundary (axum handler return types,
  `IntoResponse` mapping). It does not leak into a published library crate's
  public API.
- `///` rustdoc on internal items is NOT required.

All other §3 rules still apply and are enforced below.

---

## Summary

| Category | Count |
|----------|-------|
| unwrap()/expect() | 0 |
| panic!()/todo!()/unimplemented!() | 0 |
| println!/eprintln!/dbg! | 0 |
| `let _ =` discards | 0 |
| `.ok()` discards (uncommented) | 0 |
| Inline `//` comments (non-`Why:`) | 0 |
| Files >300 lines | 0 |
| Raw String IDs | 0 |
| Raw `sqlx::query` (outside allowlist) | 0 |
| `*Manager` suffix | 0 |

**Total scored violations:** 0

---

## Passing Checks

| Check | Status |
|-------|--------|
| `cargo fmt -p systemprompt-api` | PASS |
| `cargo build -p systemprompt-api --all-features` | PASS |
| `cargo clippy -p systemprompt-api --all-targets --all-features -- -D warnings` | PASS |
| `RUSTDOCFLAGS=-D warnings cargo doc -p systemprompt-api --no-deps --all-features` | PASS |
| `just check-bans-crate crates/entry/api` | PASS |
| `just lint-sqlx` | PASS |
| File-size gate (no `.rs` over 300 lines) | PASS |

---

## File Splits Performed

The 9 files over 300 lines from the baseline scan have been split by
cohesion (no `_helpers.rs` shuffles):

| Original | New layout |
|----------|------------|
| `services/server/routes.rs` (379) | `routes.rs` orchestrator + `routes/{protocol,static_setup,extension_mount}.rs` |
| `services/middleware/session.rs` (342) | `session.rs` middleware + `session/{lifecycle,skip}.rs` |
| `services/gateway/upstream.rs` (334) | `upstream.rs` core + `upstream/sse.rs` (OpenAI→Anthropic SSE conversion) |
| `routes/oauth/endpoints/authorize/validation.rs` (332) | `validation.rs` + `validation/{entropy,resource}.rs` |
| `services/gateway/audit.rs` (308) | `audit.rs` + `audit_internal/payload.rs` (slice/truncate helpers; sibling dir name avoids the repo `.gitignore` rule on `audit/`) |
| `routes/oauth/endpoints/token/generation.rs` (307) | `generation.rs` + `generation/client_credentials.rs` |
| `services/static_content/static_files.rs` (306) | `static_files.rs` + `static_files/{cache,responses}.rs` |
| `services/proxy/engine.rs` (303) | `engine.rs` + `engine/mcp_session.rs` (MCP session-cache logic) |
| `services/gateway/stream_tap.rs` (303) | `stream_tap.rs` + `stream_tap/sse_parser.rs` (Anthropic SSE event handlers) |

---

## Carve-out Comments

`Why:` carve-out comments added for every retained `.ok()` /
poisoned-mutex pattern that intentionally drops an error after logging:

- `services/gateway/audit.rs::effective_model` — poisoned `served_model` mutex
- `services/gateway/stream_tap.rs` — three sites (poll-on-Ok, poll-on-Err, Drop)
- `services/middleware/session.rs` — token extraction + session lookup
- `services/middleware/site_auth.rs` — token extraction
- `services/middleware/rate_limit.rs` — non-UTF-8 `x-forwarded-for`
- `services/middleware/bot_detector.rs` — non-UTF-8 user-agent
- `services/middleware/analytics/detection.rs` — best-effort behavioral lookup
- `services/gateway/flatten.rs::parse_served_model` — malformed upstream JSON
- `routes/analytics/events.rs` — slug→content_id lookup
- `routes/engagement/handlers.rs` — slug→content_id lookup
- `routes/oauth/endpoints/callback.rs` — invalid role-string filter
- `routes/oauth/endpoints/token/generation/client_credentials.rs` — invalid scope filter
- `routes/proxy/mcp.rs` — malformed execution-output JSON

---

## Notes

- All clippy lints satisfied at `-D warnings`, including
  `clippy::redundant_clone` and `clippy::too_many_arguments` (handled via
  parameter struct in `mcp_session::McpResponseCtx`).
- The entry-binary exemption means typed-error refactors of internal
  command modules are SHOULD-do, not MUST-do. Where it would have required
  pushing `anyhow` back into a published library crate, the HTTP boundary
  retained `anyhow::Error`. No new `anyhow` references were introduced.
- The `let _agent`/`let _span` named bindings in `routes/wellknown.rs` and
  `services/middleware/context/middleware.rs` are intentional `_`-prefixed
  binders, not `let _ = ...` discards.
