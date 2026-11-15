//! The prerender's popularity ordering without a database: no ranking
//! provider and an unnamed source both render unranked as typed outcomes, a
//! provider's ranking is used in its order, and a provider failure is the
//! typed `PublishError::Popularity`.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_generator::{PopularContent, PublishError, fetch_popular_ids};
use systemprompt_identifiers::{ContentId, SourceId};
use systemprompt_traits::{ContentPopularity, DynContentPopularity, RepositoryError};

#[derive(Debug)]
struct Ranked(Vec<&'static str>);

#[async_trait]
impl ContentPopularity for Ranked {
    async fn popular_content_ids(
        &self,
        _source_id: &SourceId,
        _days: i32,
        _limit: i64,
    ) -> Result<Vec<ContentId>, RepositoryError> {
        Ok(self.0.iter().map(|id| ContentId::new(*id)).collect())
    }
}

#[derive(Debug)]
struct Failing;

#[async_trait]
impl ContentPopularity for Failing {
    async fn popular_content_ids(
        &self,
        _source_id: &SourceId,
        _days: i32,
        _limit: i64,
    ) -> Result<Vec<ContentId>, RepositoryError> {
        Err(RepositoryError::InvalidArgument {
            field: "source_id",
            reason: "refused".to_owned(),
        })
    }
}

fn source() -> SourceId {
    SourceId::new("blog")
}

#[tokio::test]
async fn without_a_ranking_provider_the_source_renders_unranked() {
    let popular = fetch_popular_ids(None, "blog", &source())
        .await
        .expect("an absent provider is not a failure");
    assert_eq!(popular, PopularContent::NoRankingProvider);
    assert!(popular.ids().is_empty());
}

#[tokio::test]
async fn an_unnamed_source_is_never_ranked() {
    let provider: DynContentPopularity = Arc::new(Ranked(vec!["a"]));
    let popular = fetch_popular_ids(Some(&provider), "", &source())
        .await
        .expect("unnamed source");
    assert_eq!(popular, PopularContent::UnnamedSource);
}

#[tokio::test]
async fn a_provided_ranking_is_used_in_its_order() {
    let provider: DynContentPopularity = Arc::new(Ranked(vec!["b", "a"]));
    let popular = fetch_popular_ids(Some(&provider), "blog", &source())
        .await
        .expect("ranking");
    assert_eq!(popular.ids(), ["b".to_owned(), "a".to_owned()]);
}

#[tokio::test]
async fn a_provider_failure_is_a_typed_popularity_error() {
    let provider: DynContentPopularity = Arc::new(Failing);
    let err = fetch_popular_ids(Some(&provider), "blog", &source())
        .await
        .expect_err("provider failure");
    assert!(matches!(err, PublishError::Popularity { .. }), "{err:?}");
}
