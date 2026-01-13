use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SkillListOutput {
    pub skills: Vec<SkillSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SkillSummary {
    pub skill_id: String,
    pub name: String,
    pub enabled: bool,
    pub tags: Vec<String>,
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SkillDetailOutput {
    pub skill_id: String,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub tags: Vec<String>,
    pub category: Option<String>,
    pub file_path: String,
    pub instructions_preview: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum ListOrDetail {
    List(SkillListOutput),
    Detail(SkillDetailOutput),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SkillCreateOutput {
    pub skill_id: String,
    pub message: String,
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SkillEditOutput {
    pub skill_id: String,
    pub message: String,
    pub changes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SkillDeleteOutput {
    pub deleted: Vec<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SkillSyncOutput {
    pub direction: String,
    pub synced: usize,
    pub skipped: usize,
    pub deleted: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SkillStatusOutput {
    pub skills: Vec<SkillStatusRow>,
    pub summary: SkillStatusSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SkillStatusRow {
    pub skill_id: String,
    pub name: String,
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
pub struct SkillStatusSummary {
    pub total: usize,
    pub synced: usize,
    pub disk_only: usize,
    pub db_only: usize,
    pub modified: usize,
}
