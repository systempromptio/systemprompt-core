//! Inventory-based registration for governance policies.
//!
//! Companion to [`crate::authz::AuthzHookRegistration`]: policies register a
//! factory at static-init time and [`super::GovernanceEngine::from_config`]
//! resolves configured ids against the collected set. The four built-in
//! policies in [`super::builtin`] self-register here; extensions add their own
//! via [`crate::register_governance_policy!`] and enable them from the same
//! `governance.policies` YAML sequence.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_yaml::Value as YamlValue;

use super::secrets::SecretPatternError;
use super::types::GovernancePolicy;

/// Constructs one policy instance from its raw YAML config entry.
///
/// Runs once per [`super::GovernanceEngine::from_config`] call and must not
/// block; a factory receives `YamlValue::Null` when the policy is absent from
/// config. A rejected entry is an error the engine refuses to start on.
pub type PolicyFactory =
    fn(&YamlValue) -> Result<Box<dyn GovernancePolicy>, PolicyConfigurationError>;

/// Why a policy factory rejected its YAML entry.
#[derive(Debug, thiserror::Error)]
pub enum PolicyConfigurationError {
    #[error("unknown access scope `{scope}` in require_approval exempt_scopes")]
    UnknownExemptScope { scope: String },

    #[error(
        "require_approval condition on `{tool}` at `{path}` has no operand its `{operator}` \
         operator can use"
    )]
    UnusableCondition {
        tool: String,
        path: String,
        operator: &'static str,
    },

    #[error(
        "secret_scan is in enforce mode but compiles no secret patterns; declare `patterns` or \
         set `mode: warn`"
    )]
    ToothlessSecretScan,

    #[error("{context}: {source}")]
    Yaml {
        context: &'static str,
        #[source]
        source: serde_yaml::Error,
    },

    #[error(transparent)]
    SecretPatterns(#[from] SecretPatternError),
}

/// One inventory submission per policy. `id` is the stable referent used in
/// `governance.policies` YAML and in `governance_decisions.policy`.
#[derive(Debug, Clone, Copy)]
pub struct PolicyRegistration {
    pub id: &'static str,
    pub factory: PolicyFactory,
}

inventory::collect!(PolicyRegistration);

#[doc(hidden)]
pub use inventory;

#[macro_export]
macro_rules! register_governance_policy {
    ($id:expr, $factory:expr) => {
        $crate::policy::registry::inventory::submit! {
            $crate::policy::PolicyRegistration {
                id: $id,
                factory: $factory,
            }
        }
    };
}
