# Changelog

## [0.71.0] - 2026-10-21

### Added

- The `authz` decision plane and the `policy` governed-call chain, with the `authz` extension that owns the access-control and governance tables and their migrations.
- `GovernanceEngine::charge_and_evaluate` (and `charge`) charges a call's rate windows and evaluates it in one step; `RateCounts::charged` builds pre-charged counts.

### Changed

- **Breaking:** `GovernanceEngine::from_config` and `from_services_root` take the rate-window repository, and a chain that arms a rate window without one fails with `GovernanceEngineError::RateWindowsUnavailable`; `charge_rate_windows`, `with_rate_windows` and `RateChargeSubject` are removed.
