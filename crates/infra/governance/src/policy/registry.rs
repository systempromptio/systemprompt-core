//! The policy catalog the governance engine resolves configured ids against.
//!
//! Two sources feed it. The built-in policies in [`super::builtin`] are listed
//! in [`super::builtin::BUILTIN_POLICIES`] and constructed per engine build
//! from their `governance.policies` YAML entry. Members contribute their own
//! through `Contributions::policy`; [`PolicyCatalog::from_admitted`] runs every
//! admitted member's factory once — core members first, then installs in
//! composition order — and refuses a contributed id that equals a built-in
//! one or another contribution. A contributed policy carries its own settings,
//! so a `governance.policies` entry naming it may only enable it and set its
//! mode; an entry with `params` is refused.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use serde_yaml::Value as YamlValue;
use systemprompt_extension::{BuiltState, FactoryError, MemberResolved, Resolved, StandingTag};
use systemprompt_identifiers::{ExtensionId, PolicyId};

use super::GovernancePolicy;
use super::builtin::BUILTIN_POLICIES;
use super::secrets::{SecretPatternError, SecretScanner};

/// Constructs one built-in policy from its raw YAML config entry.
///
/// Runs once per [`super::GovernanceEngine::from_config`] call and must not
/// block; a factory receives `YamlValue::Null` when the policy is absent from
/// config. A rejected entry is an error the engine refuses to start on.
pub type PolicyFactory = fn(&YamlValue) -> Result<BuiltPolicy, PolicyConfigurationError>;

/// A constructed policy, with the credential scanner it evaluates over when it
/// is the built-in secret scan.
#[derive(Debug, Clone)]
pub struct BuiltPolicy {
    pub instance: Arc<dyn GovernancePolicy>,
    pub scanner: Option<SecretScanner>,
}

impl BuiltPolicy {
    #[must_use]
    pub fn new(instance: Arc<dyn GovernancePolicy>) -> Self {
        Self {
            instance,
            scanner: None,
        }
    }

    #[must_use]
    pub fn with_scanner(mut self, scanner: SecretScanner) -> Self {
        self.scanner = Some(scanner);
        self
    }
}

/// One built-in policy: its stable id and its YAML factory.
#[derive(Debug, Clone, Copy)]
pub struct BuiltinPolicy {
    pub id: &'static str,
    pub factory: PolicyFactory,
}

/// Why a policy rejected its YAML entry.
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

    #[error("a contributed policy carries its own settings; its config entry may not set `params`")]
    ContributedPolicyParams,

    #[error("{context}: {source}")]
    Yaml {
        context: &'static str,
        #[source]
        source: serde_yaml::Error,
    },

    #[error(transparent)]
    SecretPatterns(#[from] SecretPatternError),
}

/// Why the contributed policies could not be collected.
#[derive(Debug, thiserror::Error)]
pub enum PolicyCatalogError {
    #[error("policy factory of {owner} failed")]
    Factory {
        owner: ExtensionId,
        #[source]
        source: FactoryError,
    },
    #[error("{owner} contributes policy `{id}`, which is a built-in policy id")]
    ShadowsBuiltin { owner: ExtensionId, id: PolicyId },
    #[error("policy `{id}` contributed twice: by {first} and by {second}")]
    DuplicateId {
        id: PolicyId,
        first: ExtensionId,
        second: ExtensionId,
    },
}

#[derive(Debug, Clone)]
struct ContributedPolicy {
    owner: ExtensionId,
    instance: Arc<dyn GovernancePolicy>,
}

/// The built-in policies plus every admitted member's contributed policy.
#[derive(Debug, Clone)]
pub struct PolicyCatalog {
    builtins: &'static [BuiltinPolicy],
    contributed: Vec<ContributedPolicy>,
}

impl Default for PolicyCatalog {
    fn default() -> Self {
        Self::builtin()
    }
}

impl PolicyCatalog {
    #[must_use]
    pub const fn builtin() -> Self {
        Self {
            builtins: BUILTIN_POLICIES,
            contributed: Vec::new(),
        }
    }

    pub fn from_admitted(built: &BuiltState, r: &Resolved<'_>) -> Result<Self, PolicyCatalogError> {
        let mut catalog = Self::builtin();
        for standing in [StandingTag::Core, StandingTag::Install] {
            for member in built.members().filter(|m| m.standing == standing) {
                let scope = member.scope();
                for factory in member.sink.policies() {
                    let instance =
                        factory
                            .build(&MemberResolved::within(r, &scope))
                            .map_err(|source| PolicyCatalogError::Factory {
                                owner: member.owner.clone(),
                                source,
                            })?;
                    catalog = catalog.with_contributed(member.owner.clone(), instance)?;
                }
            }
        }
        Ok(catalog)
    }

    pub fn with_contributed(
        mut self,
        owner: ExtensionId,
        instance: Arc<dyn GovernancePolicy>,
    ) -> Result<Self, PolicyCatalogError> {
        let id = instance.id();
        if self.builtins.iter().any(|b| b.id == id.as_str()) {
            return Err(PolicyCatalogError::ShadowsBuiltin { owner, id });
        }
        if let Some(first) = self.contributed.iter().find(|c| c.instance.id() == id) {
            return Err(PolicyCatalogError::DuplicateId {
                id,
                first: first.owner.clone(),
                second: owner,
            });
        }
        self.contributed.push(ContributedPolicy { owner, instance });
        Ok(self)
    }

    pub(super) fn builtin_factory(&self, id: &str) -> Option<PolicyFactory> {
        self.builtins.iter().find(|b| b.id == id).map(|b| b.factory)
    }

    pub(super) fn contributed(&self, id: &str) -> Option<&Arc<dyn GovernancePolicy>> {
        self.contributed
            .iter()
            .find(|c| c.instance.id().as_str() == id)
            .map(|c| &c.instance)
    }

    pub(super) const fn builtins(&self) -> &'static [BuiltinPolicy] {
        self.builtins
    }

    pub(super) fn contributed_instances(&self) -> impl Iterator<Item = &Arc<dyn GovernancePolicy>> {
        self.contributed.iter().map(|c| &c.instance)
    }
}
