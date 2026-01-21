use crate::models::a2a::{Task, TaskStatus};
use crate::models::{
    ArtifactPartRow, ArtifactRow, ExecutionStepBatchRow, MessagePart, TaskMessage, TaskRow,
};
use std::collections::HashMap;
use systemprompt_identifiers::TaskId;
use systemprompt_traits::RepositoryError;

use super::batch_builders::{build_artifacts, build_execution_steps, build_messages};
use super::{converters, TaskConstructor};

pub async fn construct_tasks_batch(
    constructor: &TaskConstructor,
    task_ids: &[TaskId],
) -> Result<Vec<Task>, RepositoryError> {
    if task_ids.is_empty() {
        return Ok(Vec::new());
    }

    let pool = constructor.get_pg_pool()?;
    let task_id_strings: Vec<String> = task_ids.iter().map(|id| id.to_string()).collect();

    let task_rows = super::batch_queries::fetch_task_rows(&pool, &task_id_strings).await?;
    let all_messages = super::batch_queries::fetch_messages(&pool, &task_id_strings).await?;
    let all_parts = super::batch_queries::fetch_message_parts(&pool, &task_id_strings).await?;
    let all_artifact_rows = super::batch_queries::fetch_artifacts(&pool, &task_id_strings).await?;
    let all_execution_steps =
        super::batch_queries::fetch_execution_steps(&pool, &task_id_strings).await?;

    let artifact_ids: Vec<String> = all_artifact_rows
        .iter()
        .map(|a| a.artifact_id.clone())
        .collect();
    let all_artifact_parts =
        super::batch_queries::fetch_artifact_parts(&pool, &artifact_ids).await?;

    let parts_by_message = group_by_key(&all_parts, |p| p.message_id.clone());
    let messages_by_task = group_by_key(&all_messages, |m| m.task_id.clone());
    let artifacts_by_task = group_by_key(&all_artifact_rows, |a| a.task_id.clone());
    let artifact_parts_by_id = group_by_key(&all_artifact_parts, |p| p.artifact_id.clone());
    let steps_by_task = group_by_key(&all_execution_steps, |s| s.task_id.clone());

    build_tasks(
        &task_rows,
        &messages_by_task,
        &parts_by_message,
        &artifacts_by_task,
        &artifact_parts_by_id,
        &steps_by_task,
    )
}

fn group_by_key<T, F, K>(items: &[T], key_fn: F) -> HashMap<K, Vec<&T>>
where
    F: Fn(&T) -> K,
    K: std::hash::Hash + Eq,
{
    items.iter().fold(HashMap::new(), |mut acc, item| {
        let key = key_fn(item);
        acc.entry(key).or_default().push(item);
        acc
    })
}

fn build_tasks(
    task_rows: &[TaskRow],
    messages_by_task: &HashMap<String, Vec<&TaskMessage>>,
    parts_by_message: &HashMap<String, Vec<&MessagePart>>,
    artifacts_by_task: &HashMap<String, Vec<&ArtifactRow>>,
    artifact_parts_by_id: &HashMap<String, Vec<&ArtifactPartRow>>,
    steps_by_task: &HashMap<String, Vec<&ExecutionStepBatchRow>>,
) -> Result<Vec<Task>, RepositoryError> {
    let mut tasks = Vec::new();

    for row in task_rows {
        let history = build_messages(messages_by_task.get(&row.task_id), parts_by_message);
        let artifacts = build_artifacts(artifacts_by_task.get(&row.task_id), artifact_parts_by_id);
        let execution_steps = build_execution_steps(steps_by_task.get(&row.task_id));

        let mut metadata = converters::construct_metadata(row)?;
        if let Some(ref mut meta) = metadata {
            meta.execution_steps = execution_steps;
        }

        let task_state = converters::parse_task_state(&row.status)
            .map_err(|e| RepositoryError::InvalidData(e.to_string()))?;

        tasks.push(Task {
            id: row.task_id.clone().into(),
            context_id: row.context_id.clone().into(),
            kind: "task".to_string(),
            status: TaskStatus {
                state: task_state,
                message: None,
                timestamp: row.status_timestamp,
            },
            history,
            artifacts,
            metadata,
        });
    }

    Ok(tasks)
}
