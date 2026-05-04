//! Database push / pull: serialise the user, skill, and context tables to
//! JSON and round-trip them between a local Postgres and a cloud Postgres
//! using compile-time-checked `sqlx` upserts.

mod upsert;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use sqlx::prelude::FromRow;
use systemprompt_identifiers::{ContextId, SessionId, SkillId, SourceId, UserId};

use crate::error::SyncResult;
use crate::{SyncDirection, SyncOperationResult};

use upsert::{upsert_context, upsert_skill, upsert_user};

/// Snapshot of every table the database sync moves: users, skills, contexts.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DatabaseExport {
    /// Users in the source database.
    pub users: Vec<UserExport>,
    /// Skills in the source database.
    pub skills: Vec<SkillExport>,
    /// Contexts in the source database.
    pub contexts: Vec<ContextExport>,
    /// Time the snapshot was taken.
    pub timestamp: DateTime<Utc>,
}

/// One row of the `users` export.
#[derive(Clone, Debug, Serialize, Deserialize, FromRow)]
pub struct UserExport {
    /// User identifier.
    pub id: UserId,
    /// Login / handle.
    pub name: String,
    /// Email address.
    pub email: String,
    /// Optional full name.
    pub full_name: Option<String>,
    /// Optional display name.
    pub display_name: Option<String>,
    /// Status string (e.g. `active`).
    pub status: String,
    /// Whether the email has been verified.
    pub email_verified: bool,
    /// Granted role identifiers.
    pub roles: Vec<String>,
    /// Whether this user represents a bot account.
    pub is_bot: bool,
    /// Whether this user represents a scanner / crawler.
    pub is_scanner: bool,
    /// Optional avatar URL.
    pub avatar_url: Option<String>,
    /// Row-creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Row-update timestamp.
    pub updated_at: DateTime<Utc>,
}

/// One row of the `skills` export.
#[derive(Clone, Debug, Serialize, Deserialize, FromRow)]
pub struct SkillExport {
    /// Skill identifier.
    pub skill_id: SkillId,
    /// On-disk path of the skill markdown.
    pub file_path: String,
    /// Display name.
    pub name: String,
    /// Description.
    pub description: String,
    /// Markdown body.
    pub instructions: String,
    /// Whether the skill is enabled.
    pub enabled: bool,
    /// Optional tag list.
    pub tags: Option<Vec<String>>,
    /// Optional category identifier.
    pub category_id: Option<String>,
    /// Source identifier.
    pub source_id: SourceId,
    /// Row-creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Row-update timestamp.
    pub updated_at: DateTime<Utc>,
}

/// One row of the `contexts` export.
#[derive(Clone, Debug, Serialize, Deserialize, FromRow)]
pub struct ContextExport {
    /// Context identifier.
    pub context_id: ContextId,
    /// Owning user.
    pub user_id: UserId,
    /// Optional session this context belongs to.
    pub session_id: Option<SessionId>,
    /// Display name.
    pub name: String,
    /// Row-creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Row-update timestamp.
    pub updated_at: DateTime<Utc>,
}

/// Aggregate counts produced by an import pass.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ImportResult {
    /// Rows newly inserted.
    pub created: usize,
    /// Rows updated in place.
    pub updated: usize,
    /// Rows skipped (already up to date or rejected).
    pub skipped: usize,
}

/// Drives database push / pull.
#[derive(Debug)]
pub struct DatabaseSyncService {
    direction: SyncDirection,
    dry_run: bool,
    local_database_url: String,
    cloud_database_url: String,
}

impl DatabaseSyncService {
    /// Construct a new database sync service.
    pub fn new(
        direction: SyncDirection,
        dry_run: bool,
        local_database_url: &str,
        cloud_database_url: &str,
    ) -> Self {
        Self {
            direction,
            dry_run,
            local_database_url: local_database_url.to_string(),
            cloud_database_url: cloud_database_url.to_string(),
        }
    }

    /// Run the configured push or pull.
    pub async fn sync(&self) -> SyncResult<SyncOperationResult> {
        match self.direction {
            SyncDirection::Push => self.push().await,
            SyncDirection::Pull => self.pull().await,
        }
    }

    async fn push(&self) -> SyncResult<SyncOperationResult> {
        let export = export_from_database(&self.local_database_url).await?;
        let count = export.users.len() + export.skills.len() + export.contexts.len();

        if self.dry_run {
            return Ok(SyncOperationResult::dry_run(
                "database_push",
                count,
                serde_json::json!({
                    "users": export.users.len(),
                    "skills": export.skills.len(),
                    "contexts": export.contexts.len(),
                }),
            ));
        }

        import_to_database(&self.cloud_database_url, &export).await?;
        Ok(SyncOperationResult::success("database_push", count))
    }

    async fn pull(&self) -> SyncResult<SyncOperationResult> {
        let export = export_from_database(&self.cloud_database_url).await?;
        let count = export.users.len() + export.skills.len() + export.contexts.len();

        if self.dry_run {
            return Ok(SyncOperationResult::dry_run(
                "database_pull",
                count,
                serde_json::json!({
                    "users": export.users.len(),
                    "skills": export.skills.len(),
                    "contexts": export.contexts.len(),
                }),
            ));
        }

        import_to_database(&self.local_database_url, &export).await?;
        Ok(SyncOperationResult::success("database_pull", count))
    }
}

async fn export_from_database(database_url: &str) -> SyncResult<DatabaseExport> {
    let pool = PgPool::connect(database_url).await?;

    let users = sqlx::query_as!(
        UserExport,
        r#"SELECT id, name, email, full_name, display_name, status, email_verified,
                  roles, is_bot, is_scanner, avatar_url, created_at, updated_at
           FROM users"#
    )
    .fetch_all(&pool)
    .await?;

    let skills = sqlx::query_as!(
        SkillExport,
        r#"SELECT skill_id as "skill_id!: SkillId",
                  file_path, name, description, instructions, enabled,
                  tags, category_id,
                  source_id as "source_id!: SourceId",
                  created_at, updated_at
           FROM agent_skills"#
    )
    .fetch_all(&pool)
    .await?;

    let contexts = sqlx::query_as!(
        ContextExport,
        r#"SELECT context_id as "context_id!: ContextId",
                  user_id as "user_id!: UserId",
                  session_id as "session_id: SessionId",
                  name, created_at, updated_at
           FROM user_contexts"#
    )
    .fetch_all(&pool)
    .await?;

    Ok(DatabaseExport {
        users,
        skills,
        contexts,
        timestamp: Utc::now(),
    })
}

async fn import_to_database(
    database_url: &str,
    export: &DatabaseExport,
) -> SyncResult<ImportResult> {
    let pool = PgPool::connect(database_url).await?;
    let mut created = 0;
    let mut updated = 0;
    let mut completed = 0usize;
    let total = export.users.len() + export.skills.len() + export.contexts.len();

    for user in &export.users {
        match upsert_user(&pool, user).await {
            Ok((c, u)) => {
                created += c;
                updated += u;
                completed += 1;
            },
            Err(err) => {
                return Err(crate::error::SyncError::PartialImport {
                    completed,
                    total,
                    message: format!("user upsert failed: {err}"),
                });
            },
        }
    }

    for skill in &export.skills {
        match upsert_skill(&pool, skill).await {
            Ok((c, u)) => {
                created += c;
                updated += u;
                completed += 1;
            },
            Err(err) => {
                return Err(crate::error::SyncError::PartialImport {
                    completed,
                    total,
                    message: format!("skill upsert failed: {err}"),
                });
            },
        }
    }

    for context in &export.contexts {
        match upsert_context(&pool, context).await {
            Ok((c, u)) => {
                created += c;
                updated += u;
                completed += 1;
            },
            Err(err) => {
                return Err(crate::error::SyncError::PartialImport {
                    completed,
                    total,
                    message: format!("context upsert failed: {err}"),
                });
            },
        }
    }

    Ok(ImportResult {
        created,
        updated,
        skipped: 0,
    })
}
