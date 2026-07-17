//! JSON projections of marketplace config for the HTTP catalogue endpoints.
//!
//! These are outgoing-only, fixed-shape API bodies, so `serde_json::Value`
//! is the idiomatic construction form here (protocol boundary).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::MarketplaceId;
use systemprompt_models::services::MarketplaceConfig;

#[must_use]
pub fn render_marketplace_json(id: &str, marketplace: &MarketplaceConfig) -> serde_json::Value {
    let plugin_entries: Vec<serde_json::Value> = marketplace
        .plugins
        .include
        .iter()
        .map(|plugin_id| {
            serde_json::json!({
                "name": plugin_id,
                "source": format!("./storage/files/plugins/{plugin_id}"),
            })
        })
        .collect();

    serde_json::json!({
        "name": id,
        "owner": { "name": marketplace.author.name.clone() },
        "metadata": {
            "description": marketplace.description.clone(),
            "version": marketplace.version.clone(),
        },
        "plugins": plugin_entries,
    })
}

#[must_use]
pub fn render_marketplace_list<'a, I>(marketplaces: I) -> serde_json::Value
where
    I: IntoIterator<Item = (&'a MarketplaceId, &'a MarketplaceConfig)>,
{
    let entries: Vec<serde_json::Value> = marketplaces
        .into_iter()
        .map(|(id, m)| {
            serde_json::json!({
                "id": id.as_str(),
                "name": m.name,
                "description": m.description,
                "version": m.version,
                "visibility": m.visibility,
                "enabled": m.enabled,
            })
        })
        .collect();

    serde_json::json!({ "marketplaces": entries })
}
