use crate::models::builders::content::CategoryIdUpdate;
use crate::models::{Content, ContentKind, CreateContentParams, UpdateContentParams};
use chrono::Utc;
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_identifiers::{CategoryId, ContentId, LocaleCode, SourceId};

use super::queries;

pub(super) async fn create(
    pool: &Arc<PgPool>,
    params: &CreateContentParams,
) -> Result<Content, sqlx::Error> {
    let id = ContentId::new(uuid::Uuid::new_v4().to_string());
    let now = Utc::now();
    sqlx::query_as!(
        Content,
        r#"
        INSERT INTO markdown_content (
            id, slug, locale, title, description, body, author,
            published_at, keywords, kind, image, category_id, source_id,
            version_hash, links, updated_at, public
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
        ON CONFLICT (slug, locale) DO UPDATE SET
            title = EXCLUDED.title,
            description = EXCLUDED.description,
            body = EXCLUDED.body,
            author = EXCLUDED.author,
            published_at = EXCLUDED.published_at,
            keywords = EXCLUDED.keywords,
            kind = EXCLUDED.kind,
            image = EXCLUDED.image,
            category_id = EXCLUDED.category_id,
            source_id = EXCLUDED.source_id,
            version_hash = EXCLUDED.version_hash,
            links = EXCLUDED.links,
            updated_at = EXCLUDED.updated_at,
            public = EXCLUDED.public
        RETURNING id as "id: ContentId", slug,
                  locale as "locale: LocaleCode",
                  title, description, body, author,
                  published_at, keywords, kind, image,
                  category_id as "category_id: CategoryId",
                  source_id as "source_id: SourceId",
                  version_hash, public, COALESCE(links, '[]'::jsonb) as "links!",
                  updated_at
        "#,
        id.as_str(),
        params.slug,
        params.locale.as_str(),
        params.title,
        params.description,
        params.body,
        params.author,
        params.published_at,
        params.keywords,
        params.kind.as_str(),
        params.image,
        params.category_id.as_ref().map(CategoryId::as_str),
        params.source_id.as_str(),
        params.version_hash,
        params.links,
        now,
        params.public
    )
    .fetch_one(&**pool)
    .await
}

pub(super) async fn update(
    pool: &Arc<PgPool>,
    params: &UpdateContentParams,
) -> Result<Content, sqlx::Error> {
    let now = Utc::now();

    let current = queries::get_by_id(pool, &params.id).await?;
    let current_ref = current.as_ref();

    let category_id_value: Option<String> = match &params.category_id {
        CategoryIdUpdate::Set(cat) => Some(cat.as_str().to_owned()),
        CategoryIdUpdate::Clear => None,
        CategoryIdUpdate::Unchanged => {
            current_ref.and_then(|c| c.category_id.as_ref().map(|cat| cat.as_str().to_owned()))
        },
    };

    let kind_value: String = params.kind.clone().unwrap_or_else(|| {
        current_ref.map_or_else(
            || ContentKind::Article.as_str().to_owned(),
            |c| c.kind.clone(),
        )
    });

    let public_value: bool = params
        .public
        .unwrap_or_else(|| current_ref.is_some_and(|c| c.public));

    let author_value: String = params
        .author
        .clone()
        .unwrap_or_else(|| current_ref.map_or_else(String::new, |c| c.author.clone()));

    let published_at_value = params
        .published_at
        .unwrap_or_else(|| current_ref.map_or_else(Utc::now, |c| c.published_at));

    let links_value = params.links.clone().unwrap_or_else(|| {
        current_ref.map_or_else(|| serde_json::Value::Array(vec![]), |c| c.links.clone())
    });

    sqlx::query_as!(
        Content,
        r#"
        UPDATE markdown_content
        SET title = $1, description = $2, body = $3, keywords = $4,
            image = $5, version_hash = $6, updated_at = $7,
            category_id = $8, kind = $9, public = $10,
            author = $11, published_at = $12, links = $13
        WHERE id = $14
        RETURNING id as "id: ContentId", slug,
                  locale as "locale: LocaleCode",
                  title, description, body, author,
                  published_at, keywords, kind, image,
                  category_id as "category_id: CategoryId",
                  source_id as "source_id: SourceId",
                  version_hash, public, COALESCE(links, '[]'::jsonb) as "links!",
                  updated_at
        "#,
        params.title,
        params.description,
        params.body,
        params.keywords,
        params.image,
        params.version_hash,
        now,
        category_id_value,
        kind_value,
        public_value,
        author_value,
        published_at_value,
        links_value,
        params.id.as_str()
    )
    .fetch_one(&**pool)
    .await
}

pub(super) async fn delete(pool: &Arc<PgPool>, id: &ContentId) -> Result<(), sqlx::Error> {
    sqlx::query!("DELETE FROM markdown_content WHERE id = $1", id.as_str())
        .execute(&**pool)
        .await?;
    Ok(())
}

pub(super) async fn delete_by_source(
    pool: &Arc<PgPool>,
    source_id: &SourceId,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query!(
        "DELETE FROM markdown_content WHERE source_id = $1",
        source_id.as_str()
    )
    .execute(&**pool)
    .await?;
    Ok(result.rows_affected())
}
