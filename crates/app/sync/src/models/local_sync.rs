//! Plain-data structs returned by the local-sync diff calculators and the
//! disk-side parsers. All fields are public so callers can render them
//! directly into CLI tables or JSON responses.

use chrono::{DateTime, Utc};
use serde::Serialize;
use std::fmt;
use systemprompt_identifiers::{AgentId, SkillId, SourceId};

/// Direction in which a local sync run propagates state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub enum LocalSyncDirection {
    /// Database is the source of truth; disk is updated to match.
    #[default]
    ToDisk,
    /// Disk is the source of truth; database is updated to match.
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

/// Per-item diff classification used by every `*DiffItem` struct.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum DiffStatus {
    /// Item exists on disk but not in the database.
    Added,
    /// Item exists in the database but not on disk.
    Removed,
    /// Item exists on both sides but the hashes differ.
    Modified,
}

/// Diff entry for a single content row.
#[derive(Clone, Debug, Serialize)]
pub struct ContentDiffItem {
    /// Content slug.
    pub slug: String,
    /// Source identifier.
    pub source_id: SourceId,
    /// Diff classification.
    pub status: DiffStatus,
    /// SHA-256 hex of the disk-side title + body, when known.
    pub disk_hash: Option<String>,
    /// SHA-256 hex of the database-side title + body, when known.
    pub db_hash: Option<String>,
    /// `updated_at` from the disk frontmatter, when available.
    pub disk_updated_at: Option<DateTime<Utc>>,
    /// `updated_at` from the database row, when available.
    pub db_updated_at: Option<DateTime<Utc>>,
    /// Display title for human-readable rendering.
    pub title: Option<String>,
}

/// Diff entry for a single skill.
#[derive(Clone, Debug, Serialize)]
pub struct SkillDiffItem {
    /// Skill identifier.
    pub skill_id: SkillId,
    /// On-disk path of the skill markdown file.
    pub file_path: String,
    /// Diff classification.
    pub status: DiffStatus,
    /// SHA-256 hex of the disk-side metadata + instructions, when known.
    pub disk_hash: Option<String>,
    /// SHA-256 hex of the database-side metadata + instructions, when known.
    pub db_hash: Option<String>,
    /// Skill display name.
    pub name: Option<String>,
}

/// Diff result for a single content source.
#[derive(Clone, Debug, Serialize)]
pub struct ContentDiffResult {
    /// Source identifier this diff targets.
    pub source_id: SourceId,
    /// Items that exist on disk but not in the database.
    pub added: Vec<ContentDiffItem>,
    /// Items that exist in the database but not on disk.
    pub removed: Vec<ContentDiffItem>,
    /// Items present on both sides with differing hashes.
    pub modified: Vec<ContentDiffItem>,
    /// Count of items present on both sides with identical hashes.
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
    /// Whether the diff contains any non-trivial changes.
    pub fn has_changes(&self) -> bool {
        !self.added.is_empty() || !self.removed.is_empty() || !self.modified.is_empty()
    }
}

/// Diff result for the skills directory.
#[derive(Clone, Debug, Default, Serialize)]
pub struct SkillsDiffResult {
    /// Skills present on disk but not in the database.
    pub added: Vec<SkillDiffItem>,
    /// Skills present in the database but not on disk.
    pub removed: Vec<SkillDiffItem>,
    /// Skills present on both sides with differing hashes.
    pub modified: Vec<SkillDiffItem>,
    /// Count of skills present on both sides with identical hashes.
    pub unchanged: usize,
}

impl SkillsDiffResult {
    /// Whether the diff contains any non-trivial changes.
    pub fn has_changes(&self) -> bool {
        !self.added.is_empty() || !self.removed.is_empty() || !self.modified.is_empty()
    }
}

/// Outcome of applying a [`LocalSyncDirection`] sync operation.
#[derive(Clone, Debug, Default, Serialize)]
pub struct LocalSyncResult {
    /// Items written or imported successfully.
    pub items_synced: usize,
    /// Items skipped because they fell outside the configured operation.
    pub items_skipped: usize,
    /// Items already-modified on disk that were preserved instead of
    /// overwritten when `override_existing` was `false`.
    pub items_skipped_modified: usize,
    /// Items deleted as part of orphan cleanup.
    pub items_deleted: usize,
    /// Display strings of any errors encountered.
    pub errors: Vec<String>,
    /// Direction the sync propagated state in.
    pub direction: LocalSyncDirection,
}

/// On-disk representation of a content item parsed from a Markdown file.
#[derive(Debug)]
pub struct DiskContent {
    /// Frontmatter `slug`.
    pub slug: String,
    /// Frontmatter `title`.
    pub title: String,
    /// Markdown body (frontmatter stripped).
    pub body: String,
}

/// On-disk representation of a skill parsed from `<skill>/config.yaml` and
/// `<skill>/SKILL.md`.
#[derive(Debug)]
pub struct DiskSkill {
    /// Skill identifier.
    pub skill_id: SkillId,
    /// Display name.
    pub name: String,
    /// Description.
    pub description: String,
    /// Markdown body of `SKILL.md` (frontmatter stripped).
    pub instructions: String,
    /// On-disk path of `SKILL.md`.
    pub file_path: String,
}

/// Diff entry for a single agent.
#[derive(Clone, Debug, Serialize)]
pub struct AgentDiffItem {
    /// Agent identifier.
    pub agent_id: AgentId,
    /// Agent name (matches the directory name on disk).
    pub name: String,
    /// Diff classification.
    pub status: DiffStatus,
    /// SHA-256 hex of the disk-side metadata + system prompt, when known.
    pub disk_hash: Option<String>,
    /// SHA-256 hex of the database-side metadata + system prompt, when known.
    pub db_hash: Option<String>,
}

/// Diff result for the agents directory.
#[derive(Clone, Debug, Default, Serialize)]
pub struct AgentsDiffResult {
    /// Agents present on disk but not in the database.
    pub added: Vec<AgentDiffItem>,
    /// Agents present in the database but not on disk.
    pub removed: Vec<AgentDiffItem>,
    /// Agents present on both sides with differing hashes.
    pub modified: Vec<AgentDiffItem>,
    /// Count of agents present on both sides with identical hashes.
    pub unchanged: usize,
}

impl AgentsDiffResult {
    /// Whether the diff contains any non-trivial changes.
    pub fn has_changes(&self) -> bool {
        !self.added.is_empty() || !self.removed.is_empty() || !self.modified.is_empty()
    }
}

/// On-disk representation of an agent parsed from `<agent>/config.yaml` and
/// `<agent>/system_prompt.md`.
#[derive(Debug)]
pub struct DiskAgent {
    /// Agent identifier.
    pub agent_id: AgentId,
    /// Internal name (matches the directory name).
    pub name: String,
    /// Display name shown to users.
    pub display_name: String,
    /// Description.
    pub description: String,
    /// Markdown body of `system_prompt.md`, when present.
    pub system_prompt: Option<String>,
    /// HTTP port the agent listens on.
    pub port: u16,
}
