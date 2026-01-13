use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;
use sqlx::PgPool;

use crate::error::SyncResult;
use crate::{SyncDirection, SyncOperationResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseExport {
    pub skills: Vec<SkillExport>,
    pub contexts: Vec<ContextExport>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
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

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ContextExport {
    pub context_id: String,
    pub user_id: String,
    pub session_id: Option<String>,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
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
        let count = export.skills.len() + export.contexts.len();

        if self.dry_run {
            return Ok(SyncOperationResult::dry_run(
                "database_push",
                count,
                serde_json::json!({
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
        let count = export.skills.len() + export.contexts.len();

        if self.dry_run {
            return Ok(SyncOperationResult::dry_run(
                "database_pull",
                count,
                serde_json::json!({
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
        skills,
        contexts,
        timestamp: Utc::now(),
    })
}

async fn import_to_database(database_url: &str, export: &DatabaseExport) -> SyncResult<ImportResult> {
    let pool = PgPool::connect(database_url).await?;
    let mut created = 0;
    let mut updated = 0;

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

async fn upsert_skill(pool: &PgPool, skill: &SkillExport) -> SyncResult<(usize, usize)> {
    let result = sqlx::query!(
        r#"INSERT INTO agent_skills (skill_id, file_path, name, description, instructions,
                                     enabled, tags, category_id, source_id, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
           ON CONFLICT (skill_id) DO UPDATE SET
             file_path = EXCLUDED.file_path,
             name = EXCLUDED.name,
             description = EXCLUDED.description,
             instructions = EXCLUDED.instructions,
             enabled = EXCLUDED.enabled,
             tags = EXCLUDED.tags,
             category_id = EXCLUDED.category_id,
             source_id = EXCLUDED.source_id,
             updated_at = EXCLUDED.updated_at"#,
        skill.skill_id,
        skill.file_path,
        skill.name,
        skill.description,
        skill.instructions,
        skill.enabled,
        skill.tags.as_deref(),
        skill.category_id,
        skill.source_id,
        skill.created_at,
        skill.updated_at
    )
    .execute(pool)
    .await?;

    if result.rows_affected() > 0 && skill.created_at == skill.updated_at {
        Ok((1, 0))
    } else if result.rows_affected() > 0 {
        Ok((0, 1))
    } else {
        Ok((0, 0))
    }
}

async fn upsert_context(pool: &PgPool, context: &ContextExport) -> SyncResult<(usize, usize)> {
    let result = sqlx::query!(
        r#"INSERT INTO user_contexts (context_id, user_id, session_id, name, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, $6)
           ON CONFLICT (context_id) DO UPDATE SET
             user_id = EXCLUDED.user_id,
             session_id = EXCLUDED.session_id,
             name = EXCLUDED.name,
             updated_at = EXCLUDED.updated_at"#,
        context.context_id,
        context.user_id,
        context.session_id,
        context.name,
        context.created_at,
        context.updated_at
    )
    .execute(pool)
    .await?;

    if result.rows_affected() > 0 && context.created_at == context.updated_at {
        Ok((1, 0))
    } else if result.rows_affected() > 0 {
        Ok((0, 1))
    } else {
        Ok((0, 0))
    }
}
