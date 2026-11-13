//! Static, compile-time-declared per-route authorization tier.
//!
//! Every mounted route group names an [`AuthzPolicy`]: the set of caller
//! types (`UserType`) that may reach it. The type is data only; the host
//! server enforces it with its authorization gate after its context
//! middleware has built the request's `RequestContext`. A route declaration
//! cannot be constructed without a policy, so a route group cannot be
//! mounted authenticated-but-unauthorized.
//!
//! This is a COARSE gate — "may this kind of caller reach this route group".
//! It does NOT replace per-resource ownership checks (e.g. "does this user own
//! task X"); those remain the handler/repository layer's responsibility.
//!
//! `UserType::Anon` is a real, reachable principal — it is NOT true that every
//! request carries a human user. An anonymous token is minted by
//! `POST /oauth/session` and admitted only by [`AuthzPolicy::public`]. The
//! public surface deliberately includes a few unauthenticated writes: the
//! OAuth auth-establishment endpoints (token / authorize / webauthn, each
//! gated by its own protocol) and append-only engagement telemetry ingestion.
//! Every other public route is read-only; any new public-group handler that
//! mutates state must enforce its own per-resource ownership check, because
//! the gate will admit `Anon`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::auth::UserType;

/// The caller types a route group admits.
#[derive(Clone, Copy, Debug)]
pub struct AuthzPolicy {
    allowed: &'static [UserType],
}

impl AuthzPolicy {
    #[must_use]
    pub const fn public() -> Self {
        Self {
            allowed: &[
                UserType::Anon,
                UserType::User,
                UserType::Admin,
                UserType::A2a,
                UserType::Mcp,
                UserType::Service,
            ],
        }
    }

    #[must_use]
    pub const fn authenticated() -> Self {
        Self {
            allowed: &[
                UserType::User,
                UserType::Admin,
                UserType::A2a,
                UserType::Mcp,
                UserType::Service,
            ],
        }
    }

    #[must_use]
    pub const fn user() -> Self {
        Self {
            allowed: &[UserType::User, UserType::Admin],
        }
    }

    #[must_use]
    pub const fn admin() -> Self {
        Self {
            allowed: &[UserType::Admin],
        }
    }

    #[must_use]
    pub const fn restricted_to(allowed: &'static [UserType]) -> Self {
        Self { allowed }
    }

    // Why: MCP OAuth discovery needs the handler's WWW-Authenticate challenge, not
    // a generic 403.
    #[must_use]
    pub const fn deferred_to_handler() -> Self {
        Self::public()
    }

    #[must_use]
    pub fn permits(self, user_type: UserType) -> bool {
        self.allowed.contains(&user_type)
    }

    #[must_use]
    pub const fn allowed(self) -> &'static [UserType] {
        self.allowed
    }
}
