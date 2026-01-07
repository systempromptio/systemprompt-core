use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;
use sqlx::PgPool;

use crate::api_client::SyncApiClient;
use crate::error::SyncResult;
use crate::{SyncConfig, SyncDirection, SyncOperationResult};

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
    config: SyncConfig,
    api_client: SyncApiClient,
    database_url: String,
}

impl DatabaseSyncService {
    pub fn new(config: SyncConfig, api_client: SyncApiClient, database_url: &str) -> Self {
        Self {
            config,
            api_client,
            database_url: database_url.to_string(),
        }
    }

    pub async fn sync(&self) -> SyncResult<SyncOperationResult> {
        match self.config.direction {
            SyncDirection::Push => self.push().await,
            SyncDirection::Pull => self.pull().await,
        }
    }

    async fn push(&self) -> SyncResult<SyncOperationResult> {
        let export = self.export_local().await?;
        let count = export.skills.len() + export.contexts.len();

        if self.config.dry_run {
            return Ok(SyncOperationResult::dry_run(
                "database_push",
                count,
                serde_json::json!({
                    "skills": export.skills.len(),
                    "contexts": export.contexts.len(),
                }),
            ));
        }

        self.api_client
            .import_database(&self.config.tenant_id, &export)
            .await?;

        Ok(SyncOperationResult::success("database_push", count))
    }

    async fn pull(&self) -> SyncResult<SyncOperationResult> {
        let export = self
            .api_client
            .export_database(&self.config.tenant_id)
            .await?;

        let count = export.skills.len() + export.contexts.len();

        if self.config.dry_run {
            return Ok(SyncOperationResult::dry_run(
                "database_pull",
                count,
                serde_json::json!({
                    "skills": export.skills.len(),
                    "contexts": export.contexts.len(),
                }),
            ));
        }

        self.import_local(&export).await?;
        Ok(SyncOperationResult::success("database_pull", count))
    }

    async fn export_local(&self) -> SyncResult<DatabaseExport> {
        let pool = PgPool::connect(&self.database_url).await?;

        let skills: Vec<SkillExport> = sqlx::query_as(
            r"SELECT skill_id, file_path, name, description, instructions, enabled,
                     tags, category_id, source_id, created_at, updated_at
              FROM agent_skills",
        )
        .fetch_all(&pool)
        .await?;

        let contexts: Vec<ContextExport> = sqlx::query_as(
            r"SELECT context_id, user_id, session_id, name, created_at, updated_at
              FROM user_contexts",
        )
        .fetch_all(&pool)
        .await?;

        Ok(DatabaseExport {
            skills,
            contexts,
            timestamp: Utc::now(),
        })
    }

    async fn import_local(&self, export: &DatabaseExport) -> SyncResult<ImportResult> {
        let pool = PgPool::connect(&self.database_url).await?;
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
}

async fn upsert_skill(pool: &PgPool, skill: &SkillExport) -> SyncResult<(usize, usize)> {
    let result = sqlx::query(
        r"INSERT INTO agent_skills (skill_id, file_path, name, description, instructions,
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
            updated_at = EXCLUDED.updated_at",
    )
    .bind(&skill.skill_id)
    .bind(&skill.file_path)
    .bind(&skill.name)
    .bind(&skill.description)
    .bind(&skill.instructions)
    .bind(skill.enabled)
    .bind(skill.tags.as_deref())
    .bind(&skill.category_id)
    .bind(&skill.source_id)
    .bind(skill.created_at)
    .bind(skill.updated_at)
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
    let result = sqlx::query(
        r"INSERT INTO user_contexts (context_id, user_id, session_id, name, created_at, updated_at)
          VALUES ($1, $2, $3, $4, $5, $6)
          ON CONFLICT (context_id) DO UPDATE SET
            user_id = EXCLUDED.user_id,
            session_id = EXCLUDED.session_id,
            name = EXCLUDED.name,
            updated_at = EXCLUDED.updated_at",
    )
    .bind(&context.context_id)
    .bind(&context.user_id)
    .bind(&context.session_id)
    .bind(&context.name)
    .bind(context.created_at)
    .bind(context.updated_at)
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
