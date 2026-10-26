# Changelog

## [0.73.0] - 2026-10-25

### Breaking

- `AccessControlIngestionService::ingest_marketplace_access`, `ReconcileReport::marketplaces` and the marketplace ownership scope of `reconcile_services_authz` / `reconcile_composed_bundles` are removed: a marketplace `access:` block is read by the `access_control` plane (`rules_file::declared_marketplace_entities`) with origin `code:marketplaces/<id>/config.yaml`. `reconcile_services_authz` no longer takes an `IngestScope`.
- A gateway decision on a request no declared route matched has `EntityRef::Model` as its entity; `pattern_parents` gives a `model` entity the `provider` level. `AccessControlRepository::update_route_ids` is replaced by `move_route_rules_to_models`.
- `GovernanceEngine::load(pool, rate_windows, UnappliedPlaneMode)`: an empty `governance_config` whose plane was never applied runs no chain; under `deny` every call is `Deny { DenyReason::PlaneNotApplied }`, under `warn` it is `Warn` with that reason (`is_applied`, `unapplied_mode`). The silent warn-only fallback is gone; a plane applied without a declaration runs the warn-only chain.
- The `governance` config plane (`policy::plane`, table `governance_config`); `GovernanceEngine::load` replaces `from_services_root`; `GovernanceConfig::load` and `GovernanceConfigError::Unreadable` are removed and `DuplicatePolicyId`/`Canonical` are new.
- `rules.yaml` is the one access-control dialect and the `access_control` config plane (`AccessControlPlane`, `rules_file`, `render_current_rules`); roles.yaml ingestion is deleted; `EntityKind::{Model, Provider}` and decision-time pattern levels (`DecisionFacts`, `ParentChainIndex::pattern_parents`); `AuthzContext::gateway_invocation` takes the provider; closed `source` vocabulary (`SERVICES_SOURCE`, `CONSOLE_SOURCE_PREFIX`) and migration `027_entitlement_kinds_closed_source`; `AccessControlRepository::{list_pattern_entities, update_route_ids}`.
- `AuthzDecisionHook` gains a required `probe`, which decides a request without writing an audit row and must return what `evaluate` would. Every implementation, including an extension's, adds one. `RuleBasedHook`, `CompositeAuthzHook`, `WebhookHook`, `DenyAllHook` and `AllowAllHook` implement it without touching their audit sink.
- `systemprompt::RuntimeBuilder`, `systemprompt_governance::authz::ReconcileReport` and `systemprompt_cli::admin::config::catalog::CatalogCommands` implement `Copy`, so a non-`move` closure captures them by reference. `#[must_use]` is added to `AgentRegistry::new`, `AgentRegistryProviderService::new`, `AgentReconciler::new`, `AgentMonitor::new`, `AgentDatabaseService::new`, `LogActor::platform`, `GatewayConfig::find_route`, `DeploymentService::{load_config, list_enabled_servers}`, `bridge_profile::{provider_health, is_model_servable}` and `route_needs_thought_signatures`. Migrate by using each result, or binding it to `_` where it is discarded on purpose.
- `SchedulerError::UnknownRetentionTable` is removed and `SchedulerError` gains `Logging(LoggingError)` and `Mcp(McpDomainError)`; `RuntimeError` gains `SubjectSource(AlreadyBound)` and `AtRest(AtRestCipherError)`; `AuthzError` gains `Rules(RulesProblems)` and `Canonical(serde_json::Error)`. Exhaustive matches need an arm. `RoleDirectory` gains the required `async fn roles_of(&self, user: &UserId) -> Result<Vec<String>, AuthzError>`. Migrate by adding the arms and implementing `roles_of` on every registered role directory.

### Added

- `access_control_rules` and `access_control_entities` carry `changed_by`, `changed_at` and `reason`, stamped by `sp_stamp_change()`, and bump `config_version` (`config:access_control`). Migration `026_access_control_change_provenance` (with down): catalog-only `ADD COLUMN`s, ACCESS EXCLUSIVE for milliseconds, no rewrite or scan.
- `SubjectProviderSet::gather`: every registered dimension's values for one user.

### Fixed

- The authz reconcile and the base bundle ingest scope reconcile `ResolvedCatalog::route_ids()` (every declared policy route plus the catalog route id).
- Entitlement deny hints name `services/access-control/rules.yaml` instead of `roles.yaml`.

## [0.71.0] - 2026-10-21

### Added

- The `authz` decision plane and the `policy` governed-call chain, with the `authz` extension that owns the access-control and governance tables and their migrations.
- `GovernanceEngine::charge_and_evaluate` (and `charge`) charges a call's rate windows and evaluates it in one step; `RateCounts::charged` builds pre-charged counts.

### Changed

- **Breaking:** `GovernanceEngine::from_config` and `from_services_root` take the rate-window repository, and a chain that arms a rate window without one fails with `GovernanceEngineError::RateWindowsUnavailable`; `charge_rate_windows`, `with_rate_windows` and `RateChargeSubject` are removed.
