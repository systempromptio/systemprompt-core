//! The flush buffer transposed into one column vector per `logs` column, so a
//! batch inserts through a single `UNNEST` rather than a row-per-INSERT loop.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::models::LogEntry;
use systemprompt_identifiers::{ClientId, ContextId, TaskId};

pub(super) struct LogColumns {
    pub(super) ids: Vec<String>,
    pub(super) timestamps: Vec<chrono::DateTime<chrono::Utc>>,
    pub(super) levels: Vec<String>,
    pub(super) modules: Vec<String>,
    pub(super) messages: Vec<String>,
    pub(super) metadata: Vec<Option<String>>,
    pub(super) user_ids: Vec<String>,
    pub(super) session_ids: Vec<String>,
    pub(super) task_ids: Vec<Option<String>>,
    pub(super) trace_ids: Vec<String>,
    pub(super) context_ids: Vec<Option<String>>,
    pub(super) client_ids: Vec<Option<String>>,
    pub(super) instance_ids: Vec<Option<String>>,
}

impl LogColumns {
    pub(super) fn gather(entries: &[LogEntry]) -> Result<Self, crate::models::LoggingError> {
        let mut ids = Vec::with_capacity(entries.len());
        let mut timestamps = Vec::with_capacity(entries.len());
        let mut levels = Vec::with_capacity(entries.len());
        let mut modules = Vec::with_capacity(entries.len());
        let mut messages = Vec::with_capacity(entries.len());
        let mut metadata = Vec::with_capacity(entries.len());
        let mut user_ids = Vec::with_capacity(entries.len());
        let mut session_ids = Vec::with_capacity(entries.len());
        let mut task_ids = Vec::with_capacity(entries.len());
        let mut trace_ids = Vec::with_capacity(entries.len());
        let mut context_ids = Vec::with_capacity(entries.len());
        let mut client_ids = Vec::with_capacity(entries.len());
        let mut instance_ids = Vec::with_capacity(entries.len());
        for entry in entries {
            ids.push(entry.id.as_str().to_owned());
            timestamps.push(entry.timestamp);
            levels.push(entry.level.to_string());
            modules.push(entry.module.clone());
            messages.push(entry.message.clone());
            metadata.push(
                entry
                    .metadata
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()?,
            );
            user_ids.push(entry.user_id.as_str().to_owned());
            session_ids.push(entry.session_id.as_str().to_owned());
            task_ids.push(
                entry
                    .task_id
                    .as_ref()
                    .map(TaskId::as_str)
                    .map(str::to_owned),
            );
            trace_ids.push(entry.trace_id.as_str().to_owned());
            context_ids.push(
                entry
                    .context_id
                    .as_ref()
                    .map(ContextId::as_str)
                    .map(str::to_owned),
            );
            client_ids.push(
                entry
                    .client_id
                    .as_ref()
                    .map(ClientId::as_str)
                    .map(str::to_owned),
            );
            instance_ids.push(
                entry
                    .instance_id
                    .as_ref()
                    .map(systemprompt_identifiers::InstanceId::as_str)
                    .map(str::to_owned),
            );
        }
        Ok(Self {
            ids,
            timestamps,
            levels,
            modules,
            messages,
            metadata,
            user_ids,
            session_ids,
            task_ids,
            trace_ids,
            context_ids,
            client_ids,
            instance_ids,
        })
    }
}
