//! Gateway-route entity catalog materialization.
//!
//! Gateway route ids are content-addressed (`synthesize_route_id` in
//! `systemprompt_models`), so they are stable across installs but change the
//! moment a route's pattern or provider changes. The resolver is exact-match
//! and fail-closed: a route id with no `access_control_entities` row resolves
//! to [`DenyReason::UnknownEntity`](crate::authz::DenyReason::UnknownEntity)
//! before any rule runs.
//!
//! [`reconcile_gateway_entities`] projects the live profile's routes into the
//! entity catalog so the resolver can see them. Boot and the `admin config`
//! CLI both call it, keeping the catalog in step with the profile whether the
//! operator edited a route or simply started the app. Entities are registered
//! `default_included = false`: presence in the catalog never grants access on
//! its own — an explicit, role-scoped grant in `access_control_rules` still
//! has to allow the route.

use super::repository::AccessControlRepository;
use super::types::EntityKind;
use crate::authz::error::AuthzResult;

pub async fn reconcile_gateway_entities(
    repo: &AccessControlRepository,
    route_ids: &[&str],
    source: &str,
) -> AuthzResult<usize> {
    repo.upsert_entities(EntityKind::GatewayRoute, route_ids, false, source)
        .await?;
    Ok(route_ids.len())
}
