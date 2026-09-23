#[cfg(test)]
mod bundle;
#[cfg(test)]
mod bundle_node;
#[cfg(test)]
mod bundle_rules;
#[cfg(test)]
mod candidate;
#[cfg(test)]
mod catalog;
#[cfg(test)]
mod catalog_batch;
#[cfg(test)]
mod catalog_rules;
#[cfg(test)]
mod errors;
#[cfg(test)]
mod helpers;
#[cfg(test)]
mod import_edges;
#[cfg(test)]
mod import_manifest_shapes;
#[cfg(test)]
mod import_round_trip;
#[cfg(test)]
mod import_strict;
#[cfg(test)]
mod import_tree;
#[cfg(test)]
mod import_warnings;
#[cfg(test)]
mod keep;
#[cfg(test)]
mod managed_import_lifecycle;
#[cfg(test)]
mod managed_repository_integrity;
#[cfg(test)]
mod managed_resolution;
#[cfg(test)]
mod manifest;
#[cfg(test)]
mod registry;
#[cfg(test)]
mod resolved_cache;
#[cfg(test)]
mod scope;
#[cfg(test)]
mod service;
#[cfg(test)]
mod trace;
#[cfg(test)]
mod view;

#[cfg(test)]
use async_trait::async_trait;
#[cfg(test)]
use systemprompt_identifiers::UserId;
#[cfg(test)]
use systemprompt_marketplace::{
    AllowAllFilter, MarketplaceCandidate, MarketplaceFilter, MarketplaceFilterError,
};
#[cfg(test)]
use systemprompt_models::bridge::ids::{PluginId, Sha256Digest};
#[cfg(test)]
use systemprompt_models::bridge::manifest::PluginEntry;
#[cfg(test)]
use systemprompt_test_fixtures::fixture_user_id;

#[cfg(test)]
fn plugin(id: &str) -> PluginEntry {
    PluginEntry {
        id: PluginId::try_new(id).expect("non-empty id"),
        version: "0.0.1".into(),
        sha256: Sha256Digest::try_new(
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .expect("zero digest is valid hex"),
        files: vec![],
        hooks: Default::default(),
    }
}

#[cfg(test)]
fn sample_candidate() -> MarketplaceCandidate {
    MarketplaceCandidate {
        plugins: vec![plugin("alpha"), plugin("beta")],
        ..MarketplaceCandidate::default()
    }
}

#[tokio::test]
async fn allow_all_filter_returns_input_unchanged() {
    let filter = AllowAllFilter;
    let user = fixture_user_id();
    let before = sample_candidate();
    let after = filter
        .filter(&user, before.clone())
        .await
        .expect("AllowAllFilter must never error");
    assert_eq!(after.plugins.len(), before.plugins.len());
    assert_eq!(
        after
            .plugins
            .iter()
            .map(|p| p.id.as_str())
            .collect::<Vec<_>>(),
        before
            .plugins
            .iter()
            .map(|p| p.id.as_str())
            .collect::<Vec<_>>(),
    );
}

#[tokio::test]
async fn empty_candidate_round_trips() {
    let filter = AllowAllFilter;
    let user = fixture_user_id();
    let after = filter
        .filter(&user, MarketplaceCandidate::default())
        .await
        .expect("filter on empty candidate must succeed");
    assert!(after.is_empty());
}

#[cfg(test)]
#[derive(Debug)]
struct DropAllFilter;

#[cfg(test)]
#[async_trait]
impl MarketplaceFilter for DropAllFilter {
    async fn filter(
        &self,
        _user: &UserId,
        _candidate: MarketplaceCandidate,
    ) -> Result<MarketplaceCandidate, MarketplaceFilterError> {
        Ok(MarketplaceCandidate::default())
    }
}

#[tokio::test]
async fn custom_filter_can_drop_everything() {
    let filter = DropAllFilter;
    let user = fixture_user_id();
    let after = filter
        .filter(&user, sample_candidate())
        .await
        .expect("custom filter ok");
    assert!(after.plugins.is_empty());
    assert!(after.managed_mcp_servers.is_empty());
}

#[tokio::test]
async fn errors_propagate() {
    #[derive(Debug)]
    struct Failing;

    #[async_trait]
    impl MarketplaceFilter for Failing {
        async fn filter(
            &self,
            _user: &UserId,
            _candidate: MarketplaceCandidate,
        ) -> Result<MarketplaceCandidate, MarketplaceFilterError> {
            Err(MarketplaceFilterError::Backend("offline".into()))
        }
    }

    let result = Failing.filter(&fixture_user_id(), sample_candidate()).await;
    assert!(matches!(result, Err(MarketplaceFilterError::Backend(_))));
}

#[cfg(test)]
mod consumer_evidence;
#[cfg(test)]
mod consumer_fixture;

#[cfg(test)]
mod git_execution;

#[cfg(test)]
mod git_https;

#[cfg(test)]
mod inventory;

#[cfg(test)]
mod consumer_plan;

#[cfg(test)]
mod installation_coverage;

#[cfg(test)]
mod organization_resolution;

#[cfg(test)]
mod publication_transaction_fault;
#[cfg(test)]
mod reconciliation_persistence;
#[cfg(test)]
mod retained_distribution;
#[cfg(test)]
mod text_normalization;

#[cfg(test)]
mod authoring_capture;
#[cfg(test)]
mod source_sync_failure_recovery;
#[cfg(test)]
mod source_sync_fixture;
#[cfg(test)]
mod source_sync_regressions;

#[cfg(test)]
mod managed_authoring_lifecycle;
#[cfg(test)]
mod managed_withdrawal_lifecycle;
