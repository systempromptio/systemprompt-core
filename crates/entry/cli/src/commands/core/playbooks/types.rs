use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PlaybookListOutput {
    pub playbooks: Vec<PlaybookSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PlaybookSummary {
    pub playbook_id: String,
    pub name: String,
    pub category: String,
    pub domain: String,
    pub enabled: bool,
    pub tags: Vec<String>,
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PlaybookDetailOutput {
    pub playbook_id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub domain: String,
    pub enabled: bool,
    pub tags: Vec<String>,
    pub file_path: String,
    pub instructions_preview: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum ListOrDetail {
    List(PlaybookListOutput),
    Detail(PlaybookDetailOutput),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PlaybookSyncOutput {
    pub direction: String,
    pub synced: usize,
    pub skipped: usize,
    pub deleted: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PlaybookStatusOutput {
    pub playbooks: Vec<PlaybookStatusRow>,
    pub summary: PlaybookStatusSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PlaybookStatusRow {
    pub playbook_id: String,
    pub name: String,
    pub category: String,
    pub domain: String,
    pub on_disk: bool,
    pub in_db: bool,
    pub status: SyncStatus,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum SyncStatus {
    Synced,
    DiskOnly,
    DbOnly,
    Modified,
}

impl std::fmt::Display for SyncStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Synced => write!(f, "synced"),
            Self::DiskOnly => write!(f, "disk-only"),
            Self::DbOnly => write!(f, "db-only"),
            Self::Modified => write!(f, "modified"),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct PlaybookStatusSummary {
    pub total: usize,
    pub synced: usize,
    pub disk_only: usize,
    pub db_only: usize,
    pub modified: usize,
}
