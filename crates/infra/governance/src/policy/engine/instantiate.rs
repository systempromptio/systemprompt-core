//! Instantiating one configured policy against the catalog, and the boot
//! refusals for a policy that could never enforce what it declares.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use super::GovernanceEngineError;
use crate::policy::GovernancePolicy;
use crate::policy::builtin::SECRET_SCAN_ID;
use crate::policy::config::PolicyConfig;
use crate::policy::rate_window::RateWindowRepository;
use crate::policy::registry::{BuiltPolicy, PolicyCatalog, PolicyConfigurationError};

// Why: the warn-only defaults rely on an empty pattern catalog being legal,
// but an operator-authored `enforce` block that compiles to a scanner which
// can never deny is a silent non-enforcement — refuse it at boot. A globally
// disabled engine enforces nothing, so it is not silently non-enforcing.
pub(super) fn reject_toothless_enforcement(
    cfg: &PolicyConfig,
    built: &BuiltPolicy,
) -> Result<(), GovernanceEngineError> {
    if cfg.id != SECRET_SCAN_ID || !cfg.enabled || cfg.mode.is_warn() {
        return Ok(());
    }
    let toothless = built
        .scanner
        .as_ref()
        .is_none_or(|scanner| scanner.pattern_count() == 0);
    if toothless {
        return Err(GovernanceEngineError::InvalidPolicyConfiguration {
            id: cfg.id.clone(),
            source: PolicyConfigurationError::ToothlessSecretScan,
        });
    }
    Ok(())
}

pub(super) fn reject_uncounted_rate_window(
    cfg: &PolicyConfig,
    instance: &dyn GovernancePolicy,
    rate_windows: Option<&RateWindowRepository>,
) -> Result<(), GovernanceEngineError> {
    if !cfg.enabled || rate_windows.is_some() || instance.rate_window().is_none() {
        return Ok(());
    }
    Err(GovernanceEngineError::RateWindowsUnavailable { id: cfg.id.clone() })
}

pub(super) fn instantiate(
    catalog: &PolicyCatalog,
    cfg: &PolicyConfig,
) -> Result<BuiltPolicy, GovernanceEngineError> {
    let invalid = |source| GovernanceEngineError::InvalidPolicyConfiguration {
        id: cfg.id.clone(),
        source,
    };
    if let Some(factory) = catalog.builtin_factory(cfg.id.as_str()) {
        return factory(&cfg.params).map_err(invalid);
    }
    let instance = catalog
        .contributed(cfg.id.as_str())
        .ok_or_else(|| GovernanceEngineError::UnknownPolicyId { id: cfg.id.clone() })?;
    if !cfg.params.is_null() {
        return Err(invalid(PolicyConfigurationError::ContributedPolicyParams));
    }
    Ok(BuiltPolicy::new(Arc::clone(instance)))
}
