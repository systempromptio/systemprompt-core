//! One catalogue resolution per user per sync, through the
//! `DynBridgeCatalogSource` the `managed_resources` member provides.
//!
//! The resolution is memoised per user and shared by the manifest route and
//! every plugin-file download that follows it; a `no-cache` request asks for
//! a fresh one ([`CatalogFreshness::Fresh`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::HeaderMap;
use axum::http::header::CACHE_CONTROL;

use systemprompt_extension::bridge_catalog::{
    BridgeCatalogError, CatalogFreshness, CatalogScope, DynResolvedBridgeCatalog,
};
use systemprompt_identifiers::UserId;
use systemprompt_manifest::services::ServicesConfig;

use super::BridgeRouteState;

// Why: `Cache-Control: no-cache` is the HTTP spelling of "do not answer
// from a store without revalidating"; the bridge sends it on a Sync now.
pub fn freshness_from_headers(headers: &HeaderMap) -> CatalogFreshness {
    let no_cache = headers
        .get_all(CACHE_CONTROL)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .any(|directive| directive.trim().eq_ignore_ascii_case("no-cache"));
    if no_cache {
        CatalogFreshness::Fresh
    } else {
        CatalogFreshness::Memo
    }
}

pub async fn resolve_for_user(
    state: &BridgeRouteState,
    services: &ServicesConfig,
    user_id: &UserId,
    freshness: CatalogFreshness,
) -> Result<DynResolvedBridgeCatalog, BridgeCatalogError> {
    let scope = CatalogScope {
        services,
        services_root: state.settings.services_root(),
        api_external_url: state.settings.api_external_url(),
        owner: state.settings.system_admin(),
    };
    state.catalog.resolve(scope, user_id, freshness).await
}
