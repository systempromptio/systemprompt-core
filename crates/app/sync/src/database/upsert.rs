use sqlx::PgPool;

use super::{ContextExport, SkillExport, UserExport};
use crate::error::SyncResult;

pub(super) async fn upsert_user(pool: &PgPool, user: &UserExport) -> SyncResult<(usize, usize)> {
    let conflict_exists: Option<bool> = sqlx::query_scalar!(
        "SELECT EXISTS(SELECT 1 FROM users WHERE (name = $1 OR email = $2) AND id != $3)",
        user.name,
        user.email,
        user.id
    )
    .fetch_one(pool)
    .await?;

    if conflict_exists == Some(true) {
        tracing::debug!(
            user_id = %user.id,
            name = %user.name,
            email = %user.email,
            "User with same name or email exists with different id, skipping"
        );
        return Ok((0, 0));
    }

    let result = sqlx::query!(
        r#"INSERT INTO users (id, name, email, full_name, display_name, status, email_verified,
                              roles, is_bot, is_scanner, avatar_url, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
           ON CONFLICT (id) DO UPDATE SET
             name = EXCLUDED.name,
             email = EXCLUDED.email,
             full_name = EXCLUDED.full_name,
             display_name = EXCLUDED.display_name,
             status = EXCLUDED.status,
             email_verified = EXCLUDED.email_verified,
             roles = EXCLUDED.roles,
             is_bot = EXCLUDED.is_bot,
             is_scanner = EXCLUDED.is_scanner,
             avatar_url = EXCLUDED.avatar_url,
             updated_at = EXCLUDED.updated_at"#,
        user.id,
        user.name,
        user.email,
        user.full_name,
        user.display_name,
        user.status,
        user.email_verified,
        &user.roles,
        user.is_bot,
        user.is_scanner,
        user.avatar_url,
        user.created_at,
        user.updated_at
    )
    .execute(pool)
    .await?;

    if result.rows_affected() > 0 && user.created_at == user.updated_at {
        Ok((1, 0))
    } else if result.rows_affected() > 0 {
        Ok((0, 1))
    } else {
        Ok((0, 0))
    }
}

pub(super) async fn upsert_skill(pool: &PgPool, skill: &SkillExport) -> SyncResult<(usize, usize)> {
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

pub(super) async fn upsert_context(
    pool: &PgPool,
    context: &ContextExport,
) -> SyncResult<(usize, usize)> {
    let user_exists: Option<bool> = sqlx::query_scalar!(
        "SELECT EXISTS(SELECT 1 FROM users WHERE id = $1)",
        context.user_id
    )
    .fetch_one(pool)
    .await?;

    if user_exists != Some(true) {
        tracing::debug!(
            user_id = %context.user_id,
            context_id = %context.context_id,
            "User not found in target database, skipping context"
        );
        return Ok((0, 0));
    }

    let session_id = match &context.session_id {
        Some(sid) => {
            let exists: Option<bool> = sqlx::query_scalar!(
                "SELECT EXISTS(SELECT 1 FROM user_sessions WHERE session_id = $1)",
                sid
            )
            .fetch_one(pool)
            .await?;

            if exists == Some(true) {
                Some(sid.clone())
            } else {
                tracing::debug!(
                    session_id = %sid,
                    context_id = %context.context_id,
                    "Session not found in target database, setting session_id to NULL"
                );
                None
            }
        },
        None => None,
    };

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
        session_id,
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
