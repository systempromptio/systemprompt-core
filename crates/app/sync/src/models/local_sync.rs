use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalSyncDirection {
    ToDisk,
    ToDatabase,
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
    pub source_id: String,
    pub status: DiffStatus,
    pub disk_hash: Option<String>,
    pub db_hash: Option<String>,
    pub disk_updated_at: Option<DateTime<Utc>>,
    pub db_updated_at: Option<DateTime<Utc>>,
    pub title: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SkillDiffItem {
    pub skill_id: String,
    pub file_path: String,
    pub status: DiffStatus,
    pub disk_hash: Option<String>,
    pub db_hash: Option<String>,
    pub name: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct ContentDiffResult {
    pub source_id: String,
    pub added: Vec<ContentDiffItem>,
    pub removed: Vec<ContentDiffItem>,
    pub modified: Vec<ContentDiffItem>,
    pub unchanged: usize,
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
    pub items_deleted: usize,
    pub errors: Vec<String>,
    pub direction: String,
}

#[derive(Debug)]
pub struct DiskContent {
    pub slug: String,
    pub title: String,
    pub body: String,
}

#[derive(Debug)]
pub struct DiskSkill {
    pub skill_id: String,
    pub name: String,
    pub description: String,
    pub instructions: String,
    pub file_path: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct PlaybookDiffItem {
    pub playbook_id: String,
    pub file_path: String,
    pub category: String,
    pub domain: String,
    pub status: DiffStatus,
    pub disk_hash: Option<String>,
    pub db_hash: Option<String>,
    pub name: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct PlaybooksDiffResult {
    pub added: Vec<PlaybookDiffItem>,
    pub removed: Vec<PlaybookDiffItem>,
    pub modified: Vec<PlaybookDiffItem>,
    pub unchanged: usize,
}

impl PlaybooksDiffResult {
    pub fn has_changes(&self) -> bool {
        !self.added.is_empty() || !self.removed.is_empty() || !self.modified.is_empty()
    }
}

#[derive(Debug)]
pub struct DiskPlaybook {
    pub playbook_id: String,
    pub name: String,
    pub description: String,
    pub instructions: String,
    pub category: String,
    pub domain: String,
    pub file_path: String,
}
