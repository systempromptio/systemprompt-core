//! The messaging route shapes: Slack's and Teams' handler-verified routes,
//! each present only with its feature and empty without it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg(feature = "slack")]
pub(super) mod slack {
    use super::super::shapes::{AGENTS_RATE, limits, shape};
    use super::super::{CoreId, PrivilegedShape};
    use crate::authz::{AuthzPolicy, HandlerVerifier};
    use crate::protection::{ContextKind, RouteGroup};
    use crate::router::{Allowance, ApiBase, BasePath, BaseRef};

    pub(in crate::reserved) const SHAPES: &[PrivilegedShape] = &[PrivilegedShape {
        owner: CoreId::Slack,
        base: BaseRef::Path(BasePath::Api(ApiBase::Slack)),
        group: RouteGroup::Messaging,
        allowance: Allowance::HandlerVerified(HandlerVerifier::SlackSignature),
        shape: shape(
            AuthzPolicy::handler_verified(HandlerVerifier::SlackSignature),
            ContextKind::HandlerResolved,
            limits(AGENTS_RATE),
        ),
        routes: None,
    }];
}

#[cfg(not(feature = "slack"))]
pub(super) mod slack {
    pub(in crate::reserved) const SHAPES: &[super::super::PrivilegedShape] = &[];
}

#[cfg(feature = "teams")]
pub(super) mod teams {
    use super::super::shapes::{AGENTS_RATE, limits, shape};
    use super::super::{CoreId, PrivilegedShape};
    use crate::authz::{AuthzPolicy, HandlerVerifier};
    use crate::protection::{ContextKind, RouteGroup};
    use crate::router::{Allowance, ApiBase, BasePath, BaseRef};

    pub(in crate::reserved) const SHAPES: &[PrivilegedShape] = &[PrivilegedShape {
        owner: CoreId::Teams,
        base: BaseRef::Path(BasePath::Api(ApiBase::Teams)),
        group: RouteGroup::Messaging,
        allowance: Allowance::HandlerVerified(HandlerVerifier::TeamsBotToken),
        shape: shape(
            AuthzPolicy::handler_verified(HandlerVerifier::TeamsBotToken),
            ContextKind::HandlerResolved,
            limits(AGENTS_RATE),
        ),
        routes: None,
    }];
}

#[cfg(not(feature = "teams"))]
pub(super) mod teams {
    pub(in crate::reserved) const SHAPES: &[super::super::PrivilegedShape] = &[];
}
