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

use super::types::GovernancePolicy;

/// Constructs one policy instance from its raw YAML config entry.
///
/// Runs once per [`super::GovernanceEngine::from_config`] call and must not
/// block; a factory receives `YamlValue::Null` when the policy is absent from
/// config.
pub type PolicyFactory = fn(&YamlValue) -> Box<dyn GovernancePolicy>;

/// One inventory submission per policy. `id` is the stable referent used in
/// `governance.policies` YAML and in `governance_decisions.policy`.
#[derive(Debug, Clone, Copy)]
pub struct PolicyRegistration {
    pub id: &'static str,
    pub factory: PolicyFactory,
}

inventory::collect!(PolicyRegistration);

/// Register a governance policy factory at static-init time.
///
/// ```ignore
/// systemprompt_security::register_governance_policy!("my_policy", |params| {
///     Box::new(MyPolicy::from_yaml(params))
/// });
/// ```
#[macro_export]
macro_rules! register_governance_policy {
    ($id:expr, $factory:expr) => {
        ::inventory::submit! {
            $crate::policy::PolicyRegistration {
                id: $id,
                factory: $factory,
            }
        }
    };
}
