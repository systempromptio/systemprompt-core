//! The gateway's inference catalog as the webview receives it: providers
//! grouped with their models, how fresh the catalog is, and the Claude-only
//! subset Claude Desktop is given.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;

use crate::gateway::model_view::{CatalogGroup, desktop_models};

/// One provider of the catalog. `configured` false means the gateway lists it
/// but cannot serve it; `config_issue` says why.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub struct CatalogProviderPayload {
    pub name: String,
    pub surface: &'static str,
    pub configured: bool,
    pub models: Vec<String>,
    pub config_issue: Option<String>,
}

impl From<CatalogGroup> for CatalogProviderPayload {
    fn from(group: CatalogGroup) -> Self {
        Self {
            name: group.name,
            surface: group.surface.as_tag(),
            configured: group.configured,
            models: group.models,
            config_issue: group.config_issue,
        }
    }
}

/// How far the catalog can be trusted. `stale` keeps the last good providers
/// and carries the error that stopped the latest refresh.
#[derive(Debug, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub enum CatalogFreshnessPayload {
    Unknown,
    Fresh,
    Stale { since_unix: u64, error: String },
}

/// The one model catalog every view renders. `total` counts the distinct
/// models of configured providers; `desktop_models` is the subset Claude
/// Desktop is given.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-export", ts(export, export_to = "web/js/types/"))]
pub struct InferenceCatalogPayload {
    pub providers: Vec<CatalogProviderPayload>,
    pub total: usize,
    pub desktop_models: Vec<String>,
    pub freshness: CatalogFreshnessPayload,
    pub fetched_at_unix: Option<u64>,
}

impl InferenceCatalogPayload {
    #[must_use]
    pub fn new(
        groups: Vec<CatalogGroup>,
        freshness: CatalogFreshnessPayload,
        fetched_at_unix: Option<u64>,
    ) -> Self {
        let served: Vec<String> = groups
            .iter()
            .filter(|g| g.configured)
            .flat_map(|g| g.models.iter().cloned())
            .collect();
        Self {
            total: served.len(),
            desktop_models: desktop_models(&served),
            providers: groups
                .into_iter()
                .map(CatalogProviderPayload::from)
                .collect(),
            freshness,
            fetched_at_unix,
        }
    }
}
