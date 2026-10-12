//! Caller-bound `ListTasks`: the authenticated user's tasks, filtered,
//! paged by an opaque [`PageToken`] and trimmed to the requested history and
//! artifact shape.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use systemprompt_identifiers::UserId;

use crate::models::a2a::Task;
use crate::models::a2a::protocol::{ListTasksParams, ListTasksResult};
use crate::repository::task::{ListTasksFilter, TaskRepository};

pub const DEFAULT_PAGE_SIZE: u32 = 50;
pub const MAX_PAGE_SIZE: u32 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageToken(u32);

#[derive(Debug, thiserror::Error)]
pub enum PageTokenError {
    #[error("page token is not valid base64")]
    Encoding(#[source] base64::DecodeError),
    #[error("page token does not name a page offset")]
    Offset,
}

impl PageToken {
    pub const fn from_offset(offset: u32) -> Self {
        Self(offset)
    }

    pub const fn offset(self) -> u32 {
        self.0
    }

    pub fn encode(self) -> String {
        URL_SAFE_NO_PAD.encode(self.0.to_string())
    }

    pub fn decode(token: &str) -> Result<Self, PageTokenError> {
        let bytes = URL_SAFE_NO_PAD
            .decode(token.trim())
            .map_err(PageTokenError::Encoding)?;
        std::str::from_utf8(&bytes)
            .ok()
            .and_then(|text| text.parse::<u32>().ok())
            .map(Self)
            .ok_or(PageTokenError::Offset)
    }
}

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
    PageToken(#[source] PageTokenError),
    #[error("task listing failed")]
    Repository(#[source] systemprompt_traits::RepositoryError),
}

pub async fn list_caller_tasks(
    task_repo: &TaskRepository,
    user_id: &UserId,
    params: &ListTasksParams,
) -> Result<ListTasksResult, ListTasksError> {
    let page_size = clamp_page_size(params.page_size);
    let page = match params.page_token.as_deref().filter(|t| !t.is_empty()) {
        Some(token) => PageToken::decode(token).map_err(ListTasksError::PageToken)?,
        None => PageToken::from_offset(0),
    };

    let filter = ListTasksFilter {
        user_id,
        context_id: params.context_id.as_ref(),
        status: params.status,
        status_timestamp_after: params.status_timestamp_after,
    };

    let total = task_repo
        .count_tasks(filter)
        .await
        .map_err(ListTasksError::Repository)?;
    let mut tasks = task_repo
        .list_tasks(filter, i64::from(page_size), i64::from(page.offset()))
        .await
        .map_err(ListTasksError::Repository)?;

    let include_artifacts = params.include_artifacts.unwrap_or(false);
    for task in &mut tasks {
        trim_history(task, params.history_length);
        if !include_artifacts {
            task.artifacts = None;
        }
    }

    let returned = u32::try_from(tasks.len()).unwrap_or(u32::MAX);
    let next_offset = page.offset().saturating_add(returned);
    let total_size = u32::try_from(total).unwrap_or(u32::MAX);
    let next_page_token = if returned > 0 && next_offset < total_size {
        PageToken::from_offset(next_offset).encode()
    } else {
        String::new()
    };

    Ok(ListTasksResult {
        tasks,
        next_page_token,
        page_size,
        total_size,
    })
}
