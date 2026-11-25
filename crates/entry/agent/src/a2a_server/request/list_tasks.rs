//! Caller-bound `ListTasks`: the authenticated user's tasks, filtered,
//! paged by an opaque keyset cursor and trimmed to the requested history and
//! artifact shape.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{TaskId, UserId};
use systemprompt_models::api::{KeysetCursor, KeysetCursorError};

use systemprompt_agent::models::a2a::Task;
use systemprompt_agent::models::a2a::protocol::{ListTasksParams, ListTasksResult};
use systemprompt_agent::repository::task::{ListTasksFilter, TaskRepository};

pub const DEFAULT_PAGE_SIZE: u32 = 50;
pub const MAX_PAGE_SIZE: u32 = 100;

pub fn clamp_page_size(requested: Option<u32>) -> u32 {
    requested.map_or(DEFAULT_PAGE_SIZE, |size| size.clamp(1, MAX_PAGE_SIZE))
}

pub fn trim_history(task: &mut Task, history_length: Option<u32>) {
    let Some(limit) = history_length else {
        return;
    };
    if limit == 0 {
        task.history = None;
        return;
    }
    if let Some(history) = task.history.as_mut() {
        let keep = usize::try_from(limit).unwrap_or(usize::MAX);
        if history.len() > keep {
            history.drain(..history.len() - keep);
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ListTasksError {
    #[error("invalid pageToken")]
    PageToken(#[source] KeysetCursorError),
    #[error("task listing failed")]
    Repository(#[source] systemprompt_traits::RepositoryError),
}

pub async fn list_caller_tasks(
    task_repo: &TaskRepository,
    user_id: &UserId,
    params: &ListTasksParams,
) -> Result<ListTasksResult, ListTasksError> {
    let page_size = clamp_page_size(params.page_size);
    let after = params
        .page_token
        .as_deref()
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(str::parse::<KeysetCursor<TaskId>>)
        .transpose()
        .map_err(ListTasksError::PageToken)?;

    let filter = ListTasksFilter {
        user_id,
        context_id: params.context_id.as_ref(),
        status: params.status,
        status_timestamp_after: params.status_timestamp_after,
    };

    let page = task_repo
        .list_tasks(
            filter,
            after.as_ref(),
            usize::try_from(page_size).unwrap_or(usize::MAX),
        )
        .await
        .map_err(ListTasksError::Repository)?;
    let mut tasks = page.items;

    let include_artifacts = params.include_artifacts.unwrap_or(false);
    for task in &mut tasks {
        trim_history(task, params.history_length);
        if !include_artifacts {
            task.artifacts = None;
        }
    }

    let next_page_token = page
        .next_cursor
        .map_or_else(String::new, |cursor| cursor.to_string());

    Ok(ListTasksResult {
        tasks,
        next_page_token,
        page_size,
        total_size: None,
    })
}
