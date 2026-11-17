//! The analytics extension provides the content audience ranking the
//! content prerender reads from the extension state, and the provided ranking
//! is the analytics report query.

use systemprompt_analytics::ContentAnalyticsRepository;
use systemprompt_extension::ExtensionContext;
use systemprompt_identifiers::SourceId;
use systemprompt_test_fixtures::{ensure_test_bootstrap, test_app_context, test_db_pool};
use systemprompt_traits::DynContentPopularity;

#[tokio::test]
async fn the_extension_state_carries_the_analytics_popularity_ranking() -> anyhow::Result<()> {
    let pool = test_db_pool().await;
    let ctx = test_app_context(&ensure_test_bootstrap().database_url).await;
    let state = ctx.extension_state();
    let popularity = state
        .find::<DynContentPopularity>()
        .expect("the analytics extension provides the ranking");

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
    let direct: Vec<_> = ContentAnalyticsRepository::new(&pool)
        .popular_content(&source, 30, 5)
        .await?
        .into_iter()
        .map(|row| row.id)
        .collect();
    assert_eq!(provided.len(), 1, "{provided:?}");
    assert_eq!(provided, direct);
    Ok(())
}
