use async_trait::async_trait;
use systemprompt_identifiers::UserId;
use systemprompt_marketplace::{
    AllowAllFilter, MarketplaceCandidate, MarketplaceFilter, MarketplaceFilterError,
};
use systemprompt_models::bridge::ids::{PluginId, Sha256Digest};
use systemprompt_models::bridge::manifest::PluginEntry;

fn plugin(id: &str) -> PluginEntry {
    PluginEntry {
        id: PluginId::try_new(id).expect("non-empty id"),
        version: "0.0.1".into(),
        sha256: Sha256Digest::try_new(
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .expect("zero digest is valid hex"),
        files: vec![],
    }
}

fn sample_candidate() -> MarketplaceCandidate {
    MarketplaceCandidate::new(
        vec![plugin("alpha"), plugin("beta")],
        vec![],
        vec![],
        vec![],
    )
}

#[tokio::test]
async fn allow_all_filter_returns_input_unchanged() {
    let filter = AllowAllFilter;
    let user = UserId::new("u-1");
    let before = sample_candidate();
    let after = filter
        .filter(&user, before.clone())
        .await
        .expect("AllowAllFilter must never error");
    assert_eq!(after.plugins.len(), before.plugins.len());
    assert_eq!(
        after.plugins.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
        before.plugins.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
    );
}

#[tokio::test]
async fn empty_candidate_round_trips() {
    let filter = AllowAllFilter;
    let user = UserId::new("u-1");
    let after = filter
        .filter(&user, MarketplaceCandidate::default())
        .await
        .expect("filter on empty candidate must succeed");
    assert!(after.is_empty());
}

#[derive(Debug)]
struct DropAllFilter;

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
    let user = UserId::new("u-1");
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

    let result = Failing.filter(&UserId::new("u-1"), sample_candidate()).await;
    assert!(matches!(result, Err(MarketplaceFilterError::Backend(_))));
}
