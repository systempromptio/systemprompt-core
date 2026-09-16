//! Rebuild snapshot: a share lock over every source table and a server-side
//! cursor over each owner's reporting view.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::Value;
use sqlx::PgConnection;

use super::{SOURCE_DEFINITIONS, SourceDefinition};
use crate::Result;

/// One owner-published reporting row read from a rebuild snapshot cursor.
#[derive(Debug, sqlx::FromRow)]
pub struct SnapshotRow {
    pub entity_key: String,
    // JSON: owner-published reporting views provide the versioned row payload.
    pub row: Value,
}

/// Server-side cursor over one source's reporting view, held open for the
/// rebuild transaction so the snapshot is read in bounded batches.
#[derive(Debug, Clone, Copy)]
pub struct SnapshotCursor {
    definition: &'static SourceDefinition,
}

impl SnapshotCursor {
    pub async fn lock_sources(connection: &mut PgConnection) -> Result<()> {
        let tables = SOURCE_DEFINITIONS
            .iter()
            .map(|definition| definition.table)
            .collect::<Vec<_>>()
            .join(", ");
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "LOCK TABLE {tables} IN SHARE MODE"
        )))
        .execute(connection)
        .await?;
        Ok(())
    }

    pub async fn open(
        connection: &mut PgConnection,
        definition: &'static SourceDefinition,
    ) -> Result<Self> {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "DECLARE reporting_snapshot NO SCROLL CURSOR FOR SELECT entity_key, row FROM {}",
            definition.view,
        )))
        .execute(connection)
        .await?;
        Ok(Self { definition })
    }

    pub const fn definition(&self) -> &'static SourceDefinition {
        self.definition
    }

    pub async fn fetch(&self, connection: &mut PgConnection) -> Result<Vec<SnapshotRow>> {
        Ok(sqlx::query_as("FETCH FORWARD 1000 FROM reporting_snapshot")
            .fetch_all(connection)
            .await?)
    }

    pub async fn close(self, connection: &mut PgConnection) -> Result<()> {
        sqlx::query("CLOSE reporting_snapshot")
            .execute(connection)
            .await?;
        Ok(())
    }
}
