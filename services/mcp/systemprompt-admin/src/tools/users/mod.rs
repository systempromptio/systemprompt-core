mod models;
pub mod repository;
mod schema;

pub use schema::{users_input_schema, users_input_schema_with_roles, users_output_schema};

use anyhow::Result;
use repository::UsersRepository;
use rmcp::{
    model::{CallToolRequestParam, CallToolResult, Content},
    service::RequestContext,
    ErrorData as McpError, RoleServer,
};
use serde_json::{json, Map as JsonMap, Value as JsonValue};
use systemprompt::database::DbPool;
use systemprompt::identifiers::{ArtifactId, McpExecutionId, UserId};
use systemprompt::models::artifacts::{
    Column, ColumnType, ExecutionMetadata, TableArtifact, ToolResponse,
};
use systemprompt::users::UserRepository;

pub async fn handle_users(
    pool: &DbPool,
    request: CallToolRequestParam,
    _ctx: RequestContext<RoleServer>,
    mcp_execution_id: &McpExecutionId,
) -> Result<CallToolResult, McpError> {
    let args = request.arguments.unwrap_or_default();
    let action = args
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("list");

    match action {
        "list" => handle_list_users(pool, &args, mcp_execution_id).await,
        "assign_role" => handle_assign_role(pool, &args, mcp_execution_id).await,
        "remove_role" => handle_remove_role(pool, &args, mcp_execution_id).await,
        "delete" => handle_delete_user(pool, &args, mcp_execution_id).await,
        _ => Err(McpError::invalid_params(
            format!("Unknown action: {action}"),
            None,
        )),
    }
}

async fn handle_list_users(
    pool: &DbPool,
    args: &JsonMap<String, JsonValue>,
    mcp_execution_id: &McpExecutionId,
) -> Result<CallToolResult, McpError> {
    let user_id = args.get("user_id").and_then(|v| v.as_str());

    let repo = UsersRepository::new(pool.clone())
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;

    tracing::debug!(user_id = ?user_id, "Listing users");

    let users = repo
        .list_users(user_id)
        .await
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;

    tracing::debug!(count = users.len(), "Users listed");

    let items: Vec<JsonValue> = users.iter().map(|u| json!(u)).collect();

    let columns = vec![
        Column::new("id", ColumnType::String).with_label("ID"),
        Column::new("name", ColumnType::String).with_label("Name"),
        Column::new("email", ColumnType::String).with_label("Email"),
        Column::new("display_name", ColumnType::String).with_label("Display Name"),
        Column::new("status", ColumnType::String).with_label("Status"),
        Column::new("roles", ColumnType::String).with_label("Roles"),
        Column::new("total_sessions", ColumnType::Integer).with_label("Sessions"),
        Column::new("created_at", ColumnType::Date).with_label("Created"),
    ];

    let metadata = ExecutionMetadata::new().tool("users");
    let artifact_id = ArtifactId::new(uuid::Uuid::new_v4().to_string());
    let artifact = TableArtifact::new(columns)
        .with_rows(items.clone())
        .with_metadata(metadata.clone());
    let tool_response = ToolResponse::new(
        artifact_id,
        mcp_execution_id.clone(),
        artifact,
        metadata.clone(),
    );

    Ok(CallToolResult {
        content: vec![Content::text(format!(
            "Found {} users\n\n{}",
            users.len(),
            serde_json::to_string_pretty(&items).unwrap_or_default()
        ))],
        structured_content: Some(tool_response.to_json()),
        is_error: Some(false),
        meta: metadata.to_meta(),
    })
}

async fn handle_assign_role(
    pool: &DbPool,
    args: &JsonMap<String, JsonValue>,
    _mcp_execution_id: &McpExecutionId,
) -> Result<CallToolResult, McpError> {
    let user_id_str = args
        .get("user_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("user_id is required for assign_role", None))?;

    let role = args
        .get("role")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("role is required for assign_role", None))?;

    let user_id = UserId::new(user_id_str.to_string());
    let user_repo =
        UserRepository::new(pool).map_err(|e| McpError::internal_error(e.to_string(), None))?;

    let user = user_repo
        .find_by_id(&user_id)
        .await
        .map_err(|e| McpError::internal_error(e.to_string(), None))?
        .ok_or_else(|| McpError::invalid_params(format!("User not found: {user_id_str}"), None))?;

    let mut roles: Vec<String> = user.roles.clone();
    if !roles.contains(&role.to_string()) {
        roles.push(role.to_string());
    }

    let updated_user = user_repo
        .assign_roles(&user_id, &roles)
        .await
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;

    tracing::info!(user_id = %user_id_str, role = %role, "Role assigned to user");

    let metadata = ExecutionMetadata::new().tool("users");
    Ok(CallToolResult {
        content: vec![Content::text(format!(
            "Successfully assigned role '{}' to user '{}' ({})\nCurrent roles: {:?}",
            role, updated_user.name, user_id_str, updated_user.roles
        ))],
        structured_content: Some(json!({
            "success": true,
            "action": "assign_role",
            "user_id": user_id_str,
            "role": role,
            "current_roles": updated_user.roles
        })),
        is_error: Some(false),
        meta: metadata.to_meta(),
    })
}

async fn handle_remove_role(
    pool: &DbPool,
    args: &JsonMap<String, JsonValue>,
    _mcp_execution_id: &McpExecutionId,
) -> Result<CallToolResult, McpError> {
    let user_id_str = args
        .get("user_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("user_id is required for remove_role", None))?;

    let role = args
        .get("role")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("role is required for remove_role", None))?;

    let user_id = UserId::new(user_id_str.to_string());
    let user_repo =
        UserRepository::new(pool).map_err(|e| McpError::internal_error(e.to_string(), None))?;

    let user = user_repo
        .find_by_id(&user_id)
        .await
        .map_err(|e| McpError::internal_error(e.to_string(), None))?
        .ok_or_else(|| McpError::invalid_params(format!("User not found: {user_id_str}"), None))?;

    let mut roles: Vec<String> = user.roles.clone();
    roles.retain(|r| r != role);

    if roles.is_empty() {
        return Err(McpError::invalid_params(
            "Cannot remove the last role from a user. Users must have at least one role.",
            None,
        ));
    }

    let updated_user = user_repo
        .assign_roles(&user_id, &roles)
        .await
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;

    tracing::info!(user_id = %user_id_str, role = %role, "Role removed from user");

    let metadata = ExecutionMetadata::new().tool("users");
    Ok(CallToolResult {
        content: vec![Content::text(format!(
            "Successfully removed role '{}' from user '{}' ({})\nCurrent roles: {:?}",
            role, updated_user.name, user_id_str, updated_user.roles
        ))],
        structured_content: Some(json!({
            "success": true,
            "action": "remove_role",
            "user_id": user_id_str,
            "role": role,
            "current_roles": updated_user.roles
        })),
        is_error: Some(false),
        meta: metadata.to_meta(),
    })
}

async fn handle_delete_user(
    pool: &DbPool,
    args: &JsonMap<String, JsonValue>,
    _mcp_execution_id: &McpExecutionId,
) -> Result<CallToolResult, McpError> {
    let user_id_str = args
        .get("user_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("user_id is required for delete", None))?;

    let user_id = UserId::new(user_id_str.to_string());
    let user_repo =
        UserRepository::new(pool).map_err(|e| McpError::internal_error(e.to_string(), None))?;

    let user = user_repo
        .find_by_id(&user_id)
        .await
        .map_err(|e| McpError::internal_error(e.to_string(), None))?
        .ok_or_else(|| McpError::invalid_params(format!("User not found: {user_id_str}"), None))?;

    user_repo
        .delete(&user_id)
        .await
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;

    tracing::info!(user_id = %user_id_str, user_name = %user.name, "User deleted (soft delete)");

    let metadata = ExecutionMetadata::new().tool("users");
    Ok(CallToolResult {
        content: vec![Content::text(format!(
            "Successfully deleted user '{}' ({}). User status has been set to 'deleted'.",
            user.name, user_id_str
        ))],
        structured_content: Some(json!({
            "success": true,
            "action": "delete",
            "user_id": user_id_str,
            "user_name": user.name
        })),
        is_error: Some(false),
        meta: metadata.to_meta(),
    })
}
