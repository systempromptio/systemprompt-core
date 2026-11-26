//! Bounded, keyed reads and compare-and-swap writes over one declared
//! ciphertext column, for `admin secrets reseal`.
//!
//! The table, key and column names come from a member's admitted
//! `ResealColumn` declaration, so the SQL is necessarily built at runtime;
//! every name is a [`SafeIdentifier`] and rendered quoted. Rows are paged in
//! key order (each key column cast to text, compared as one array), and a
//! batch of replacements is written in one transaction, each row only if its
//! ciphertext is still the value that was read, so a concurrent writer's
//! value is never overwritten; with a nonce column the nonce that was read is
//! part of that comparison. A `TEXT` ciphertext column (no nonce column) is
//! read and written as its UTF-8 bytes. A key with a NULL column is named like
//! any other: Postgres array equality treats two NULL elements as equal, so
//! the compare-and-swap matches the row it read, and only a row still holding
//! the ciphertext (and nonce) that was read is written.
//! [`ResealStore::recorded_owner`] reads the member the installer recorded for
//! a table. The store targets one [`SchemaName`]: a declared table is named
//! by that schema everywhere, qualified the way the installer records it
//! ([`SchemaName::qualify`]) when the owner record is read and rendered
//! `"schema"."table"` in every read and write.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use sqlx::{PgPool, Row};

use super::identifier::{IdentifierError, SafeIdentifier};
use crate::lifecycle::SchemaName;

/// One declared ciphertext column, its table and the key that names a row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResealTable {
    table: SafeIdentifier,
    keys: Vec<SafeIdentifier>,
    ciphertext: SafeIdentifier,
    nonce: Option<SafeIdentifier>,
}

impl ResealTable {
    pub fn new(
        table: &str,
        keys: &[&str],
        ciphertext: &str,
        nonce: Option<&str>,
    ) -> Result<Self, IdentifierError> {
        if keys.is_empty() {
            return Err(IdentifierError::Empty);
        }
        Ok(Self {
            table: SafeIdentifier::parse(table)?,
            keys: keys
                .iter()
                .map(|key| SafeIdentifier::parse(key))
                .collect::<Result<_, _>>()?,
            ciphertext: SafeIdentifier::parse(ciphertext)?,
            nonce: nonce.map(SafeIdentifier::parse).transpose()?,
        })
    }

    #[must_use]
    pub fn table(&self) -> &str {
        self.table.as_str()
    }

    #[must_use]
    pub fn ciphertext(&self) -> &str {
        self.ciphertext.as_str()
    }

    fn read_ciphertext(&self) -> String {
        match self.nonce {
            Some(_) => self.ciphertext.quoted(),
            None => format!("convert_to({}, 'UTF8')", self.ciphertext.quoted()),
        }
    }

    fn read_nonce(&self) -> String {
        self.nonce
            .as_ref()
            .map_or_else(|| "''::bytea".to_owned(), SafeIdentifier::quoted)
    }

    fn write_value(&self, param: &str) -> String {
        match self.nonce {
            Some(_) => param.to_owned(),
            None => format!("convert_from({param}, 'UTF8')"),
        }
    }

    fn key_array(&self) -> String {
        let keys: Vec<String> = self
            .keys
            .iter()
            .map(|key| format!("{}::text", key.quoted()))
            .collect();
        format!("ARRAY[{}]", keys.join(", "))
    }
}

/// One row's key (each column as text, `None` where the column is NULL),
/// ciphertext and nonce.
#[derive(Clone, PartialEq, Eq)]
pub struct SealedRow {
    pub keys: Vec<Option<String>>,
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
}

impl std::fmt::Debug for SealedRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SealedRow")
            .field("keys", &self.keys)
            .finish_non_exhaustive()
    }
}

/// A row and the ciphertext that replaces the one it was read with.
#[derive(Debug, Clone)]
pub struct Replacement<'a> {
    pub row: &'a SealedRow,
    pub ciphertext: Vec<u8>,
}

/// Reads and rewrites declared ciphertext columns.
#[derive(Debug, Clone)]
pub struct ResealStore {
    pool: Arc<PgPool>,
    schema: SchemaName,
}

impl ResealStore {
    #[must_use]
    pub const fn new(pool: Arc<PgPool>, schema: SchemaName) -> Self {
        Self { pool, schema }
    }

    pub async fn recorded_owner(&self, table: &str) -> Result<Option<String>, sqlx::Error> {
        sqlx::query_scalar(sqlx::AssertSqlSafe(
            "SELECT extension_id FROM schema_table_owners WHERE table_name = $1",
        ))
        .bind(self.schema.qualify(table))
        .fetch_optional(self.pool.as_ref())
        .await
    }

    pub async fn page(
        &self,
        target: &ResealTable,
        after: Option<&[Option<String>]>,
        limit: i64,
    ) -> Result<Vec<SealedRow>, sqlx::Error> {
        let keys = target.key_array();
        let sql = format!(
            "SELECT {keys} AS keys, {ciphertext} AS ciphertext, {nonce} AS nonce FROM {table} \
             WHERE $1::text[] IS NULL OR {keys} > $1::text[] ORDER BY {keys} LIMIT $2",
            ciphertext = target.read_ciphertext(),
            nonce = target.read_nonce(),
            table = self.relation(target),
        );
        let rows = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(after.map(<[Option<String>]>::to_vec))
            .bind(limit)
            .fetch_all(self.pool.as_ref())
            .await?;
        rows.iter()
            .map(|row| {
                Ok(SealedRow {
                    keys: row.try_get("keys")?,
                    ciphertext: row.try_get("ciphertext")?,
                    nonce: row.try_get("nonce")?,
                })
            })
            .collect()
    }

    pub async fn replace(
        &self,
        target: &ResealTable,
        replacements: &[Replacement<'_>],
    ) -> Result<u64, sqlx::Error> {
        let sql = format!(
            "UPDATE {table} SET {ciphertext} = {new} WHERE {keys} = $2::text[] AND {ciphertext} = \
             {old} AND {nonce} = $4",
            table = self.relation(target),
            ciphertext = target.ciphertext.quoted(),
            new = target.write_value("$1"),
            old = target.write_value("$3"),
            keys = target.key_array(),
            nonce = target.read_nonce(),
        );
        let mut tx = self.pool.begin().await?;
        let mut written = 0;
        for replacement in replacements {
            written += sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
                .bind(&replacement.ciphertext)
                .bind(&replacement.row.keys)
                .bind(&replacement.row.ciphertext)
                .bind(&replacement.row.nonce)
                .execute(&mut *tx)
                .await?
                .rows_affected();
        }
        tx.commit().await?;
        Ok(written)
    }

    fn relation(&self, target: &ResealTable) -> String {
        format!("\"{}\".{}", self.schema.as_str(), target.table.quoted())
    }
}
