use axum::extract::{Query, State};
use axum::Json;
use systemprompt_core_database::DatabaseProvider;
use systemprompt_runtime::AppContext;

use super::types::{to_api_error, ApiResult, DatabaseExport, ExportQuery, RecordCounts};

pub async fn export(
    State(ctx): State<AppContext>,
    Query(query): Query<ExportQuery>,
) -> ApiResult<Json<DatabaseExport>> {
    let pool = ctx.db_pool();

    let tables: Vec<&str> = query.tables.as_ref().map_or_else(
        || vec!["services", "skills", "contexts"],
        |t| t.split(',').collect(),
    );

    let include_services = tables.contains(&"services") || tables.contains(&"agents");
    let include_skills = tables.contains(&"skills");
    let include_contexts = tables.contains(&"contexts");

    let services = if include_services {
        export_services(pool.as_ref()).await.map_err(to_api_error)?
    } else {
        vec![]
    };

    let skills = if include_skills {
        export_skills(pool.as_ref()).await.map_err(to_api_error)?
    } else {
        vec![]
    };

    let contexts = if include_contexts {
        export_contexts(pool.as_ref()).await.map_err(to_api_error)?
    } else {
        vec![]
    };

    Ok(Json(DatabaseExport {
        record_counts: RecordCounts {
            services: services.len(),
            skills: skills.len(),
            contexts: contexts.len(),
        },
        services,
        skills,
        contexts,
        exported_at: chrono::Utc::now(),
    }))
}

async fn export_services(pool: &dyn DatabaseProvider) -> anyhow::Result<Vec<serde_json::Value>> {
    let query = "SELECT name, module_name, status, port, created_at, updated_at FROM services \
                 WHERE module_name = 'agent'";
    let rows = pool.fetch_all(&query, &[]).await?;

    Ok(rows
        .into_iter()
        .map(|row| serde_json::Value::Object(row.into_iter().collect()))
        .collect())
}

async fn export_skills(pool: &dyn DatabaseProvider) -> anyhow::Result<Vec<serde_json::Value>> {
    let query = "SELECT skill_id, file_path, name, description, instructions, enabled, tags, \
                 category_id, source_id, created_at, updated_at FROM agent_skills";
    let rows = pool.fetch_all(&query, &[]).await?;

    Ok(rows
        .into_iter()
        .map(|row| serde_json::Value::Object(row.into_iter().collect()))
        .collect())
}

async fn export_contexts(pool: &dyn DatabaseProvider) -> anyhow::Result<Vec<serde_json::Value>> {
    let query =
        "SELECT context_id, user_id, session_id, name, created_at, updated_at FROM user_contexts";
    let rows = pool.fetch_all(&query, &[]).await?;

    Ok(rows
        .into_iter()
        .map(|row| serde_json::Value::Object(row.into_iter().collect()))
        .collect())
}
