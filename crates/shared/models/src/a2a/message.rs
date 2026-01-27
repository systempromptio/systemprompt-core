use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ContextId, MessageId, TaskId};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Message {
    pub role: String,
    pub parts: Vec<Part>,
    #[serde(rename = "messageId")]
    pub id: MessageId,
    #[serde(rename = "taskId")]
    pub task_id: Option<TaskId>,
    #[serde(rename = "contextId")]
    pub context_id: ContextId,
    #[serde(rename = "kind")]
    pub kind: String,
    pub metadata: Option<serde_json::Value>,
    pub extensions: Option<Vec<String>>,
    #[serde(rename = "referenceTaskIds")]
    pub reference_task_ids: Option<Vec<TaskId>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Agent,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(tag = "kind")]
pub enum Part {
    #[serde(rename = "text")]
    Text(TextPart),
    #[serde(rename = "data")]
    Data(DataPart),
    #[serde(rename = "file")]
    File(FilePart),
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
    pub file: FileWithBytes,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct FileWithBytes {
    pub name: Option<String>,
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
    pub bytes: String,
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
