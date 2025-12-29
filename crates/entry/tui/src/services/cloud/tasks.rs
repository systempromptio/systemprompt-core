use anyhow::Result;
use systemprompt_client::SystempromptClient;
use systemprompt_identifiers::JwtToken;
use systemprompt_models::a2a::Task;

pub async fn delete_task(api_url: &str, token: &JwtToken, task_id: &str) -> Result<()> {
    let client = SystempromptClient::new(api_url)?.with_token(token.clone());
    client
        .delete_task(task_id)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to delete task: {}", e))
}

pub async fn fetch_tasks_by_context(
    api_url: &str,
    token: &JwtToken,
    context_id: &str,
) -> Result<Vec<Task>> {
    let client = SystempromptClient::new(api_url)?.with_token(token.clone());
    client
        .list_tasks(context_id)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to fetch tasks: {}", e))
}
