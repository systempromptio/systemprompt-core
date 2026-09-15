//! Splitting `FOREIGN KEY` constraints out of a declarative `CREATE TABLE`.

use systemprompt_database::{FkDeferralError, SplitCreateTable, split_create_table_foreign_keys};

fn split(sql: &str) -> SplitCreateTable {
    split_create_table_foreign_keys(sql).expect("split")
}

#[test]
fn create_table_without_foreign_keys_is_returned_verbatim() {
    let sql = "CREATE TABLE IF NOT EXISTS   t (\n  id TEXT PRIMARY KEY -- keep me\n)";
    let out = split(sql);
    assert_eq!(out.create_table_sql, sql);
    assert!(out.foreign_keys.is_empty());
}

#[test]
fn table_level_composite_foreign_key_is_split_into_alter_with_default_name() {
    let out = split(
        "CREATE TABLE IF NOT EXISTS reviewed_production_failures (\n  id TEXT PRIMARY KEY,\n  \
         owner_id TEXT NOT NULL,\n  invocation_id TEXT NOT NULL,\n  UNIQUE(owner_id,invocation_id),\n  \
         FOREIGN KEY(owner_id,invocation_id) REFERENCES plugin_usage_events(user_id,id)\n)",
    );
    assert!(
        out.create_table_sql.contains("IF NOT EXISTS"),
        "{}",
        out.create_table_sql
    );
    assert!(
        out.create_table_sql
            .contains("UNIQUE (owner_id, invocation_id)"),
        "{}",
        out.create_table_sql
    );
    assert!(
        !out.create_table_sql.contains("REFERENCES"),
        "{}",
        out.create_table_sql
    );

    assert_eq!(out.foreign_keys.len(), 1);
    let key = &out.foreign_keys[0];
    assert_eq!(
        key.constraint_name,
        "reviewed_production_failures_owner_id_invocation_id_fkey"
    );
    assert_eq!(key.columns, vec!["owner_id", "invocation_id"]);
    assert_eq!(key.referenced_columns, vec!["user_id", "id"]);
    assert_eq!(key.table, "\"reviewed_production_failures\"");
    assert_eq!(key.referenced_table, "\"plugin_usage_events\"");
    assert_eq!(
        key.sql,
        "ALTER TABLE reviewed_production_failures ADD CONSTRAINT \
         reviewed_production_failures_owner_id_invocation_id_fkey FOREIGN KEY (owner_id, \
         invocation_id) REFERENCES plugin_usage_events (user_id, id)"
    );
}

#[test]
fn column_level_references_keeps_default_check_and_generated() {
    let out = split(
        "CREATE TABLE IF NOT EXISTS t (\n  id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,\n  \
         n INT NOT NULL DEFAULT 1 CHECK (n > 0),\n  user_id TEXT NOT NULL REFERENCES users(id) ON \
         DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,\n  created_at TIMESTAMPTZ NOT NULL DEFAULT \
         NOW()\n)",
    );
    let create = &out.create_table_sql;
    assert!(!create.contains("REFERENCES"), "{create}");
    assert!(create.contains("GENERATED ALWAYS AS IDENTITY"), "{create}");
    assert!(create.contains("DEFAULT 1"), "{create}");
    assert!(create.contains("CHECK (n > 0)"), "{create}");
    assert!(create.contains("DEFAULT now()"), "{create}");
    assert!(create.contains("PRIMARY KEY"), "{create}");
    assert!(!create.contains("DEFERRABLE"), "{create}");

    assert_eq!(out.foreign_keys.len(), 1);
    let key = &out.foreign_keys[0];
    assert_eq!(key.constraint_name, "t_user_id_fkey");
    assert_eq!(key.columns, vec!["user_id"]);
    assert!(
        key.sql
            .contains("FOREIGN KEY (user_id) REFERENCES users (id)"),
        "{}",
        key.sql
    );
    assert!(key.sql.contains("ON DELETE CASCADE"), "{}", key.sql);
    assert!(
        key.sql.contains("DEFERRABLE INITIALLY DEFERRED"),
        "{}",
        key.sql
    );
}

#[test]
fn named_constraint_schema_and_explicit_columns_are_preserved() {
    let out = split(
        "CREATE TABLE app.t (a TEXT, b TEXT, CONSTRAINT t_owner FOREIGN KEY (a, b) REFERENCES \
         app.r (x, y) ON UPDATE CASCADE)",
    );
    let key = &out.foreign_keys[0];
    assert_eq!(key.constraint_name, "t_owner");
    assert_eq!(key.table, "\"app\".\"t\"");
    assert_eq!(key.referenced_table, "\"app\".\"r\"");
    assert_eq!(key.referenced_columns, vec!["x", "y"]);
    assert!(
        key.sql
            .starts_with("ALTER TABLE app.t ADD CONSTRAINT t_owner"),
        "{}",
        key.sql
    );
    assert!(key.sql.contains("ON UPDATE CASCADE"), "{}", key.sql);
}

#[test]
fn references_without_columns_yields_empty_referenced_columns() {
    let out = split("CREATE TABLE t (r_id TEXT REFERENCES r)");
    let key = &out.foreign_keys[0];
    assert!(key.referenced_columns.is_empty());
    assert_eq!(
        key.sql,
        "ALTER TABLE t ADD CONSTRAINT t_r_id_fkey FOREIGN KEY (r_id) REFERENCES r"
    );
}

#[test]
fn other_column_attributes_stay_on_the_column() {
    let out = split("CREATE TABLE t (a TEXT UNIQUE DEFERRABLE, r_id TEXT REFERENCES r (id))");
    assert!(
        out.create_table_sql.contains("UNIQUE DEFERRABLE"),
        "{}",
        out.create_table_sql
    );
    assert!(
        !out.foreign_keys[0].sql.contains("DEFERRABLE"),
        "{}",
        out.foreign_keys[0].sql
    );
}

#[test]
fn split_outputs_reparse() {
    let out = split(
        "CREATE TABLE IF NOT EXISTS t (id TEXT PRIMARY KEY, a TEXT, b TEXT, FOREIGN KEY (a, b) \
         REFERENCES r (x, y))",
    );
    pg_query::parse(&out.create_table_sql).expect("create reparses");
    pg_query::parse(&out.foreign_keys[0].sql).expect("alter reparses");
}

#[test]
fn a_default_name_is_cut_to_the_identifier_limit() {
    let column = "c".repeat(80);
    let out = split(&format!("CREATE TABLE t ({column} TEXT REFERENCES r (id))"));
    assert_eq!(out.foreign_keys[0].constraint_name.len(), 63);
}

#[test]
fn a_non_create_statement_is_refused() {
    let err = split_create_table_foreign_keys("SELECT 1").expect_err("refused");
    assert!(matches!(err, FkDeferralError::NotCreateTable), "{err}");
}

#[test]
fn unparseable_sql_is_a_typed_parse_error() {
    let err = split_create_table_foreign_keys("CREATE TABLE (((").expect_err("refused");
    assert!(matches!(err, FkDeferralError::Parse(_)), "{err}");
}

#[test]
fn a_deferrable_after_a_unique_between_it_and_the_key_stays_on_the_unique() {
    // Postgres attaches DEFERRABLE to the last key-like constraint on the
    // column; here that is UNIQUE, not the REFERENCES before it.
    let out = split("CREATE TABLE t (r_id TEXT REFERENCES r (id) UNIQUE DEFERRABLE)");
    assert!(
        out.create_table_sql.contains("UNIQUE DEFERRABLE"),
        "{}",
        out.create_table_sql
    );
    assert!(
        !out.foreign_keys[0].sql.contains("DEFERRABLE"),
        "{}",
        out.foreign_keys[0].sql
    );
}
