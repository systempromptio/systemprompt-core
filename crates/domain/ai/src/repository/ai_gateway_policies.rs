//! Repository for `ai_gateway_policies` rows.
//!
//! The pool-backed methods serve the request path. The `*_on` associated
//! functions run on a caller's connection: the `gateway_policies` config
//! plane reads and writes through them inside the applier's transaction, so
//! the rows, their change context and the audit commit together.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::Value;
use sqlx::{PgConnection, PgPool};
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::AiGatewayPolicyId;
use systemprompt_traits::RepositoryError;

#[must_use]
#[derive(Debug, Clone)]
pub struct AiGatewayPolicyRepository {
    pool: Arc<PgPool>,
}

#[derive(Debug, Clone)]
pub struct GatewayPolicyRow {
    pub id: AiGatewayPolicyId,
    pub name: String,
    // JSON: JSONB `spec` column — decoded into the policy spec by the gateway service.
    pub spec: Value,
    pub enabled: bool,
    pub priority: i32,
}

impl AiGatewayPolicyRepository {
    pub fn new(db: &DbPool) -> Self {
        Self { pool: db.pool() }
    }

    pub async fn list_for_global(&self) -> Result<Vec<GatewayPolicyRow>, RepositoryError> {
        let rows = sqlx::query!(
            r#"
            SELECT id as "id!: AiGatewayPolicyId", name, spec, enabled, priority
            FROM ai_gateway_policies
            WHERE enabled = TRUE
            ORDER BY priority ASC, name ASC
            "#,
        )
        .fetch_all(self.pool.as_ref())
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| GatewayPolicyRow {
                id: r.id,
                name: r.name,
                spec: r.spec,
                enabled: r.enabled,
                priority: r.priority,
            })
            .collect())
    }

    pub async fn list_all_on(
        conn: &mut PgConnection,
    ) -> Result<Vec<GatewayPolicyRow>, RepositoryError> {
        let rows = sqlx::query!(
            r#"
            SELECT id as "id!: AiGatewayPolicyId", name, spec, enabled, priority
            FROM ai_gateway_policies
            ORDER BY name
            "#,
        )
        .fetch_all(conn)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| GatewayPolicyRow {
                id: r.id,
                name: r.name,
                spec: r.spec,
                enabled: r.enabled,
                priority: r.priority,
            })
            .collect())
    }

    // JSON: JSONB `spec` column — the serialised policy spec, stored as written.
    pub async fn upsert_on(
        conn: &mut PgConnection,
        row: &GatewayPolicyWrite<'_>,
    ) -> Result<(), RepositoryError> {
        let id = AiGatewayPolicyId::generate();
        sqlx::query!(
            r#"
            INSERT INTO ai_gateway_policies
                (id, name, spec, enabled, priority, source, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            ON CONFLICT (name) DO UPDATE
            SET spec = EXCLUDED.spec,
                enabled = EXCLUDED.enabled,
                priority = EXCLUDED.priority,
                source = EXCLUDED.source,
                updated_at = CURRENT_TIMESTAMP
            "#,
            id.as_str(),
            row.name,
            row.spec,
            row.enabled,
            row.priority,
            row.source,
        )
        .execute(conn)
        .await?;
        Ok(())
    }

    pub async fn delete_on(conn: &mut PgConnection, name: &str) -> Result<(), RepositoryError> {
        sqlx::query!("DELETE FROM ai_gateway_policies WHERE name = $1", name)
            .execute(conn)
            .await?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GatewayPolicyWrite<'a> {
    pub name: &'a str,
    // JSON: JSONB `spec` column — the serialised policy spec, stored as written.
    pub spec: &'a Value,
    pub enabled: bool,
    pub priority: i32,
    pub source: &'a str,
}
