//! The content extension provides the content audience ranking the content
//! prerender reads from the extension state, and the content-audience report
//! port the analytics content commands read; the provided ranking orders the
//! report port's rows of the source.

use systemprompt_content::ContentAudienceReportRepository;
use systemprompt_identifiers::SourceId;
use systemprompt_test_fixtures::{ensure_test_bootstrap, test_app_context, test_db_pool};
use systemprompt_traits::DynContentPopularity;
use systemprompt_traits::usage_reports::content::{
    ContentAudienceReport, DynContentAudienceReport,
};

#[tokio::test]
async fn the_extension_state_carries_the_content_popularity_ranking() -> anyhow::Result<()> {
    let pool = test_db_pool().await;
    let ctx = test_app_context(&ensure_test_bootstrap().database_url).await;
    let state = ctx.kernel().state().clone();
    let popularity = state
        .find::<DynContentPopularity>()
        .expect("the content extension provides the ranking");
    assert!(state.find::<DynContentAudienceReport>().is_some());

    let source = SourceId::new(format!("src-{}", uuid::Uuid::new_v4()));
    let slug = format!("pop-{}", uuid::Uuid::new_v4().simple());
    sqlx::query(
        "INSERT INTO markdown_content (id, slug, title, description, body, author, \
         published_at, keywords, source_id, version_hash) \
         VALUES ($1, $2, 'T', 'D', 'B', 'A', NOW(), 'k', $3, 'h')",
    )
    .bind(format!("mc-{slug}"))
    .bind(&slug)
    .bind(source.as_str())
    .execute(pool.pool().as_ref())
    .await?;

    let provided = popularity.popular_content_ids(&source, 30, 5).await?;
    let direct: Vec<_> = ContentAudienceReportRepository::new(&pool)
        .source_content(&source)
        .await?
        .into_iter()
        .map(|row| row.id)
        .collect();
    assert_eq!(provided.len(), 1, "{provided:?}");
    assert_eq!(provided, direct);
    Ok(())
}

#[tokio::test]
async fn an_unknown_source_has_no_report_content() -> anyhow::Result<()> {
    let pool = test_db_pool().await;
    let source = SourceId::new(format!("src-{}", uuid::Uuid::new_v4()));
    let rows = ContentAudienceReportRepository::new(&pool)
        .source_content(&source)
        .await?;
    assert!(rows.is_empty());
    Ok(())
}
