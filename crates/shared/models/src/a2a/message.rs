use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ContextId, MessageId, TaskId};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_field_names)]
pub struct Message {
    pub role: MessageRole,
    pub parts: Vec<Part>,
    pub message_id: MessageId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<TaskId>,
    pub context_id: ContextId,
    pub metadata: Option<serde_json::Value>,
    pub extensions: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_task_ids: Option<Vec<TaskId>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum MessageRole {
    #[serde(rename = "ROLE_USER")]
    User,
    #[serde(rename = "ROLE_AGENT")]
    Agent,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum Part {
    Text(TextPart),
    File(FilePart),
    Data(DataPart),
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct TextPart {
    pub text: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct DataPart {
    pub data: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct FilePart {
    pub file: FileContent,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FileContent {
    pub name: Option<String>,
    pub mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

impl Part {
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(text_part) => Some(&text_part.text),
            _ => None,
        }
    }

    pub fn as_data(&self) -> Option<serde_json::Value> {
        match self {
            Self::Data(data_part) => Some(serde_json::Value::Object(data_part.data.clone())),
            _ => None,
        }
    }

    pub fn as_file(&self) -> Option<serde_json::Value> {
        match self {
            Self::File(file_part) => serde_json::to_value(&file_part.file).ok(),
            _ => None,
        }
    }
}
