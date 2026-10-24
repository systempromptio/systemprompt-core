# Changelog

## [0.73.0] - Unreleased

### Breaking

- `GovernanceEngine::load` fails closed: an empty `governance_config` whose plane was never applied builds a deny-all engine (`is_applied() == false`) that refuses every call with `DenyReason::PlaneNotApplied { plane }`; the warn-only fallback is gone. A plane applied without a declaration runs the warn-only chain.

- The `governance` config plane (`policy::plane`, table `governance_config`); `GovernanceEngine::load` replaces `from_services_root`; `GovernanceConfig::load` and `GovernanceConfigError::Unreadable` are removed and `DuplicatePolicyId`/`Canonical` are new.
- `rules.yaml` is the one access-control dialect and the `access_control` config plane (`AccessControlPlane`, `rules_file`, `render_current_rules`); roles.yaml ingestion is deleted; `EntityKind::{Model, Provider}` and decision-time pattern levels (`DecisionFacts`, `ParentChainIndex::pattern_parents`); `AuthzContext::gateway_invocation` takes the provider; closed `source` vocabulary (`SERVICES_SOURCE`, `CONSOLE_SOURCE_PREFIX`) and migration `027_entitlement_kinds_closed_source`; `AccessControlRepository::{list_pattern_entities, update_route_ids}`.

### Fixed

- The authz reconcile and the base bundle ingest scope reconcile `ResolvedCatalog::route_ids()` (every declared policy route plus the catalog route id).
## [Unreleased]

### Breaking

- `AuthzDecisionHook` gains a required `probe`, which decides a request without writing an audit row and must return what `evaluate` would. Every implementation, including an extension's, adds one. `RuleBasedHook`, `CompositeAuthzHook`, `WebhookHook`, `DenyAllHook` and `AllowAllHook` implement it without touching their audit sink.

### Added

- `access_control_rules` and `access_control_entities` carry `changed_by`, `changed_at` and `reason`, stamped by `sp_stamp_change()`, and bump `config_version` (`config:access_control`). Migration `026_access_control_change_provenance` (with down): catalog-only `ADD COLUMN`s, ACCESS EXCLUSIVE for milliseconds, no rewrite or scan.
- `SubjectProviderSet::gather`: every registered dimension's values for one user.

### Fixed

- Entitlement deny hints name `services/access-control/rules.yaml` instead of `roles.yaml`.

## [0.71.0] - 2026-10-21

### Added

- The `authz` decision plane and the `policy` governed-call chain, with the `authz` extension that owns the access-control and governance tables and their migrations.
- `GovernanceEngine::charge_and_evaluate` (and `charge`) charges a call's rate windows and evaluates it in one step; `RateCounts::charged` builds pre-charged counts.

### Changed

- **Breaking:** `GovernanceEngine::from_config` and `from_services_root` take the rate-window repository, and a chain that arms a rate window without one fails with `GovernanceEngineError::RateWindowsUnavailable`; `charge_rate_windows`, `with_rate_windows` and `RateChargeSubject` are removed.
