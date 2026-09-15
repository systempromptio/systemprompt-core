use systemprompt_database::services::schema_linter::{
    LintError, LintSeverity, created_table_names, lint_declarative_schema, lint_declarative_schemas,
};

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
        Ok(warnings) => panic!("expected lint failure, got Ok with {warnings:?}"),
        Err(errs) => errs,
    }
}

#[test]
fn accepts_pure_declarative_schema() {
    lint_ok(
        "CREATE TABLE IF NOT EXISTS users (id TEXT PRIMARY KEY);\nCREATE INDEX IF NOT EXISTS \
         idx_users_id ON users(id);\nCREATE OR REPLACE VIEW v_users AS SELECT * FROM users;\n",
    );
}

#[test]
fn rejects_alter_table_add_column() {
    let errs = lint_err("ALTER TABLE users ADD COLUMN email TEXT;");
    assert!(errs.iter().any(|e| e.message.contains("ALTER")));
}

#[test]
fn rejects_top_level_do_block() {
    let errs = lint_err("DO $$ BEGIN PERFORM 1; END $$;");
    assert!(errs.iter().any(|e| e.message.contains("DO")));
}

#[test]
fn rejects_update_insert_delete() {
    assert!(
        lint_err("UPDATE users SET x = 1;")
            .iter()
            .any(|e| e.message.contains("UPDATE"))
    );
    assert!(
        lint_err("INSERT INTO users (id) VALUES ('a');")
            .iter()
            .any(|e| e.message.contains("INSERT"))
    );
    assert!(
        lint_err("DELETE FROM users WHERE id = 'a';")
            .iter()
            .any(|e| e.message.contains("DELETE"))
    );
}

#[test]
fn rejects_truncate_grant_revoke_rename() {
    assert!(
        lint_err("TRUNCATE users;")
            .iter()
            .any(|e| e.message.contains("TRUNCATE"))
    );
    assert!(
        lint_err("GRANT SELECT ON users TO public;")
            .iter()
            .any(|e| e.message.contains("GRANT"))
    );
    assert!(
        lint_err("REVOKE SELECT ON users FROM public;")
            .iter()
            .any(|e| e.message.contains("GRANT") || e.message.contains("REVOKE"))
    );
    assert!(
        lint_err("ALTER TABLE users RENAME COLUMN a TO b;")
            .iter()
            .any(|e| e.message.contains("ALTER") || e.message.contains("RENAME"))
    );
}

#[test]
fn accepts_create_or_replace_view() {
    lint_ok(
        "CREATE TABLE IF NOT EXISTS users (id TEXT PRIMARY KEY);\nCREATE OR REPLACE VIEW v_users \
         AS SELECT * FROM users;",
    );
}

#[test]
fn accepts_create_index_if_not_exists() {
    lint_ok(
        "CREATE TABLE IF NOT EXISTS users (id TEXT PRIMARY KEY, email TEXT);\nCREATE INDEX IF NOT \
         EXISTS idx_x ON users(email);\nCREATE UNIQUE INDEX IF NOT EXISTS idx_x_unique ON \
         users(email);",
    );
}

#[test]
fn warns_on_create_table_without_if_not_exists_but_passes() {
    lint_ok("CREATE TABLE foo (id TEXT);");
}

#[test]
fn error_points_at_correct_line_and_column() {
    let sql = "CREATE TABLE IF NOT EXISTS users (id TEXT);\n-- a comment\nALTER TABLE users ADD \
               COLUMN email TEXT;";
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
        "CREATE OR REPLACE FUNCTION refresh_user(uid TEXT) RETURNS VOID AS $$\nBEGIN\nUPDATE \
         users SET seen = NOW() WHERE id = uid;\nINSERT INTO audit (uid) VALUES (uid);\nEND;\n$$ \
         LANGUAGE plpgsql;",
    );
}

#[test]
fn rejects_drop_table_and_drop_view() {
    assert!(
        lint_err("DROP TABLE users;")
            .iter()
            .any(|e| e.message.contains("DROP"))
    );
    assert!(
        lint_err("DROP VIEW v_users CASCADE;")
            .iter()
            .any(|e| e.message.contains("DROP"))
    );
}

#[test]
fn accepts_create_function() {
    lint_ok(
        "CREATE OR REPLACE FUNCTION foo() RETURNS INTEGER AS $$\nBEGIN\nRETURN 42;\nEND;\n$$ \
         LANGUAGE plpgsql;",
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
        "CREATE TABLE IF NOT EXISTS users (id TEXT PRIMARY KEY);\nCOMMENT ON TABLE users IS \
         'application users';\nCOMMENT ON COLUMN users.id IS 'primary key';",
    );
}

#[test]
fn accepts_create_trigger() {
    lint_ok(
        "CREATE OR REPLACE FUNCTION touch_updated_at() RETURNS TRIGGER AS $$\nBEGIN \
         NEW.updated_at = NOW(); RETURN NEW; END;\n$$ LANGUAGE plpgsql;\nCREATE TABLE IF NOT \
         EXISTS users (id TEXT, updated_at TIMESTAMPTZ);\nCREATE TRIGGER trg_users_updated BEFORE \
         UPDATE ON users\nFOR EACH ROW EXECUTE FUNCTION touch_updated_at();",
    );
}

#[test]
fn unknown_index_column_is_rejected() {
    let errs = lint_err(
        "CREATE TABLE IF NOT EXISTS users (id TEXT PRIMARY KEY, email TEXT);\nCREATE INDEX \
         idx_users_dept ON users(department);",
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
        "CREATE TABLE IF NOT EXISTS users (id TEXT PRIMARY KEY, email TEXT);\nCREATE INDEX \
         idx_users_email ON users(email);",
    );
}

#[test]
fn index_against_external_table_is_skipped() {
    lint_ok("CREATE INDEX idx_other_x ON other_table(some_col);");
}

#[test]
fn index_expression_is_not_checked_as_column() {
    lint_ok(
        "CREATE TABLE IF NOT EXISTS users (id TEXT PRIMARY KEY, email TEXT);\nCREATE INDEX \
         idx_users_lower_email ON users (LOWER(email));",
    );
}

#[test]
fn view_with_unknown_column_against_in_input_table_is_rejected() {
    let errs = lint_err(
        "CREATE TABLE IF NOT EXISTS users (id TEXT PRIMARY KEY, email TEXT);\nCREATE OR REPLACE \
         VIEW v_users AS SELECT id, missing_col FROM users;",
    );
    assert!(
        errs.iter()
            .any(|e| e.message.contains("unknown column") && e.message.contains("missing_col"))
    );
}

#[test]
fn view_against_external_table_is_skipped() {
    lint_ok("CREATE OR REPLACE VIEW v_x AS SELECT a.id, a.foo FROM external_table a;");
}

#[test]
fn view_select_star_passes() {
    lint_ok(
        "CREATE TABLE IF NOT EXISTS users (id TEXT, email TEXT);\nCREATE OR REPLACE VIEW v_users \
         AS SELECT * FROM users;",
    );
}

#[test]
fn view_with_join_is_not_flagged() {
    lint_ok(
        "CREATE TABLE IF NOT EXISTS users (id TEXT PRIMARY KEY, email TEXT);\nCREATE OR REPLACE \
         VIEW v AS\nSELECT u.email, t.title FROM users u JOIN tasks t ON t.user_id = u.id;",
    );
}

#[test]
fn check_constraint_string_literals_are_not_flagged() {
    lint_ok(
        "CREATE TABLE IF NOT EXISTS jobs (\nid TEXT PRIMARY KEY,\nstatus TEXT NOT NULL CHECK \
         (status IN ('pending', 'running', 'done'))\n);",
    );
}

#[test]
fn references_to_external_tables_are_not_flagged() {
    lint_ok(
        "CREATE TABLE IF NOT EXISTS user_api_keys (\nid TEXT PRIMARY KEY,\nuser_id TEXT NOT NULL \
         REFERENCES users(id) ON DELETE CASCADE\n);",
    );
}

#[test]
fn parse_failure_returns_lint_error() {
    let errs = lint_err("CREATE TABLE %%% WHATEVER (");
    assert!(errs.iter().any(|e| e.message.contains("SQL parse failed")));
}

#[test]
fn realistic_postgres_schema_with_jsonb_arrays_passes() {
    lint_ok(
        "CREATE TABLE IF NOT EXISTS events (\nid TEXT PRIMARY KEY DEFAULT \
         gen_random_uuid()::TEXT,\ntags TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],\nmetadata JSONB \
         NOT NULL DEFAULT '{}'::JSONB,\ncreated_at TIMESTAMPTZ NOT NULL DEFAULT \
         CURRENT_TIMESTAMP\n);\nCREATE INDEX IF NOT EXISTS idx_events_metadata ON events USING \
         GIN (metadata);\nCREATE INDEX IF NOT EXISTS idx_events_created ON events (created_at \
         DESC);",
    );
}

#[test]
fn create_view_complex_select_with_ctes_is_not_flagged() {
    lint_ok(
        "CREATE TABLE IF NOT EXISTS sessions (id TEXT PRIMARY KEY, started_at \
         TIMESTAMPTZ);\nCREATE OR REPLACE VIEW v_sessions_by_day AS\nWITH days AS (SELECT \
         DATE(started_at) AS d FROM sessions)\nSELECT d, COUNT(*) AS n FROM days GROUP BY d;",
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
        "CREATE TABLE IF NOT EXISTS users (id TEXT PRIMARY KEY);\nCREATE INDEX \
         idx_users_department ON users(department);",
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

#[test]
fn error_position_skips_block_comments_before_statement() {
    let sql = "CREATE TABLE IF NOT EXISTS users (id TEXT);\n/* outer /* nested */ block \
               */\n-- trailing line comment\nALTER TABLE users ADD COLUMN email TEXT;";
    let errs = lint_err(sql);
    let alter = errs
        .iter()
        .find(|e| e.message.contains("ALTER"))
        .expect("expected ALTER error");
    assert_eq!(
        alter.line, 4,
        "position must skip nested block and line comments: {alter:?}"
    );
    assert_eq!(alter.column, 1);
}

#[test]
fn error_position_skips_inline_block_comment_on_same_line() {
    let sql = "CREATE TABLE IF NOT EXISTS users (id TEXT);\n/* lead */ ALTER TABLE users ADD \
               COLUMN email TEXT;";
    let errs = lint_err(sql);
    let alter = errs
        .iter()
        .find(|e| e.message.contains("ALTER"))
        .expect("expected ALTER error");
    assert_eq!(alter.line, 2, "{alter:?}");
    assert_eq!(
        alter.column, 12,
        "column must point at the first significant token: {alter:?}"
    );
}

// --- arms the existing corpus does not reach ---

#[test]
fn a_create_extension_without_if_not_exists_is_a_warning_not_a_rejection() {
    let warnings = lint_declarative_schema("CREATE EXTENSION pg_trgm;", "warn_ext")
        .expect("a missing IF NOT EXISTS is advisory — only errors reject the schema");
    assert_eq!(warnings.len(), 1, "the warning is surfaced, not dropped");
    assert_eq!(warnings[0].severity, LintSeverity::Warning);
    assert_eq!(warnings[0].source, "warn_ext");

    let clean = lint_declarative_schema("CREATE EXTENSION IF NOT EXISTS pg_trgm;", "ok_ext")
        .expect("the guarded form is clean");
    assert!(clean.is_empty());
}

#[test]
fn a_create_table_without_if_not_exists_is_surfaced_as_a_warning() {
    let warnings = lint_declarative_schema("CREATE TABLE t (id INT PRIMARY KEY);", "t.sql")
        .expect("advisory only");
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].line, 1);
    assert!(
        warnings[0].message.contains("IF NOT EXISTS"),
        "{}",
        warnings[0]
    );
}

#[test]
fn created_table_names_keeps_the_schema_qualifier() {
    let names = created_table_names(
        "CREATE TABLE IF NOT EXISTS s.t (id INT PRIMARY KEY); CREATE TABLE IF NOT EXISTS u (id \
         INT PRIMARY KEY);",
    )
    .expect("parses");
    assert_eq!(names, vec!["s.t".to_owned(), "u".to_owned()]);
}

#[test]
fn created_table_names_reports_a_parse_failure_instead_of_owning_nothing() {
    created_table_names("CREATE TABLE (((").expect_err("unparseable SQL is not an empty schema");
}

#[test]
fn sql_that_does_not_parse_is_reported_as_a_single_parse_error() {
    let errs = lint_declarative_schema("CREATE TABLE (((", "bad_sql")
        .expect_err("unparseable SQL cannot be linted");
    assert_eq!(
        errs.len(),
        1,
        "an unparseable file yields one parse error, not a cascade: {errs:?}"
    );
    assert!(
        errs[0].to_string().contains("SQL parse failed"),
        "got {}",
        errs[0]
    );
}

#[test]
fn declarative_object_kinds_other_than_tables_pass_untouched() {
    lint_declarative_schema(
        "CREATE TABLE IF NOT EXISTS t (id TEXT PRIMARY KEY, body TEXT);\n\
         CREATE INDEX IF NOT EXISTS t_body_idx ON t (body);\n\
         CREATE OR REPLACE VIEW v AS SELECT id FROM t;\n\
         COMMENT ON TABLE t IS 'a table';",
        "declarative_kinds",
    )
    .expect("indexes, views and comments are declarative");
}

#[test]
fn an_empty_schema_file_lints_clean() {
    lint_declarative_schema("", "empty").expect("an empty file declares nothing");
    lint_declarative_schema("-- only a comment\n", "comment_only")
        .expect("a comment-only file declares nothing");
}

#[test]
fn every_reported_error_carries_the_source_name_it_was_given() {
    let errs = lint_declarative_schema("INSERT INTO t VALUES (1);", "my_schema.sql")
        .expect_err("imperative SQL must be rejected");
    assert!(
        errs.iter().all(|e| e.to_string().contains("my_schema.sql")),
        "every diagnostic must name the file it came from: {errs:?}"
    );
}

// ── Foreign keys and the uniqueness they reference ───────────────────────────

#[test]
fn foreign_key_to_in_input_table_without_matching_unique_is_rejected() {
    let errs = lint_err(
        "CREATE TABLE IF NOT EXISTS events (id TEXT PRIMARY KEY, user_id TEXT NOT NULL);\n\
         CREATE UNIQUE INDEX IF NOT EXISTS events_owner ON events (user_id, id);\n\
         CREATE TABLE IF NOT EXISTS reviews (id TEXT PRIMARY KEY, owner_id TEXT, event_id TEXT, \
         FOREIGN KEY (owner_id, event_id) REFERENCES events (user_id, id));",
    );
    let e = errs
        .iter()
        .find(|e| e.message.contains("foreign key on `reviews`"))
        .expect("the composite key is reported");
    assert!(
        e.message.contains("references `events`(user_id, id)"),
        "{}",
        e.message
    );
    assert!(e.message.contains("CREATE UNIQUE INDEX"), "{}", e.message);
    assert_eq!(e.line, 3, "{e}");
}

#[test]
fn foreign_key_matching_table_level_unique_passes_in_either_column_order() {
    lint_ok(
        "CREATE TABLE IF NOT EXISTS events (id TEXT PRIMARY KEY, user_id TEXT NOT NULL, \
         UNIQUE (user_id, id));\n\
         CREATE TABLE IF NOT EXISTS reviews (id TEXT PRIMARY KEY, owner_id TEXT, event_id TEXT, \
         FOREIGN KEY (event_id, owner_id) REFERENCES events (id, user_id));",
    );
}

#[test]
fn foreign_key_matching_column_level_primary_key_passes() {
    lint_ok(
        "CREATE TABLE IF NOT EXISTS users (id TEXT PRIMARY KEY);\n\
         CREATE TABLE IF NOT EXISTS keys (id TEXT PRIMARY KEY, user_id TEXT REFERENCES users (id));",
    );
}

#[test]
fn foreign_key_matching_column_level_unique_passes() {
    lint_ok(
        "CREATE TABLE IF NOT EXISTS users (id TEXT PRIMARY KEY, email TEXT UNIQUE);\n\
         CREATE TABLE IF NOT EXISTS invites (id TEXT PRIMARY KEY, email TEXT REFERENCES users \
         (email));",
    );
}

#[test]
fn references_without_columns_uses_the_referenced_primary_key() {
    lint_ok(
        "CREATE TABLE IF NOT EXISTS users (id TEXT PRIMARY KEY);\n\
         CREATE TABLE IF NOT EXISTS keys (id TEXT PRIMARY KEY, user_id TEXT REFERENCES users);",
    );
    let errs = lint_err(
        "CREATE TABLE IF NOT EXISTS logs (id TEXT);\n\
         CREATE TABLE IF NOT EXISTS keys (id TEXT PRIMARY KEY, log_id TEXT REFERENCES logs);",
    );
    assert!(
        errs.iter()
            .any(|e| e.message.contains("declares no PRIMARY KEY")),
        "{errs:?}"
    );
}

#[test]
fn a_subset_or_superset_unique_does_not_satisfy_a_composite_key() {
    let errs = lint_err(
        "CREATE TABLE IF NOT EXISTS events (id TEXT PRIMARY KEY, user_id TEXT, kind TEXT, UNIQUE \
         (user_id, id, kind));\n\
         CREATE TABLE IF NOT EXISTS reviews (id TEXT PRIMARY KEY, owner_id TEXT, event_id TEXT, \
         FOREIGN KEY (owner_id, event_id) REFERENCES events (user_id, id));",
    );
    assert!(
        errs.iter()
            .any(|e| e.message.contains("exactly those columns")),
        "{errs:?}"
    );
}

#[test]
fn foreign_key_to_external_table_is_skipped() {
    lint_ok(
        "CREATE TABLE IF NOT EXISTS reviews (id TEXT PRIMARY KEY, owner_id TEXT, event_id TEXT, \
         FOREIGN KEY (owner_id, event_id) REFERENCES somewhere_else (user_id, id));",
    );
}

#[test]
fn foreign_key_across_files_of_one_extension_is_checked_with_per_file_position() {
    let a = "CREATE TABLE IF NOT EXISTS events (id TEXT PRIMARY KEY, user_id TEXT NOT NULL);";
    let b = "-- a comment line first\nCREATE TABLE IF NOT EXISTS reviews (id TEXT PRIMARY KEY, \
             owner_id TEXT, event_id TEXT, FOREIGN KEY (owner_id, event_id) REFERENCES events \
             (user_id, id));";
    let errs = lint_declarative_schemas(&[("a.sql", a), ("b.sql", b)]).expect_err("rejected");
    let e = errs
        .iter()
        .find(|e| e.message.contains("foreign key on `reviews`"))
        .expect("reported");
    assert_eq!(e.source, "b.sql");
    assert_eq!(e.line, 2);

    let fixed_a = "CREATE TABLE IF NOT EXISTS events (id TEXT PRIMARY KEY, user_id TEXT NOT NULL, \
                   UNIQUE (user_id, id));";
    lint_declarative_schemas(&[("a.sql", fixed_a), ("b.sql", b)]).expect("declared unique passes");
}

#[test]
fn a_parse_failure_in_one_file_still_lints_the_others() {
    let errs = lint_declarative_schemas(&[
        ("broken.sql", "CREATE TABLE %%% ("),
        ("ok.sql", "INSERT INTO t VALUES (1);"),
    ])
    .expect_err("both reported");
    assert!(
        errs.iter()
            .any(|e| e.source == "broken.sql" && e.message.contains("SQL parse failed"))
    );
    assert!(
        errs.iter()
            .any(|e| e.source == "ok.sql" && e.message.contains("imperative SQL"))
    );
}

#[test]
fn a_quoted_mixed_case_unique_column_does_not_satisfy_a_lowercase_reference() {
    let errs = lint_err(
        "CREATE TABLE IF NOT EXISTS p (\"Id\" INT UNIQUE);\nCREATE TABLE IF NOT EXISTS c (p_id \
         INT REFERENCES p(id));",
    );
    assert!(
        errs.iter()
            .any(|e| e.message.contains("declares no PRIMARY KEY")),
        "{errs:?}"
    );
}

#[test]
fn a_duplicated_referenced_column_does_not_match_a_two_column_unique() {
    let errs = lint_err(
        "CREATE TABLE IF NOT EXISTS p (a INT, b INT, UNIQUE (a, b));\nCREATE TABLE IF NOT \
         EXISTS c (x INT, y INT, FOREIGN KEY (x, y) REFERENCES p(a, a));",
    );
    assert!(
        errs.iter()
            .any(|e| e.message.contains("declares no PRIMARY KEY")),
        "{errs:?}"
    );
}

#[test]
fn same_named_tables_in_different_schemas_do_not_alias_each_other() {
    let errs = lint_err(
        "CREATE TABLE IF NOT EXISTS a.p (id INT PRIMARY KEY);\nCREATE TABLE IF NOT EXISTS b.p \
         (id INT);\nCREATE INDEX IF NOT EXISTS i ON b.p (missing);",
    );
    assert!(
        errs.iter().any(|e| e.message.contains("unknown column")),
        "{errs:?}"
    );
}
