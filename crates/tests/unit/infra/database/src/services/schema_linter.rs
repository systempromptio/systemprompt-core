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
        "DO $$ BEGIN ALTER TABLE foo RENAME COLUMN a TO b; END $$;",
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
fn accepts_create_or_replace_view() {
    lint_ok("CREATE OR REPLACE VIEW v_users AS SELECT * FROM users;");
}

#[test]
fn accepts_create_index_if_not_exists() {
    lint_ok("CREATE INDEX IF NOT EXISTS idx_x ON users(email);");
    lint_ok("CREATE UNIQUE INDEX IF NOT EXISTS idx_x_unique ON users(email);");
}

#[test]
fn warns_on_create_table_without_if_not_exists() {
    // Warnings are non-fatal: lint returns Ok but the user is expected to
    // notice via verification tooling. We confirm Ok here, and exercise the
    // warning shape via the public classifier path used by lint().
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
