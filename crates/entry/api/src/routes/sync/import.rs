use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use systemprompt_core_database::DatabaseProvider;
use systemprompt_runtime::AppContext;

use super::types::{
    to_api_error, ApiResult, DatabaseImportRequest, ExportError, ImportResult, ImportResults,
    TableResult,
};

pub async fn import(
    State(ctx): State<AppContext>,
    Json(request): Json<DatabaseImportRequest>,
) -> ApiResult<Json<ImportResult>> {
    let pool = ctx.db_pool();
    let strategy = validate_merge_strategy(request.merge_strategy.as_deref())?;

    let mut tx = pool.begin_transaction().await.map_err(to_api_error)?;

    let services_result = import_services(&mut *tx, &request.services, strategy)
        .await
        .map_err(to_api_error)?;

    let skills_result = import_skills(&mut *tx, &request.skills, strategy)
        .await
        .map_err(to_api_error)?;

    let contexts_result = import_contexts(&mut *tx, &request.contexts, strategy)
        .await
        .map_err(to_api_error)?;

    tx.commit().await.map_err(to_api_error)?;

    Ok(Json(ImportResult {
        imported_at: chrono::Utc::now(),
        results: ImportResults {
            services: services_result,
            skills: skills_result,
            contexts: contexts_result,
        },
    }))
}

async fn import_services(
    tx: &mut dyn systemprompt_core_database::DatabaseTransaction,
    services: &[serde_json::Value],
    strategy: &str,
) -> anyhow::Result<TableResult> {
    let mut result = TableResult::default();

    if strategy == "replace" {
        let delete_query = "DELETE FROM services WHERE module_name = 'agent'";
        let deleted = tx.execute(&delete_query, &[]).await?;
        result.deleted = deleted as usize;
    }

    for service in services {
        let name = service.get("name").and_then(|v| v.as_str());
        let module_name = service
            .get("module_name")
            .and_then(|v| v.as_str())
            .unwrap_or("agent");
        let port = service
            .get("port")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(8000);

        let Some(name) = name else {
            result.skipped += 1;
            continue;
        };

        let exists_query = "SELECT name FROM services WHERE name = $1";
        let existing = tx.fetch_optional(&exists_query, &[&name]).await?;

        if existing.is_some() && strategy == "skip_existing" {
            result.skipped += 1;
            continue;
        }

        let affected = if existing.is_some() {
            let update_query = "UPDATE services SET module_name = $2, port = $3, updated_at = \
                                NOW() WHERE name = $1";
            tx.execute(&update_query, &[&name, &module_name, &(port as i32)])
                .await?
        } else {
            let insert_query = "INSERT INTO services (name, module_name, port, status) VALUES \
                                ($1, $2, $3, 'stopped')";
            tx.execute(&insert_query, &[&name, &module_name, &(port as i32)])
                .await?
        };

        if affected > 0 {
            if existing.is_some() {
                result.updated += 1;
            } else {
                result.created += 1;
            }
        } else {
            result.skipped += 1;
        }
    }

    Ok(result)
}

async fn import_skills(
    tx: &mut dyn systemprompt_core_database::DatabaseTransaction,
    skills: &[serde_json::Value],
    strategy: &str,
) -> anyhow::Result<TableResult> {
    let mut result = TableResult::default();

    if strategy == "replace" {
        let delete_query = "DELETE FROM agent_skills";
        let deleted = tx.execute(&delete_query, &[]).await?;
        result.deleted = deleted as usize;
    }

    for skill in skills {
        let skill_id = skill.get("skill_id").and_then(|v| v.as_str());
        let file_path = skill.get("file_path").and_then(|v| v.as_str());
        let name = skill.get("name").and_then(|v| v.as_str());
        let description = skill
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let instructions = skill
            .get("instructions")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let enabled = skill
            .get("enabled")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true);
        let source_id = skill
            .get("source_id")
            .and_then(|v| v.as_str())
            .unwrap_or("imported");

        let (Some(skill_id), Some(file_path), Some(name)) = (skill_id, file_path, name) else {
            result.skipped += 1;
            continue;
        };

        let exists_query = "SELECT skill_id FROM agent_skills WHERE skill_id = $1";
        let existing = tx.fetch_optional(&exists_query, &[&skill_id]).await?;

        if existing.is_some() && strategy == "skip_existing" {
            result.skipped += 1;
            continue;
        }

        let affected = if existing.is_some() {
            let update_query = "UPDATE agent_skills SET file_path = $2, name = $3, description = \
                                $4, instructions = $5, enabled = $6, source_id = $7, updated_at = \
                                NOW() WHERE skill_id = $1";
            tx.execute(
                &update_query,
                &[
                    &skill_id,
                    &file_path,
                    &name,
                    &description,
                    &instructions,
                    &enabled,
                    &source_id,
                ],
            )
            .await?
        } else {
            let insert_query = "INSERT INTO agent_skills (skill_id, file_path, name, description, \
                                instructions, enabled, source_id) VALUES ($1, $2, $3, $4, $5, $6, \
                                $7)";
            tx.execute(
                &insert_query,
                &[
                    &skill_id,
                    &file_path,
                    &name,
                    &description,
                    &instructions,
                    &enabled,
                    &source_id,
                ],
            )
            .await?
        };

        if affected > 0 {
            if existing.is_some() {
                result.updated += 1;
            } else {
                result.created += 1;
            }
        } else {
            result.skipped += 1;
        }
    }

    Ok(result)
}

async fn import_contexts(
    tx: &mut dyn systemprompt_core_database::DatabaseTransaction,
    contexts: &[serde_json::Value],
    strategy: &str,
) -> anyhow::Result<TableResult> {
    let mut result = TableResult::default();

    if strategy == "replace" {
        let delete_query = "DELETE FROM user_contexts";
        let deleted = tx.execute(&delete_query, &[]).await?;
        result.deleted = deleted as usize;
    }

    for context in contexts {
        let context_id = context.get("context_id").and_then(|v| v.as_str());
        let user_id = context.get("user_id").and_then(|v| v.as_str());
        let name = context.get("name").and_then(|v| v.as_str()).unwrap_or("");

        let (Some(context_id), Some(user_id)) = (context_id, user_id) else {
            result.skipped += 1;
            continue;
        };

        let exists_query = "SELECT context_id FROM user_contexts WHERE context_id = $1";
        let existing = tx.fetch_optional(&exists_query, &[&context_id]).await?;

        if existing.is_some() && strategy == "skip_existing" {
            result.skipped += 1;
            continue;
        }

        let affected = if existing.is_some() {
            let update_query =
                "UPDATE user_contexts SET name = $2, updated_at = NOW() WHERE context_id = $1";
            tx.execute(&update_query, &[&context_id, &name]).await?
        } else {
            let insert_query =
                "INSERT INTO user_contexts (context_id, user_id, name) VALUES ($1, $2, $3)";
            tx.execute(&insert_query, &[&context_id, &user_id, &name])
                .await?
        };

        if affected > 0 {
            if existing.is_some() {
                result.updated += 1;
            } else {
                result.created += 1;
            }
        } else {
            result.skipped += 1;
        }
    }

    Ok(result)
}

fn validate_merge_strategy(
    strategy: Option<&str>,
) -> Result<&str, (StatusCode, Json<ExportError>)> {
    match strategy {
        Some("replace") => Ok("replace"),
        Some("skip_existing") => Ok("skip_existing"),
        Some("merge") | None => Ok("merge"),
        Some(invalid) => Err((
            StatusCode::BAD_REQUEST,
            Json(ExportError {
                error: format!(
                    "Invalid merge_strategy: '{invalid}'. Valid values: merge, replace, \
                     skip_existing"
                ),
            }),
        )),
    }
}
