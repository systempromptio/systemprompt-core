use crate::models::{Playbook, PlaybookRow};
use anyhow::{Context, Result};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{PlaybookId, SourceId};

#[derive(Debug)]
pub struct PlaybookRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
}

impl PlaybookRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        let pool = db.pool_arc().context("PostgreSQL pool not available")?;
        let write_pool = db
            .write_pool_arc()
            .context("Write PostgreSQL pool not available")?;
        Ok(Self { pool, write_pool })
    }

    pub async fn create(&self, playbook: &Playbook) -> Result<()> {
        let pool = &self.write_pool;
        let playbook_id_str = playbook.playbook_id.as_str();
        let source_id_str = playbook.source_id.as_str();

        sqlx::query!(
            "INSERT INTO agent_playbooks (playbook_id, file_path, name, description, instructions,
             enabled, tags, category, domain, source_id)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
            playbook_id_str,
            playbook.file_path,
            playbook.name,
            playbook.description,
            playbook.instructions,
            playbook.enabled,
            &playbook.tags[..],
            playbook.category,
            playbook.domain,
            source_id_str
        )
        .execute(pool.as_ref())
        .await
        .context(format!("Failed to create playbook: {}", playbook.name))?;

        Ok(())
    }

    pub async fn get_by_playbook_id(&self, playbook_id: &PlaybookId) -> Result<Option<Playbook>> {
        let pool = &self.pool;
        let playbook_id_str = playbook_id.as_str();

        let row = sqlx::query_as!(
            PlaybookRow,
            r#"SELECT
                playbook_id as "playbook_id!: PlaybookId",
                file_path as "file_path!",
                name as "name!",
                description as "description!",
                instructions as "instructions!",
                enabled as "enabled!",
                tags,
                category as "category!",
                domain as "domain!",
                source_id as "source_id!: SourceId",
                created_at as "created_at!",
                updated_at as "updated_at!"
            FROM agent_playbooks WHERE playbook_id = $1"#,
            playbook_id_str
        )
        .fetch_optional(pool.as_ref())
        .await
        .context(format!("Failed to get playbook by id: {playbook_id}"))?;

        row.map(playbook_from_row).transpose()
    }

    pub async fn get_by_file_path(&self, file_path: &str) -> Result<Option<Playbook>> {
        let pool = &self.pool;

        let row = sqlx::query_as!(
            PlaybookRow,
            r#"SELECT
                playbook_id as "playbook_id!: PlaybookId",
                file_path as "file_path!",
                name as "name!",
                description as "description!",
                instructions as "instructions!",
                enabled as "enabled!",
                tags,
                category as "category!",
                domain as "domain!",
                source_id as "source_id!: SourceId",
                created_at as "created_at!",
                updated_at as "updated_at!"
            FROM agent_playbooks WHERE file_path = $1"#,
            file_path
        )
        .fetch_optional(pool.as_ref())
        .await
        .context(format!("Failed to get playbook by file path: {file_path}"))?;

        row.map(playbook_from_row).transpose()
    }

    pub async fn list_enabled(&self) -> Result<Vec<Playbook>> {
        let pool = &self.pool;

        let rows = sqlx::query_as!(
            PlaybookRow,
            r#"SELECT
                playbook_id as "playbook_id!: PlaybookId",
                file_path as "file_path!",
                name as "name!",
                description as "description!",
                instructions as "instructions!",
                enabled as "enabled!",
                tags,
                category as "category!",
                domain as "domain!",
                source_id as "source_id!: SourceId",
                created_at as "created_at!",
                updated_at as "updated_at!"
            FROM agent_playbooks WHERE enabled = true ORDER BY category, domain ASC"#
        )
        .fetch_all(pool.as_ref())
        .await
        .context("Failed to list enabled playbooks")?;

        rows.into_iter()
            .map(playbook_from_row)
            .collect::<Result<Vec<_>>>()
    }

    pub async fn list_all(&self) -> Result<Vec<Playbook>> {
        let pool = &self.pool;

        let rows = sqlx::query_as!(
            PlaybookRow,
            r#"SELECT
                playbook_id as "playbook_id!: PlaybookId",
                file_path as "file_path!",
                name as "name!",
                description as "description!",
                instructions as "instructions!",
                enabled as "enabled!",
                tags,
                category as "category!",
                domain as "domain!",
                source_id as "source_id!: SourceId",
                created_at as "created_at!",
                updated_at as "updated_at!"
            FROM agent_playbooks ORDER BY category, domain ASC"#
        )
        .fetch_all(pool.as_ref())
        .await
        .context("Failed to list all playbooks")?;

        rows.into_iter()
            .map(playbook_from_row)
            .collect::<Result<Vec<_>>>()
    }

    pub async fn list_by_category(&self, category: &str) -> Result<Vec<Playbook>> {
        let pool = &self.pool;

        let rows = sqlx::query_as!(
            PlaybookRow,
            r#"SELECT
                playbook_id as "playbook_id!: PlaybookId",
                file_path as "file_path!",
                name as "name!",
                description as "description!",
                instructions as "instructions!",
                enabled as "enabled!",
                tags,
                category as "category!",
                domain as "domain!",
                source_id as "source_id!: SourceId",
                created_at as "created_at!",
                updated_at as "updated_at!"
            FROM agent_playbooks WHERE category = $1 ORDER BY domain ASC"#,
            category
        )
        .fetch_all(pool.as_ref())
        .await
        .context(format!("Failed to list playbooks by category: {category}"))?;

        rows.into_iter()
            .map(playbook_from_row)
            .collect::<Result<Vec<_>>>()
    }

    pub async fn update(&self, playbook_id: &PlaybookId, playbook: &Playbook) -> Result<()> {
        let pool = &self.write_pool;
        let playbook_id_str = playbook_id.as_str();

        sqlx::query!(
            "UPDATE agent_playbooks SET name = $1, description = $2, instructions = $3, enabled = \
             $4,
             tags = $5, updated_at = CURRENT_TIMESTAMP
             WHERE playbook_id = $6",
            playbook.name,
            playbook.description,
            playbook.instructions,
            playbook.enabled,
            &playbook.tags[..],
            playbook_id_str
        )
        .execute(pool.as_ref())
        .await
        .context(format!("Failed to update playbook: {}", playbook.name))?;

        Ok(())
    }

    pub async fn delete(&self, playbook_id: &PlaybookId) -> Result<()> {
        let pool = &self.write_pool;
        let playbook_id_str = playbook_id.as_str();

        sqlx::query!(
            "DELETE FROM agent_playbooks WHERE playbook_id = $1",
            playbook_id_str
        )
        .execute(pool.as_ref())
        .await
        .context(format!("Failed to delete playbook: {playbook_id}"))?;

        Ok(())
    }
}

fn playbook_from_row(row: PlaybookRow) -> Result<Playbook> {
    Ok(Playbook {
        playbook_id: row.playbook_id,
        file_path: row.file_path,
        name: row.name,
        description: row.description,
        instructions: row.instructions,
        enabled: row.enabled,
        tags: row.tags.unwrap_or_else(Vec::new),
        category: row.category,
        domain: row.domain,
        source_id: row.source_id,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}
