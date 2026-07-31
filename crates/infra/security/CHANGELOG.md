# Changelog

## [0.27.0] - 2026-07-29

### Breaking

- **Breaking:** `PolicyContext.tool` is replaced by `target: GovernedTarget`, and `PolicyContext.tool_input` by `input: &GovernedInput`; a governed call is an MCP tool invocation or a submitted prompt, and a prompt names no tool. Migrate by constructing `GovernedTarget::Tool { tool }` / `GovernedInput::tool_arguments(..)` at each enforcement point, and by reading `ctx.target.tool()` where a policy applies to tool calls only.
- **Breaking:** `SecretLocation` carries a third field, `redacted`, and `SecretLocation::new` takes three arguments; `path` holds the dotted path to the offending field. Migrate by passing the path and the redacted excerpt separately.
- **Breaking:** `McpToolInput` moves from `policy::types` to `policy::governed`. Migrate by importing from `systemprompt_security::policy`, which re-exports both modules' public types.
- **Breaking:** `GovernanceChain` is deleted. It returned only the first deny, discarding the per-policy trace every audit row needs, so no consumer ever called it — downstreams each reimplemented a traced loop instead. Migrate to `GovernanceEngine::evaluate`, which returns an `Evaluation { decision, chain }` with a per-entry `ChainEntryOutcome` trace.

### Added

- `governance.enabled: false` switches the whole chain off in one key, leaving the per-policy declarations intact so the configuration survives being turned back on. `GovernanceConfig` gains the matching `enabled` field, and `GovernanceConfig::validate` is the strict boot-time loader — it returns the error so a misconfigured installation refuses to start, where `load` stays lenient for the request path. The fallback direction is deliberate: defaults enable every policy, so an unreadable file yields more enforcement than it declared, never less, and governance cannot be disabled by deleting the file.
- `secret_scan` takes an optional `entropy` block (`enabled`, `min_len`, `threshold`, `allowlist`) configuring the high-entropy backstop. An absent block, an absent key, or a key of the wrong shape falls back to the built-in default — a typo must not silently disable credential detection — and an `allowlist` regex that fails to compile is skipped with an error rather than failing the policy. `policy::secrets::detect_secrets_with` applies a caller-supplied `EntropyConfig`.
- The entropy backstop no longer flags serialised data as a credential. A base64 token that decodes to mostly-text or to a well-formed protobuf message is treated as structured payload rather than key material; a decode counts as protobuf only when it consumes the buffer exactly, carries at least two fields, and holds a length-delimited payload that is itself text or protobuf, which random key material clears by accident far less than one time in a hundred.
- A governance decision whose chain ran entirely disabled records `governance_disabled` rather than `default_allow`, so an unguarded installation is distinguishable from a healthy one in the flat `policy` column that operational queries filter on.

- `GovernanceEngine::global` returns the process-wide engine, so every enforcement point charges one rate limiter. The buckets are instance-scoped, and a second engine gives its callers their own budget and silently doubles every operator limit — the MCP governance webhook and the inference gateway must share one. `GovernanceEngine::from_config` remains for tests and for callers that genuinely want an isolated chain.
- `DecisionAudit.context_id` records the conversational context a governed call belongs to, omitted from the serialized blob when absent. The MCP webhook knows no context; the gateway does, and without it an inference decision cannot be joined back to the request it judged.

- `GovernedInput::strings` yields every string in a governed payload with its dotted path, so a scanner reports the path the input type defines rather than one it reconstructs, and `GovernedInput::location_kind` names the surface a finding sits on (`tool_input` or `prompt`).
- `GovernedTarget::as_str` gives the audit-visible name of a governed target, recording a prompt submission as `PROMPT_TARGET_NAME`.
- `SecretLocation` implements `Display`, and `DenyReason::SecretLeak` renders it in place of a `Debug` dump.
- The governance runtime moves into core. `policy::GovernanceEngine` instantiates the configured chain from the inventory registry (`register_governance_policy!`, mirroring `register_authz_hook!`) and a `GovernanceConfig` parsed from the `governance.policies` YAML shape; `evaluate` runs it first-deny-wins with a full per-entry trace — disabled and skipped-after-deny entries record `skip` with zero duration. Registered policies absent from the config are appended disabled so the trace shows them rather than omitting them. The engine is caller-owned: policy state (the rate limiter's sliding window) is instance-scoped, never process-global.
- The four built-in policies ship with the crate under their stable ids — `secret_scan` (built-in pattern table + `extra_patterns` prefixes), `scope_check` (`admin_only_prefixes`), `tool_blocklist` (`patterns`), and `rate_limit` (per-`{session,user}` sliding window, idempotent per `call_id`) — emitting the typed `DenyReason` variants this crate already defined for them.
- `policy::secrets` exposes the shared credential scanner: `SECRET_PATTERNS`, `detect_secrets` over a `GovernedInput`, `scan_str_for_secret` for string surfaces, and the high-entropy backstop `find_high_entropy_token`.
- `policy::DecisionAudit` (with `PrincipalSnapshot`, `AuditTarget`, `ApproverStamp`, `AuditOrigin`, `ChainEntryOutcome`) is the typed blob persisted into `governance_decisions.evaluated_rules`, and `record_decision` derives the flat columns (`policy` is the first failing chain entry) before delegating to `insert_governance_decision`. `DecisionAudit.act_chain` carries the RFC 8693 delegation lineage; it and `approver` are omitted from the blob when empty, so pre-existing rows and new rows share one shape.

## [0.26.0] - 2026-07-28

### Breaking

- **Breaking:** `PolicyContext` carries a `call_id`, and `GovernancePolicy::evaluate` is documented as idempotent per that id: evaluating one call twice must yield the same `Decision` and leave the same state behind as evaluating it once. One call is legitimately evaluated more than once — an enforcement point behind another still runs the chain, because callers that never passed the first can reach it — so a policy that counts calls must count calls rather than evaluations. Migrate by populating the new field at each enforcement point; a policy that keeps no state is unaffected.

## [0.23.0] - 2026-07-24

### Added

- `AccessControlIngestionService::ingest_config_from_yaml_path` reads an `AccessControlConfig` from disk and ingests it, replacing the `AccessControlLocalSync` wrapper that lived in the deleted `systemprompt-sync` crate.

## [0.22.0] - 2026-07-21

### Breaking

- A declared access-control ruleset is authoritative and closed: `authz::resolve` consults an entity's parents only when the entity declares no rules of its own, so an entity that names roles is closed to every role it does not name, including via a parent's `default_included`. Migrate by adding an explicit `allow` rule for every role that should keep access to an entity that declares any rule, or by removing the entity's rules to restore inheritance.
- `authz::resolve` takes a `ResolveInput` bundle carrying the entity, its rules, the caller, and an ordered `parents` slice of `ResolveParent` values. Migrate by constructing `ResolveInput`; `RuleBasedHook` passes an empty parent slice and is unaffected by the closed-ruleset change.
- `ed25519-dalek` moves from 2 to 3, changing the `SigningKey` / `VerifyingKey` / `Signature` types in `manifest_signing`. The Ed25519 wire format is unchanged, so manifests signed by earlier releases still verify. Migrate by moving dependent crates to `ed25519-dalek` 3.

### Fixed

- `serde_jcs` moves to 0.2, matching the version the bridge verifies with. Core previously canonicalised RFC 8785 payloads with 0.1 while the bridge used 0.2, so any divergence between them would have surfaced as an unexplained manifest signature rejection.

## [0.21.1] - 2026-07-17

### Changed
- Source files now carry a Business Source License 1.1 header referencing <https://systemprompt.io>.

## [0.19.0] - 2026-07-02

### Breaking

- The minimum supported Rust version is 1.94.
- SQLx is upgraded to 0.9.

### Changed

- Workspace version bump; internal tracing-field and comment cleanup in the authorization engine, no public API change.

## [0.17.0] - 2026-06-24

### Added

- Messaging identity ingestion: Slack and Teams users are resolved to authorization entities so chat actors are governed like any other caller.

## [0.16.0] - 2026-06-22

### Breaking

- JWT validation requires a first-party audience claim (`web`, `api`, `a2a`, or `mcp`); tokens minted without an audience are rejected.
- The minimum supported Rust version is 1.88.

## [0.14.0] - 2026-06-01

### Changed

- Workspace version bump; no API changes in this crate.

## [0.13.1] - 2026-06-01

### Changed

- Workspace version bump; no API changes in this crate.

## [0.13.0] - 2026-05-28

### Removed

- `AuthMode` enum and the `AuthMode::Optional` A2A optional-auth anonymous context path. `AuthValidationService::validate_request` now takes only the headers — there is no longer a "optional auth that quietly returns an anonymous principal" mode. Callers requiring anonymous access wire the public/no-auth route flavour instead.

## [0.12.0] - 2026-05-27

### Breaking

- `RuleType::Department`, `DenyReason::DepartmentDeny`, and `MatchedBy::DepartmentAllow` removed from the authz resolver. `ResolveInput` drops its `department` field. Migration `008_drop_department_acl.sql` narrows `access_control_rules.rule_type` to `('role','user')` and deletes any existing department rows.
- `AccessControlRepository::list_role_department_rules_for_export` renamed to `list_role_rules_for_export`.
- `AppContextBuilder::with_authz_hook` is now generic over `H: AuthzDecisionHook + 'static`; callers pass owned hook values. Callers holding an `Arc<dyn AuthzDecisionHook>` use the new `with_shared_authz_hook(SharedAuthzHook)` method.
- `SharedAuthzHook` moved to `systemprompt_security::authz::hook`; the `authz` facade re-export is unchanged.
- `AuthzMode::Extension` selection at bootstrap requires a hook supplied via `with_authz_hook(...)` or registered through `register_authz_hook!`; bootstrap errors if neither is present.

### Added

- `RuleBasedHook` — the core RBAC resolver promoted to a first-class `AuthzDecisionHook`. Wraps the sync `authz::resolver::resolve` so extensions compose it via `CompositeAuthzHook`. Bootstrap composes `[RuleBasedHook, ...extensions]` automatically when a DB pool is available; `mode: webhook` composes `[RuleBasedHook, WebhookHook]`.
- `AuthzSource::RuleBased` audit-source variant (`policy = "authz_rule_based"`) so resolver decisions stay observable in `governance_decisions` alongside webhook and extension rows.
- `authz::registry` inventory site for static-init authz hook registration (`register_authz_hook!`), used when binaries delegate to `systemprompt::cli::run()` and have no builder call-site.

## [0.11.0] - 2026-05-20

### Breaking

- `SessionGenerator::new` now takes only `issuer`; the `jwt_secret` argument is gone. Tokens are signed via the process-wide `TokenAuthority` (RS256) and there is no shared secret to plumb through.
- `AuthValidationService::new` likewise drops the leading `secret` parameter and now takes `(issuer, audiences)`.
- `AdminTokenParams` no longer carries `jwt_secret`. Token signing reads the active RSA key from the `TokenAuthority` cache.

### Added

- `at_rest` module exposing `hmac_sha256` and `hmac_sha256_hex` for storing identifiers (refresh-token ids, authorisation codes) as peppered HMAC-SHA-256 digests rather than plaintext.
- Authorisation policy plumbing supporting the new compile-time `RouterExt::with_auth(_, AuthzPolicy::*)` middleware in `entry/api`. Every authenticated route declares its policy at registration.

### Changed

- `repository.rs` query sites use compile-time-verified `query!` / `query_scalar!` macros throughout, in line with the repository-pattern rule.

### Fixed

- Authz `bootstrap.rs` tests are no longer flaky: a process-wide `tokio::sync::Mutex` serialises the shared global hook slot, so concurrent tests no longer observe half-installed hooks.

## [0.9.2] - 2026-05-14

### Added

- `authz` module: deny-overrides resolver, `access_control_rules` repository, and `AuthzDecisionHook` extension surface shared by gateway and MCP enforcement.
- `authz::audit` submodule: `AuthzAuditSink`, `DbAuditSink`, `NullAuditSink`, and `GovernanceDecisionRepository` for governance decision persistence.
- `authz::ingestion::AccessControlIngestionService` for loading rule sets from configuration.
- `AllowAllHook`, `DenyAllHook`, and `WebhookHook` implementations of `AuthzDecisionHook`.
- `auth::HookTokenValidator` and `ValidatedHookClaims` for bridge hook-token minting and verification.
- `JwtAudience::Cowork` audience variant wired through `AuthValidationService`.

### Changed

- Crate description reframed around the four-layer governance pipeline and unified authz decision plane.

## [0.4.3] - 2026-04-29

### Breaking

- **Breaking:** Removed `DOMAIN_SEPARATOR` and the `Sha256(DOMAIN_SEPARATOR || jwt_secret)` derivation path. Migrate by configuring `manifest_signing_secret_seed` directly.

### Added

- `manifest_signing::sign_value<T: Serialize>` and `canonicalize<T>` for RFC 8785 JCS canonical JSON.
- `serde_jcs` dependency.

### Changed

- `manifest_signing::signing_key` reads its ed25519 seed from `manifest_signing_secret_seed`, isolating manifest signatures from JWT HMAC compromise.

## [0.3.0] - 2026-04-22

### Fixed

- `signing_key` removes a redundant clone and handles concurrent initialisation via `OnceLock::set` instead of `expect`.

## [0.1.18] - 2026-03-27

### Breaking

- **Breaking:** Removed hardcoded `sp_tui` client ID from JWT generation. Migrate by passing `client_id` on `AdminTokenParams`.

### Added

- `client_id` field on `AdminTokenParams` for configurable JWT client ID.

### Changed

- Upgraded to Rust 2024 edition.

## [0.1.0] - 2026-02-02

### Changed

- First stable release at workspace-aligned version.

## [0.0.13] - 2026-01-27

### Changed

- Version bump for workspace alignment.

## [0.0.11] - 2026-01-26

### Fixed

- Resolved clippy warnings in the security scanner module.

## [0.0.3] - 2026-01-22

### Added

- Migration system infrastructure.

### Fixed

- Schema validation now accepts VIEW-based schemas.

## [0.0.2] - 2026-01-22

### Changed

- Each domain crate now owns its SQL schemas via the `Extension` trait; centralised module loaders removed from `systemprompt-loader`.

### Fixed

- Corrected `include_str!` paths that pointed outside the crate directory so the crate compiles standalone from crates.io.

## [0.0.1] - 2026-01-21

### Added

- Initial release.
