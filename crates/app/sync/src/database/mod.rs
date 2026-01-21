mod upsert;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;
use sqlx::PgPool;

use crate::error::SyncResult;
use crate::{SyncDirection, SyncOperationResult};

use upsert::{upsert_context, upsert_skill, upsert_user};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DatabaseExport {
    pub users: Vec<UserExport>,
    pub skills: Vec<SkillExport>,
    pub contexts: Vec<ContextExport>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, FromRow)]
pub struct UserExport {
    pub id: String,
    pub name: String,
    pub email: String,
    pub full_name: Option<String>,
    pub display_name: Option<String>,
    pub status: String,
    pub email_verified: bool,
    pub roles: Vec<String>,
    pub is_bot: bool,
    pub is_scanner: bool,
    pub avatar_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, FromRow)]
pub struct SkillExport {
    pub skill_id: String,
    pub file_path: String,
    pub name: String,
    pub description: String,
    pub instructions: String,
    pub enabled: bool,
    pub tags: Option<Vec<String>>,
    pub category_id: Option<String>,
    pub source_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, FromRow)]
pub struct ContextExport {
    pub context_id: String,
    pub user_id: String,
    pub session_id: Option<String>,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ImportResult {
    pub created: usize,
    pub updated: usize,
    pub skipped: usize,
}

#[derive(Debug)]
pub struct DatabaseSyncService {
    direction: SyncDirection,
    dry_run: bool,
    local_database_url: String,
    cloud_database_url: String,
}

impl DatabaseSyncService {
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
        r#"SELECT skill_id, file_path, name, description, instructions, enabled,
                  tags, category_id, source_id, created_at, updated_at
           FROM agent_skills"#
    )
    .fetch_all(&pool)
    .await?;

    let contexts = sqlx::query_as!(
        ContextExport,
        r#"SELECT context_id, user_id, session_id, name, created_at, updated_at
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

    for user in &export.users {
        let (c, u) = upsert_user(&pool, user).await?;
        created += c;
        updated += u;
    }

    for skill in &export.skills {
        let (c, u) = upsert_skill(&pool, skill).await?;
        created += c;
        updated += u;
    }

    for context in &export.contexts {
        let (c, u) = upsert_context(&pool, context).await?;
        created += c;
        updated += u;
    }

    Ok(ImportResult {
        created,
        updated,
        skipped: 0,
    })
}
