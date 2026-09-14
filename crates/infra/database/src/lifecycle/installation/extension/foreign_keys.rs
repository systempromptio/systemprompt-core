//! The final installation phase: foreign keys deferred out of the
//! declarative `CREATE TABLE`s, applied after every extension's migrations
//! and dependent DDL so the unique indexes they need exist on legacy
//! databases too.
//!
//! Each key is skipped when `pg_constraint` already holds a foreign key on
//! the same columns to the same referenced columns — whatever its name — so
//! a key a migration authored under its own name is honoured rather than
//! duplicated. A key that is missing is added `NOT VALID` and then validated
//! inside a savepoint: on a fresh database that always succeeds and the
//! result is indistinguishable from an inline declaration; on an existing
//! database whose `CREATE TABLE IF NOT EXISTS` was a no-op the key may never
//! have existed, and rows that violate it must not turn a boot into an
//! outage — the key stays `NOT VALID`, enforced for new rows, and the warning
//! names it so an operator can repair the data and validate it.
//!
//! A key that cannot be created at all — the referenced uniqueness is
//! missing — is a schema bug on a fresh database, where the declarative
//! schema alone ran, and installation fails naming the table. On an
//! established database it is pre-existing drift: the inline declaration
//! never took effect there either, so the key is reported at error level and
//! the boot continues; the fix is a migration that adds the referenced
//! unique index, and the upgrade gate downstream diffs the two shapes.
//!
//! `FOREIGN_KEY_EXISTS_SQL`: A foreign key with the same constrained and
//! referenced columns, by name rather than attnum so it holds across databases
//! whose column numbering differs. An empty `$4` stands for the referenced
//! primary key.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::LoaderError;
use systemprompt_identifiers::ToDbValue;
use tracing::{debug, error, warn};

use super::super::fk_deferral::DeferredForeignKey;
use crate::models::DatabaseTransaction;
use crate::services::DatabaseProvider;

const FOREIGN_KEY_EXISTS_SQL: &str = "SELECT 1
FROM pg_constraint c
WHERE c.contype = 'f'
  AND c.conrelid = to_regclass($1)
  AND c.confrelid = to_regclass($2)
  AND (SELECT array_agg(a.attname::text ORDER BY k.ord)
         FROM unnest(c.conkey) WITH ORDINALITY AS k(attnum, ord)
         JOIN pg_attribute a ON a.attrelid = c.conrelid AND a.attnum = k.attnum) = $3::text[]
  AND (
        (SELECT array_agg(a.attname::text ORDER BY k.ord)
           FROM unnest(c.confkey) WITH ORDINALITY AS k(attnum, ord)
           JOIN pg_attribute a ON a.attrelid = c.confrelid AND a.attnum = k.attnum) = $4::text[]
     OR (cardinality($4::text[]) = 0
         AND c.confkey = (SELECT p.conkey FROM pg_constraint p
                           WHERE p.conrelid = c.confrelid AND p.contype = 'p'))
  )
LIMIT 1";

pub(super) async fn apply_foreign_keys(
    db: &dyn DatabaseProvider,
    keys: &[DeferredForeignKey],
    extension_id: &str,
    fresh: bool,
) -> Result<(), LoaderError> {
    if keys.is_empty() {
        return Ok(());
    }

    let failed = |message: String| LoaderError::SchemaInstallationFailed {
        extension: extension_id.to_owned(),
        message,
    };

    let mut tx = db
        .begin_transaction()
        .await
        .map_err(|e| failed(format!("Failed to begin transaction: {e}")))?;

    let total = keys.len();
    for (idx, key) in keys.iter().enumerate() {
        let Err(e) = apply_one(tx.as_mut(), key, extension_id).await else {
            continue;
        };
        let explanation = format!(
            "Foreign key {n}/{total} failed: {e}\n\
             This FOREIGN KEY was declared inline on CREATE TABLE {table} and is applied \
             after migrations; the referenced table must expose a PRIMARY KEY or UNIQUE \
             constraint on ({referenced}) — declare it in the referenced CREATE TABLE and \
             converge existing databases with a migration.\nSQL:\n{sql}",
            n = idx + 1,
            table = key.source_table,
            referenced = key.referenced_columns.join(", "),
            sql = key.sql,
        );
        if fresh {
            let rollback_note = match tx.rollback().await {
                Ok(()) => String::new(),
                Err(rb) => format!(" (rollback also failed: {rb})"),
            };
            return Err(failed(format!("{explanation}{rollback_note}")));
        }
        error!(
            extension = extension_id,
            table = %key.source_table,
            constraint = %key.constraint_name,
            "Declared foreign key is absent on this established database and cannot be \
             created; add the referenced unique index with a migration. {explanation}"
        );
    }

    tx.commit()
        .await
        .map_err(|e| failed(format!("Failed to commit transaction: {e}")))?;
    Ok(())
}

async fn apply_one(
    tx: &mut dyn DatabaseTransaction,
    key: &DeferredForeignKey,
    extension_id: &str,
) -> Result<(), crate::error::RepositoryError> {
    let params: [&dyn ToDbValue; 4] = [
        &key.table,
        &key.referenced_table,
        &key.columns,
        &key.referenced_columns,
    ];
    if tx
        .fetch_optional(&FOREIGN_KEY_EXISTS_SQL, &params)
        .await?
        .is_some()
    {
        debug!(
            extension = extension_id,
            table = %key.source_table,
            constraint = %key.constraint_name,
            "Foreign key already present; skipping"
        );
        return Ok(());
    }

    // Why: a failed statement aborts the transaction; the savepoints keep the
    // other keys of this extension applicable whatever happens to this one.
    let add = format!("{} NOT VALID", key.sql);
    tx.execute(&"SAVEPOINT deferred_fk_add", &[]).await?;
    if let Err(e) = tx.execute(&add.as_str(), &[]).await {
        tx.execute(&"ROLLBACK TO SAVEPOINT deferred_fk_add", &[])
            .await?;
        return Err(e);
    }
    tx.execute(&"RELEASE SAVEPOINT deferred_fk_add", &[])
        .await?;

    let validate = format!(
        "ALTER TABLE {} VALIDATE CONSTRAINT {}",
        key.table,
        quote_identifier(&key.constraint_name)
    );
    tx.execute(&"SAVEPOINT deferred_fk_validate", &[]).await?;
    match tx.execute(&validate.as_str(), &[]).await {
        Ok(_) => {
            tx.execute(&"RELEASE SAVEPOINT deferred_fk_validate", &[])
                .await?;
        },
        Err(e) => {
            tx.execute(&"ROLLBACK TO SAVEPOINT deferred_fk_validate", &[])
                .await?;
            warn!(
                extension = extension_id,
                table = %key.source_table,
                constraint = %key.constraint_name,
                error = %e,
                "Existing rows violate a foreign key the declarative schema declares; the key \
                 is left NOT VALID (enforced for new rows). Repair the rows, then run \
                 ALTER TABLE … VALIDATE CONSTRAINT."
            );
        },
    }
    Ok(())
}

fn quote_identifier(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}
