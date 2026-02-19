use chrono::{DateTime, Utc};
use serde::Serialize;
use std::fmt;
use systemprompt_identifiers::{AgentId, SkillId, SourceId};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub enum LocalSyncDirection {
    #[default]
    ToDisk,
    ToDatabase,
}

impl fmt::Display for LocalSyncDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ToDisk => write!(f, "to_disk"),
            Self::ToDatabase => write!(f, "to_database"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum DiffStatus {
    Added,
    Removed,
    Modified,
}

#[derive(Clone, Debug, Serialize)]
pub struct ContentDiffItem {
    pub slug: String,
    pub source_id: SourceId,
    pub status: DiffStatus,
    pub disk_hash: Option<String>,
    pub db_hash: Option<String>,
    pub disk_updated_at: Option<DateTime<Utc>>,
    pub db_updated_at: Option<DateTime<Utc>>,
    pub title: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SkillDiffItem {
    pub skill_id: SkillId,
    pub file_path: String,
    pub status: DiffStatus,
    pub disk_hash: Option<String>,
    pub db_hash: Option<String>,
    pub name: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ContentDiffResult {
    pub source_id: SourceId,
    pub added: Vec<ContentDiffItem>,
    pub removed: Vec<ContentDiffItem>,
    pub modified: Vec<ContentDiffItem>,
    pub unchanged: usize,
}

impl Default for ContentDiffResult {
    fn default() -> Self {
        Self {
            source_id: SourceId::new(""),
            added: Vec::new(),
            removed: Vec::new(),
            modified: Vec::new(),
            unchanged: 0,
        }
    }
}

impl ContentDiffResult {
    pub fn has_changes(&self) -> bool {
        !self.added.is_empty() || !self.removed.is_empty() || !self.modified.is_empty()
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct SkillsDiffResult {
    pub added: Vec<SkillDiffItem>,
    pub removed: Vec<SkillDiffItem>,
    pub modified: Vec<SkillDiffItem>,
    pub unchanged: usize,
}

impl SkillsDiffResult {
    pub fn has_changes(&self) -> bool {
        !self.added.is_empty() || !self.removed.is_empty() || !self.modified.is_empty()
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct LocalSyncResult {
    pub items_synced: usize,
    pub items_skipped: usize,
    pub items_skipped_modified: usize,
    pub items_deleted: usize,
    pub errors: Vec<String>,
    pub direction: LocalSyncDirection,
}

#[derive(Debug)]
pub struct DiskContent {
    pub slug: String,
    pub title: String,
    pub body: String,
}

#[derive(Debug)]
pub struct DiskSkill {
    pub skill_id: SkillId,
    pub name: String,
    pub description: String,
    pub instructions: String,
    pub file_path: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct AgentDiffItem {
    pub agent_id: AgentId,
    pub name: String,
    pub status: DiffStatus,
    pub disk_hash: Option<String>,
    pub db_hash: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct AgentsDiffResult {
    pub added: Vec<AgentDiffItem>,
    pub removed: Vec<AgentDiffItem>,
    pub modified: Vec<AgentDiffItem>,
    pub unchanged: usize,
}

impl AgentsDiffResult {
    pub fn has_changes(&self) -> bool {
        !self.added.is_empty() || !self.removed.is_empty() || !self.modified.is_empty()
    }
}

#[derive(Debug)]
pub struct DiskAgent {
    pub agent_id: AgentId,
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub system_prompt: Option<String>,
    pub port: u16,
}
