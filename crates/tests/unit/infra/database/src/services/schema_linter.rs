use systemprompt_database::services::{LintError, lint_declarative_schema};

fn lint_ok(sql: &str) {
    if let Err(errs) = lint_declarative_schema(sql, "test") {
        panic!(
            "expected pure declarative SQL to pass, got: {}",
            errs.iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}

fn lint_err(sql: &str) -> Vec<LintError> {
    match lint_declarative_schema(sql, "test") {
        Ok(()) => panic!("expected lint failure, got Ok"),
        Err(errs) => errs,
    }
}

#[test]
fn accepts_pure_declarative_schema() {
    lint_ok(
        "CREATE TABLE IF NOT EXISTS users (id TEXT PRIMARY KEY);\n\
         CREATE INDEX IF NOT EXISTS idx_users_id ON users(id);\n\
         CREATE OR REPLACE VIEW v_users AS SELECT * FROM users;\n",
    );
}

#[test]
fn rejects_alter_table_add_column() {
    let errs = lint_err("ALTER TABLE users ADD COLUMN email TEXT;");
    assert!(errs.iter().any(|e| e.message.contains("ALTER")));
}

#[test]
fn rejects_top_level_do_block() {
    let errs = lint_err(
        "DO $$ BEGIN PERFORM 1; END $$;",
    );
    assert!(errs.iter().any(|e| e.message.contains("DO")));
}

#[test]
fn rejects_update_insert_delete() {
    assert!(lint_err("UPDATE users SET x = 1;")
        .iter()
        .any(|e| e.message.contains("UPDATE")));
    assert!(lint_err("INSERT INTO users (id) VALUES ('a');")
        .iter()
        .any(|e| e.message.contains("INSERT")));
    assert!(lint_err("DELETE FROM users WHERE id = 'a';")
        .iter()
        .any(|e| e.message.contains("DELETE")));
}

#[test]
fn rejects_truncate_grant_revoke_rename() {
    assert!(lint_err("TRUNCATE users;")
        .iter()
        .any(|e| e.message.contains("TRUNCATE")));
    assert!(lint_err("GRANT SELECT ON users TO public;")
        .iter()
        .any(|e| e.message.contains("GRANT")));
    assert!(lint_err("REVOKE SELECT ON users FROM public;")
        .iter()
        .any(|e| e.message.contains("GRANT") || e.message.contains("REVOKE")));
    assert!(lint_err("ALTER TABLE users RENAME COLUMN a TO b;")
        .iter()
        .any(|e| e.message.contains("ALTER") || e.message.contains("RENAME")));
}

#[test]
fn accepts_create_or_replace_view() {
    lint_ok(
        "CREATE TABLE IF NOT EXISTS users (id TEXT PRIMARY KEY);\n\
         CREATE OR REPLACE VIEW v_users AS SELECT * FROM users;",
    );
}

#[test]
fn accepts_create_index_if_not_exists() {
    lint_ok(
        "CREATE TABLE IF NOT EXISTS users (id TEXT PRIMARY KEY, email TEXT);\n\
         CREATE INDEX IF NOT EXISTS idx_x ON users(email);\n\
         CREATE UNIQUE INDEX IF NOT EXISTS idx_x_unique ON users(email);",
    );
}

#[test]
fn warns_on_create_table_without_if_not_exists_but_passes() {
    lint_ok("CREATE TABLE foo (id TEXT);");
}

#[test]
fn error_points_at_correct_line_and_column() {
    let sql = "CREATE TABLE IF NOT EXISTS users (id TEXT);\n\
               -- a comment\n\
               ALTER TABLE users ADD COLUMN email TEXT;";
    let errs = lint_err(sql);
    let alter = errs
        .iter()
        .find(|e| e.message.contains("ALTER"))
        .expect("expected ALTER error");
    assert_eq!(alter.line, 3, "ALTER should be on line 3, got {alter:?}");
    assert_eq!(alter.column, 1);
}

#[test]
fn lint_inside_dollar_quoted_function_body_is_skipped() {
    lint_ok(
        "CREATE OR REPLACE FUNCTION refresh_user(uid TEXT) RETURNS VOID AS $$\n\
         BEGIN\n\
           UPDATE users SET seen = NOW() WHERE id = uid;\n\
           INSERT INTO audit (uid) VALUES (uid);\n\
         END;\n\
         $$ LANGUAGE plpgsql;",
    );
}

#[test]
fn rejects_drop_table_and_drop_view() {
    assert!(lint_err("DROP TABLE users;")
        .iter()
        .any(|e| e.message.contains("DROP")));
    assert!(lint_err("DROP VIEW v_users CASCADE;")
        .iter()
        .any(|e| e.message.contains("DROP")));
}

#[test]
fn accepts_create_function() {
    lint_ok(
        "CREATE OR REPLACE FUNCTION foo() RETURNS INTEGER AS $$\n\
         BEGIN\n\
           RETURN 42;\n\
         END;\n\
         $$ LANGUAGE plpgsql;",
    );
}

#[test]
fn accepts_create_extension_if_not_exists() {
    lint_ok("CREATE EXTENSION IF NOT EXISTS pgcrypto;");
}

#[test]
fn accepts_composite_type_and_enum() {
    lint_ok("CREATE TYPE address AS (street TEXT, city TEXT);");
    lint_ok("CREATE TYPE status AS ENUM ('a', 'b', 'c');");
}

#[test]
fn accepts_comment_on() {
    lint_ok(
        "CREATE TABLE IF NOT EXISTS users (id TEXT PRIMARY KEY);\n\
         COMMENT ON TABLE users IS 'application users';\n\
         COMMENT ON COLUMN users.id IS 'primary key';",
    );
}

#[test]
fn accepts_create_trigger() {
    lint_ok(
        "CREATE OR REPLACE FUNCTION touch_updated_at() RETURNS TRIGGER AS $$\n\
         BEGIN NEW.updated_at = NOW(); RETURN NEW; END;\n\
         $$ LANGUAGE plpgsql;\n\
         CREATE TABLE IF NOT EXISTS users (id TEXT, updated_at TIMESTAMPTZ);\n\
         CREATE TRIGGER trg_users_updated BEFORE UPDATE ON users\n\
         FOR EACH ROW EXECUTE FUNCTION touch_updated_at();",
    );
}

#[test]
fn unknown_index_column_is_rejected() {
    let errs = lint_err(
        "CREATE TABLE IF NOT EXISTS users (id TEXT PRIMARY KEY, email TEXT);\n\
         CREATE INDEX idx_users_dept ON users(department);",
    );
    let unknown = errs
        .iter()
        .find(|e| e.message.contains("unknown column"))
        .expect("expected unknown column error");
    assert!(
        unknown.message.contains("`department`"),
        "message should name the column: {}",
        unknown.message
    );
    assert!(
        unknown.message.contains("`users`"),
        "message should name the table: {}",
        unknown.message
    );
}

#[test]
fn known_index_column_passes() {
    lint_ok(
        "CREATE TABLE IF NOT EXISTS users (id TEXT PRIMARY KEY, email TEXT);\n\
         CREATE INDEX idx_users_email ON users(email);",
    );
}

#[test]
fn index_against_external_table_is_skipped() {
    lint_ok("CREATE INDEX idx_other_x ON other_table(some_col);");
}

#[test]
fn index_expression_is_not_checked_as_column() {
    lint_ok(
        "CREATE TABLE IF NOT EXISTS users (id TEXT PRIMARY KEY, email TEXT);\n\
         CREATE INDEX idx_users_lower_email ON users (LOWER(email));",
    );
}

#[test]
fn view_with_unknown_column_against_in_input_table_is_rejected() {
    let errs = lint_err(
        "CREATE TABLE IF NOT EXISTS users (id TEXT PRIMARY KEY, email TEXT);\n\
         CREATE OR REPLACE VIEW v_users AS SELECT id, missing_col FROM users;",
    );
    assert!(errs
        .iter()
        .any(|e| e.message.contains("unknown column") && e.message.contains("missing_col")));
}

#[test]
fn view_against_external_table_is_skipped() {
    lint_ok("CREATE OR REPLACE VIEW v_x AS SELECT a.id, a.foo FROM external_table a;");
}

#[test]
fn view_select_star_passes() {
    lint_ok(
        "CREATE TABLE IF NOT EXISTS users (id TEXT, email TEXT);\n\
         CREATE OR REPLACE VIEW v_users AS SELECT * FROM users;",
    );
}

#[test]
fn view_with_join_is_not_flagged() {
    lint_ok(
        "CREATE TABLE IF NOT EXISTS users (id TEXT PRIMARY KEY, email TEXT);\n\
         CREATE OR REPLACE VIEW v AS\n\
         SELECT u.email, t.title FROM users u JOIN tasks t ON t.user_id = u.id;",
    );
}

#[test]
fn check_constraint_string_literals_are_not_flagged() {
    lint_ok(
        "CREATE TABLE IF NOT EXISTS jobs (\n\
           id TEXT PRIMARY KEY,\n\
           status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'done'))\n\
         );",
    );
}

#[test]
fn references_to_external_tables_are_not_flagged() {
    lint_ok(
        "CREATE TABLE IF NOT EXISTS user_api_keys (\n\
           id TEXT PRIMARY KEY,\n\
           user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE\n\
         );",
    );
}

#[test]
fn parse_failure_returns_lint_error() {
    let errs = lint_err("CREATE TABLE %%% WHATEVER (");
    assert!(errs
        .iter()
        .any(|e| e.message.contains("SQL parse failed")));
}

#[test]
fn realistic_postgres_schema_with_jsonb_arrays_passes() {
    lint_ok(
        "CREATE TABLE IF NOT EXISTS events (\n\
           id TEXT PRIMARY KEY DEFAULT gen_random_uuid()::TEXT,\n\
           tags TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],\n\
           metadata JSONB NOT NULL DEFAULT '{}'::JSONB,\n\
           created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP\n\
         );\n\
         CREATE INDEX IF NOT EXISTS idx_events_metadata ON events USING GIN (metadata);\n\
         CREATE INDEX IF NOT EXISTS idx_events_created ON events (created_at DESC);",
    );
}

#[test]
fn create_view_complex_select_with_ctes_is_not_flagged() {
    lint_ok(
        "CREATE TABLE IF NOT EXISTS sessions (id TEXT PRIMARY KEY, started_at TIMESTAMPTZ);\n\
         CREATE OR REPLACE VIEW v_sessions_by_day AS\n\
         WITH days AS (SELECT DATE(started_at) AS d FROM sessions)\n\
         SELECT d, COUNT(*) AS n FROM days GROUP BY d;",
    );
}

#[test]
fn alter_database_is_rejected() {
    let errs = lint_err("ALTER DATABASE mydb SET search_path = public;");
    assert!(errs.iter().any(|e| e.message.contains("ALTER")));
}

#[test]
fn copy_statement_is_rejected() {
    let errs = lint_err("COPY users FROM '/tmp/users.csv';");
    assert!(errs.iter().any(|e| e.message.contains("COPY")));
}

#[test]
fn bare_select_is_rejected() {
    let errs = lint_err("SELECT 1;");
    assert!(errs.iter().any(|e| e.message.contains("SELECT")));
}

#[test]
fn unknown_column_includes_index_name_in_message() {
    let errs = lint_err(
        "CREATE TABLE IF NOT EXISTS users (id TEXT PRIMARY KEY);\n\
         CREATE INDEX idx_users_department ON users(department);",
    );
    let err = errs
        .iter()
        .find(|e| e.message.contains("unknown column"))
        .expect("expected unknown column error");
    assert!(
        err.message.contains("idx_users_department"),
        "message should reference the index name: {}",
        err.message
    );
}
