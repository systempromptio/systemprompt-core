//! Identity policy applied to every browser-redirect SSO sign-in.
//!
//! A provider validates the `IdP`'s answer and yields an [`ExternalIdentity`];
//! the policy then decides whether that identity may sign in here and with
//! which roles. The checks run in a fixed order — email domain, group
//! overage, group requirement — so the first refusal is always the same one
//! for the same input, and the audit trail stays comparable across providers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::ExternalIdentity;

fn default_roles() -> Vec<String> {
    vec!["user".to_owned()]
}

/// Who may sign in through a provider, and with which roles.
///
/// `link_by_email` (default `false`) lets an identity with no federated link
/// claim the active, interactive account that already owns its verified
/// email, linking the two instead of provisioning a duplicate.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SsoIdentityPolicy {
    #[serde(default)]
    pub allowed_email_domains: Vec<String>,
    #[serde(default)]
    pub auto_provision: bool,
    #[serde(default)]
    pub link_by_email: bool,
    #[serde(default)]
    pub deny_without_group: bool,
    #[serde(default)]
    pub group_roles: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub group_role_patterns: BTreeMap<String, Vec<String>>,
    #[serde(default = "default_roles")]
    pub default_roles: Vec<String>,
}

impl Default for SsoIdentityPolicy {
    fn default() -> Self {
        Self {
            allowed_email_domains: Vec::new(),
            auto_provision: false,
            link_by_email: false,
            deny_without_group: false,
            group_roles: BTreeMap::new(),
            group_role_patterns: BTreeMap::new(),
            default_roles: default_roles(),
        }
    }
}

/// Why the policy refused an otherwise valid identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SsoRefusal {
    DomainNotAllowed,
    GroupOverage,
    NoGroup,
    NotProvisioned,
}

/// An identity the policy admitted, with the roles it resolved to.
///
/// Once the driver has resolved the local user, `roles` is replaced by the set the
/// registered role resolver settled on, so the session and the success sink
/// both see the roles actually granted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatedIdentity {
    pub external_sub: String,
    pub email: String,
    pub display_name: String,
    pub groups: Vec<String>,
    pub roles: Vec<String>,
}

impl SsoIdentityPolicy {
    #[must_use]
    pub fn email_allowed(&self, email: &str) -> bool {
        if self.allowed_email_domains.is_empty() {
            return true;
        }
        let Some((_, domain)) = email.rsplit_once('@') else {
            return false;
        };
        self.allowed_email_domains
            .iter()
            .any(|allowed| allowed.trim_start_matches('@').eq_ignore_ascii_case(domain))
    }

    #[must_use]
    pub fn roles_for_groups(&self, groups: &[String]) -> Vec<String> {
        let mut roles: Vec<String> = Vec::new();
        let mut push = |granted: &[String]| {
            for role in granted {
                if !roles.contains(role) {
                    roles.push(role.clone());
                }
            }
        };
        for group in groups {
            if let Some(granted) = self.group_roles.get(group) {
                push(granted);
            }
        }
        for group in groups {
            for (pattern, granted) in &self.group_role_patterns {
                if glob_matches(pattern, group) {
                    push(granted);
                }
            }
        }
        roles
    }

    pub fn gate(
        &self,
        identity: &ExternalIdentity,
        email: &str,
    ) -> Result<GatedIdentity, SsoRefusal> {
        if !self.email_allowed(email) {
            return Err(SsoRefusal::DomainNotAllowed);
        }
        if identity.groups_overage {
            return Err(SsoRefusal::GroupOverage);
        }
        if identity.groups.is_empty() && self.deny_without_group {
            return Err(SsoRefusal::NoGroup);
        }
        let mut roles = self.roles_for_groups(&identity.groups);
        if roles.is_empty() {
            roles.clone_from(&self.default_roles);
        }
        let display_name = identity
            .display_name
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map_or_else(|| email.to_owned(), str::to_owned);
        Ok(GatedIdentity {
            external_sub: identity.subject.clone(),
            email: email.to_owned(),
            display_name,
            groups: identity.groups.clone(),
            roles,
        })
    }
}

fn glob_matches(pattern: &str, value: &str) -> bool {
    match pattern.split_once('*') {
        None => pattern == value,
        Some((prefix, suffix)) => {
            value.len() >= prefix.len() + suffix.len()
                && value.starts_with(prefix)
                && value.ends_with(suffix)
        },
    }
}
