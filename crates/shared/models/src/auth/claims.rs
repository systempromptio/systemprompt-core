//! Self-issued JWT claim shapes and the RFC 8693 delegation chain.
//!
//! [`JwtClaims`] is the canonical token payload; its scope/roles/attributes
//! fields are the transport for the platform's three authorization layers
//! (PBAC, RBAC, ABAC). [`ActClaim`] models the recursive `act` delegation
//! chain, capped at [`MAX_ACT_CHAIN_DEPTH`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::str::FromStr;
use systemprompt_identifiers::{ClientId, SessionId, UserId};

use super::{
    JwtAudience, Permission, RateLimitTier, TokenType, UserType, parse_permissions,
    permissions_to_string,
};
use systemprompt_identifiers::Actor;

/// RFC 8693 §4.1 actor (`act`) claim.
///
/// Captures the immediate actor (`iss` + `sub`) that requested a token
/// exchange and a recursive `act` link to its own delegating actor. The
/// chain is walked outermost-first by [`ActClaim::flatten_to_chain`]:
/// the outermost `JwtClaims.act` is the most recent delegate, and each
/// nested `act` is the actor that delegated to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActClaim {
    pub iss: String,
    pub sub: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub act: Box<Option<Self>>,
}

/// Maximum delegation-chain depth accepted by the platform. RFC 8693
/// does not bound `act` recursion; the cap protects audit storage and
/// makes "ever-growing delegation lineage" rejectable at decode time.
pub const MAX_ACT_CHAIN_DEPTH: usize = 16;

impl ActClaim {
    /// Walk the `act` chain and return reconstructed [`Actor`] values in
    /// outermost-first order: index 0 is the most recent delegate
    /// (i.e. `self`), and the last element is the original delegating
    /// principal. The chain is truncated at [`MAX_ACT_CHAIN_DEPTH`].
    ///
    /// Every link is reconstructed as [`Actor::user`] with the `sub`
    /// claim as the [`UserId`]; `ActorKind` cannot be discerned from a
    /// bare RFC 8693 `act` chain, so we default to `User`.
    #[must_use]
    pub fn flatten_to_chain(&self) -> Vec<Actor> {
        let mut chain = Vec::new();
        let mut cursor = Some(self);
        while let Some(node) = cursor {
            if chain.len() >= MAX_ACT_CHAIN_DEPTH {
                break;
            }
            chain.push(Actor::user(UserId::new(node.sub.clone())));
            cursor = node.act.as_ref().as_ref();
        }
        chain
    }

    /// Count nodes in the delegation chain without allocating. Returns
    /// `MAX_ACT_CHAIN_DEPTH + 1` if the chain exceeds the cap so callers
    /// can short-circuit with a single comparison.
    #[must_use]
    pub fn depth(&self) -> usize {
        let mut depth = 0usize;
        let mut cursor = Some(self);
        while let Some(node) = cursor {
            depth += 1;
            if depth > MAX_ACT_CHAIN_DEPTH {
                return depth;
            }
            cursor = node.act.as_ref().as_ref();
        }
        depth
    }
}

/// Self-issued JWT claim shape.
///
/// Fields are grouped by who consumes them downstream. The platform runs
/// authorization in three layers (see `internal/guides/authz.md`); each
/// attribute field below is the transport for one of those layers. Do not
/// delete a field without first removing its readers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    pub sub: String,
    pub iat: i64,
    pub exp: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nbf: Option<i64>,
    pub iss: String,
    #[serde(
        serialize_with = "serialize_audiences",
        deserialize_with = "deserialize_audiences"
    )]
    pub aud: Vec<JwtAudience>,
    pub jti: String,

    /// PBAC scope: the `Permission` set this principal is granted. Enforced
    /// at every route via `with_auth(scope)`; the first element is the
    /// privilege level used by [`UserType::from_permissions`].
    #[serde(
        serialize_with = "serialize_scope",
        deserialize_with = "deserialize_scope"
    )]
    pub scope: Vec<Permission>,

    pub username: String,
    pub email: String,
    pub user_type: UserType,

    /// RBAC attribute: role strings minted from the user row at OAuth
    /// issuance. Consumed by `authz::resolver::resolve` for the core RBAC
    /// check against `access_control_rules` (`rule_type = role`) and
    /// forwarded into every `AuthzDecisionHook::evaluate` call. The only
    /// first-class identity vector core inspects; everything else is
    /// extension-defined via [`attributes`](Self::attributes).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roles: Vec<String>,

    /// Opaque ABAC attribute bag. The token issuer mints whatever
    /// namespaced facts the extension authz hook needs (e.g.
    /// `"acme.desk": "fixed-income"`, `"boeing.clearance": "ts/sci"`).
    /// Core never interprets values — keys SHOULD be dotted-namespaced.
    /// Forwarded verbatim into `AuthzRequest.attributes`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attributes: BTreeMap<String, serde_json::Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<ClientId>,
    pub token_type: TokenType,
    pub auth_time: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<SessionId>,

    /// Rate-limit attribute: minted from `UserType::rate_tier` so the tier is
    /// committed at issuance rather than re-derived per request. Read at the
    /// rate-limit middleware via `RequestContext::rate_limit_tier()`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit_tier: Option<RateLimitTier>,

    /// Hook attribute: the plugin id this token was minted for. Validated by
    /// the hook-token validator in `systemprompt_security::auth::hook_token` —
    /// a hook request whose `plugin_id` claim disagrees with the URL path's
    /// `plugin_id` is rejected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_id: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub act: Option<ActClaim>,
}

fn serialize_audiences<S>(auds: &[JwtAudience], s: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::ser::SerializeSeq;
    let mut seq = s.serialize_seq(Some(auds.len()))?;
    for aud in auds {
        seq.serialize_element(aud.as_str())?;
    }
    seq.end()
}

fn deserialize_audiences<'de, D>(d: D) -> Result<Vec<JwtAudience>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let strings: Vec<String> = Vec::deserialize(d)?;
    strings
        .iter()
        .map(|s| JwtAudience::from_str(s).map_err(serde::de::Error::custom))
        .collect()
}

fn serialize_scope<S>(permissions: &[Permission], s: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    s.serialize_str(&permissions_to_string(permissions))
}

fn deserialize_scope<'de, D>(d: D) -> Result<Vec<Permission>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let scope_string: String = String::deserialize(d)?;
    parse_permissions(&scope_string).map_err(serde::de::Error::custom)
}

impl JwtClaims {
    pub fn has_permission(&self, permission: Permission) -> bool {
        self.scope.contains(&permission)
    }

    pub fn permissions(&self) -> &[Permission] {
        &self.scope
    }

    pub fn get_permissions(&self) -> Vec<Permission> {
        self.scope.clone()
    }

    pub fn get_scopes(&self) -> Vec<String> {
        self.scope.iter().map(ToString::to_string).collect()
    }

    pub fn is_admin(&self) -> bool {
        self.has_permission(Permission::Admin)
    }

    pub fn is_registered_user(&self) -> bool {
        self.has_permission(Permission::User)
    }

    pub fn is_anonymous(&self) -> bool {
        self.has_permission(Permission::Anonymous)
    }

    pub fn has_audience(&self, aud: &JwtAudience) -> bool {
        self.aud.contains(aud)
    }

    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|r| r == role)
    }

    pub fn roles(&self) -> &[String] {
        &self.roles
    }
}
