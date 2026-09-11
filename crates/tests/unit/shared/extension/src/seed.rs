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
