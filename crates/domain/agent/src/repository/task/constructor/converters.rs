use crate::error::TaskError;
use crate::models::a2a::{DataPart, FileContent, FilePart, Part, TaskState, TextPart};
use crate::models::{MessagePart, TaskRow};
use systemprompt_models::a2a::TaskMetadata;
use systemprompt_traits::RepositoryError;

pub fn construct_metadata(row: &TaskRow) -> TaskMetadata {
    let metadata_json = row
        .metadata
        .as_ref()
        .map_or_else(|| "{}".to_string(), ToString::to_string);

    let agent_name = row
        .agent_name
        .as_ref()
        .map_or_else(String::new, ToString::to_string);

    let mut metadata = serde_json::from_str::<TaskMetadata>(&metadata_json)
        .unwrap_or_else(|_| TaskMetadata::new_agent_message(agent_name.clone()));

    metadata.agent_name = agent_name;
    metadata.created_at = row.created_at.to_rfc3339();
    metadata.updated_at = Some(row.updated_at.to_rfc3339());
    metadata.started_at = row.started_at.map(|dt| dt.to_rfc3339());
    metadata.completed_at = row.completed_at.map(|dt| dt.to_rfc3339());
    metadata.execution_time_ms = row.execution_time_ms.map(i64::from);

    metadata
}

pub fn parse_task_state(state_str: &str) -> Result<TaskState, TaskError> {
    match state_str {
        "TASK_STATE_SUBMITTED" | "submitted" => Ok(TaskState::Submitted),
        "TASK_STATE_WORKING" | "working" => Ok(TaskState::Working),
        "TASK_STATE_INPUT_REQUIRED" | "input-required" => Ok(TaskState::InputRequired),
        "TASK_STATE_COMPLETED" | "completed" => Ok(TaskState::Completed),
        "TASK_STATE_CANCELED" | "canceled" | "cancelled" => Ok(TaskState::Canceled),
        "TASK_STATE_FAILED" | "failed" => Ok(TaskState::Failed),
        "TASK_STATE_REJECTED" | "rejected" => Ok(TaskState::Rejected),
        "TASK_STATE_AUTH_REQUIRED" | "auth-required" => Ok(TaskState::AuthRequired),
        "TASK_STATE_PENDING" => Ok(TaskState::Pending),
        "TASK_STATE_UNKNOWN" | "unknown" => Ok(TaskState::Unknown),
        _ => Err(TaskError::InvalidTaskState {
            state: state_str.to_string(),
        }),
    }
}

pub fn build_part_from_row(part_row: &MessagePart) -> Option<Part> {
    match part_row.part_kind.as_str() {
        "text" => {
            let text = part_row.text_content.clone().unwrap_or_else(String::new);
            Some(Part::Text(TextPart { text }))
        },
        "data" => {
            let data_value = part_row.data_content.as_ref()?;
            let data = data_value.as_object()?;
            Some(Part::Data(DataPart { data: data.clone() }))
        },
        "file" => Some(Part::File(FilePart {
            file: FileContent {
                name: part_row.file_name.clone(),
                mime_type: part_row.file_mime_type.clone(),
                bytes: part_row.file_bytes.clone(),
                url: None,
            },
        })),
        _ => None,
    }
}

pub fn build_parts_from_rows(part_rows: &[MessagePart]) -> Result<Vec<Part>, RepositoryError> {
    let mut parts = Vec::new();
    for part_row in part_rows {
        let part = match part_row.part_kind.as_str() {
            "text" => {
                let text = part_row.text_content.clone().unwrap_or_else(String::new);
                Part::Text(TextPart { text })
            },
            "data" => {
                let data_value = part_row.data_content.as_ref().ok_or_else(|| {
                    RepositoryError::InvalidData("Missing data_content for data part".to_string())
                })?;

                let data = data_value
                    .as_object()
                    .ok_or_else(|| {
                        RepositoryError::InvalidData(
                            "data_content must be a JSON object".to_string(),
                        )
                    })?
                    .clone();

                Part::Data(DataPart { data })
            },
            "file" => Part::File(FilePart {
                file: FileContent {
                    name: part_row.file_name.clone(),
                    mime_type: part_row.file_mime_type.clone(),
                    bytes: part_row.file_bytes.clone(),
                    url: None,
                },
            }),
            _ => continue,
        };
        parts.push(part);
    }
    Ok(parts)
}
