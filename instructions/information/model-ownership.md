# ADR: Canonical ownership of data models

## Context

A CLI bug in `systemprompt admin agents list` and `systemprompt core agents list` returned different results for the same installation. Root cause: each command tree had defined its own local `AgentSummary` struct as a narrower projection of the same canonical `AgentConfig`, and the two projections had drifted in both field set and loader path. Neither was wrong in isolation — the failure mode was that the shared truth had been forked at the presentation layer.

An audit then found this was a pattern, not an accident. Across the tree there were roughly 30 duplicate or projection structs in the CLI alone, 15 more in domain/infra code, and 240+ raw-`String` ID violations against the existing `systemprompt-identifiers` crate — all of which should have been typed from day one.

This ADR codifies the rules that eliminated the drift, so future reviewers can cite it when rejecting regressions.

## Decision

1. **Single source of truth.** Every data shape with domain meaning lives in `crates/shared/models/` or a sibling shared crate (`identifiers`, `traits`, `provider-contracts`). Entry/domain/app layers do not re-declare those shapes.
2. **Projections via `impl From`.** When a layer needs a narrower view (e.g. a CLI `list` output), that projection lives as `impl From<&CanonicalType> for ProjectionType` next to the canonical type in the shared crate — not as a local `types.rs` in the CLI.
3. **Typed IDs everywhere.** Every domain-meaningful identifier uses a typed wrapper from `systemprompt_identifiers`. Raw `String` / `&str` is banned for field names like `user_id`, `agent_id`, `task_id`, `tenant_id`, `context_id`, `session_id`, `file_id`, `skill_id`, `client_id`, `artifact_id`, `message_id`, `role_id`, `hook_id`, `execution_step_id`, `content_id`, `source_id`.
4. **Banned names inside CLI `types.rs`.** `*Summary`, `*Detail`, `*Info`, `*View` — those names imply "projection of a shared model" and must live in the shared crate. Presentation-only tabular shapes can use `*Row` inside a `display.rs` file.
5. **Legitimate exceptions.**
   - External protocol DTOs (OAuth 2.0, WebAuthn, OpenAI adapter types, MCP wire types, JSON-RPC error envelopes) — shaped by remote specs, not by us.
   - Database `*Row` structs used only with `sqlx::query_as!` and mapped immediately into a domain model.
   - Everything in `crates/tests/`.
   - `trace_id` — no typed equivalent exists yet.

## Enforcement

- **CI gate**: `scripts/lint-raw-ids.sh` greps `crates/domain/`, `crates/app/`, `crates/entry/` for banned field/parameter patterns and exits non-zero on any new violation. Wired into `just lint-raw-ids`.
- **Code review**: reviewers cite this ADR when rejecting local structs that mirror shared ones, or raw-`String` IDs that have a typed equivalent in `systemprompt_identifiers`.

## Consequences

Refactors are safer because there is exactly one place to change a domain shape; silent drift between admin/core (or CLI/API) views of the same entity cannot recur. The cost is one `impl From` per projection, which is small and keeps projection logic next to the type it projects from.

## History

Migrated in five parallel waves on 2026-04-15.

- **Wave 1** added unified `AgentSummary`, `SkillSummary`, `SkillDetail`, `McpServerSummary`, `PluginSummary`, `UserSummary`, `SessionSummary`, `ProfileInfo`, `ArtifactSummary` to `systemprompt-models`; hardened `ContentSummary` trait to use `ContentId`/`SourceId` and `ArtifactMetadata::with_skill_id` to take `SkillId`.
- **Wave 2** deleted local CLI duplicates: two `AgentSummary`, two `McpServerSummary`, three `ProfileInfo`, `UserSummary`, `SessionSummary`, `SkillSummary`, `SkillDetailOutput`, `PluginSummary`, `PluginComponentDetail`, `ContextDetailOutput` (dead code).
- **Wave 3** threaded `TaskId`, `ContextId`, `AgentId`, `UserId`, `ClientId` through A2A protocol (`push_notification.rs`, `events.rs`, `requests.rs`), agent orchestration events, a2a_server artifact processing, and oauth/webauthn endpoints.
- **Wave 4** typed the `AiRequest` sqlx row and its siblings end-to-end, hardened `ai_trace_service` return types, propagated typed IDs through CLI trace commands and `admin session login`, and converted remaining `SourceId` parameters in `app/generator` and `app/sync`.
- **Wave 5** swept residual CLI typed-ID parameters in `core/skills`, `cloud/**`, `admin/**`, `infrastructure/logs/**`; added the `scripts/lint-raw-ids.sh` guardrail and this ADR.

Subsequent reviewers: if this doc goes stale, update it — it is meant to describe the rules the code follows today, not a frozen snapshot.
