use systemprompt_extension::seed::Seed;

#[test]
fn seed_new_stores_id_and_sql() {
    let seed = Seed::new(
        "initial_roles",
        "INSERT INTO roles (name) VALUES ('admin') ON CONFLICT DO NOTHING",
    );
    assert_eq!(seed.id, "initial_roles");
    assert!(seed.sql.contains("INSERT INTO roles"));
}

#[test]
fn seed_id_field_is_static_str() {
    let seed = Seed::new("seed_id", "UPDATE config SET value = 'x' WHERE key = 'y'");
    let id: &'static str = seed.id;
    assert_eq!(id, "seed_id");
}

#[test]
fn seed_sql_field_is_static_str() {
    let seed = Seed::new("s", "UPDATE t SET v = 1");
    let sql: &'static str = seed.sql;
    assert!(sql.contains("UPDATE"));
}

#[test]
fn seed_debug_format_includes_id() {
    let seed = Seed::new("debug_seed", "UPDATE x SET y = 1");
    let debug = format!("{seed:?}");
    assert!(debug.contains("debug_seed"));
}

#[test]
fn seed_clone_produces_equal_values() {
    let seed = Seed::new(
        "clone_me",
        "INSERT INTO t (id) VALUES (1) ON CONFLICT DO NOTHING",
    );
    let cloned = seed;
    assert_eq!(cloned.id, "clone_me");
    assert_eq!(cloned.sql, seed.sql);
}

#[test]
fn seed_copy_semantics() {
    let seed = Seed::new(
        "copyable",
        "MERGE INTO t USING src ON t.id = src.id WHEN MATCHED THEN UPDATE SET v = src.v",
    );
    let second = seed;
    assert_eq!(second.id, seed.id);
}

#[test]
fn seed_new_is_const() {
    const SEED: Seed = Seed::new(
        "const_seed",
        "UPDATE settings SET active = true WHERE id = 'x'",
    );
    assert_eq!(SEED.id, "const_seed");
}

#[test]
fn seed_empty_id_is_accepted() {
    let seed = Seed::new("", "INSERT INTO t (id) VALUES (1) ON CONFLICT DO NOTHING");
    assert_eq!(seed.id, "");
}

#[test]
fn seed_multiple_seeds_have_independent_ids() {
    let a = Seed::new("seed_a", "UPDATE a SET v = 1");
    let b = Seed::new("seed_b", "UPDATE b SET v = 2");
    assert_ne!(a.id, b.id);
    assert_ne!(a.sql, b.sql);
}
