use systemprompt_extension::Migration;

#[test]
fn migration_new_fields() {
    let migration = Migration::new(1, "create_users", "CREATE TABLE users (id TEXT)");
    assert_eq!(migration.version, 1);
    assert_eq!(migration.name, "create_users");
    assert_eq!(migration.sql, "CREATE TABLE users (id TEXT)");
}

#[test]
fn migration_checksum_deterministic() {
    let migration = Migration::new(1, "test", "SELECT 1");
    let checksum_a = migration.checksum();
    let checksum_b = migration.checksum();
    assert_eq!(checksum_a, checksum_b);
}

#[test]
fn migration_different_sql_different_checksum() {
    let migration_a = Migration::new(1, "test", "SELECT 1");
    let migration_b = Migration::new(1, "test", "SELECT 2");
    assert_ne!(migration_a.checksum(), migration_b.checksum());
}

#[test]
fn migration_checksum_is_hex_string() {
    let migration = Migration::new(1, "test", "CREATE TABLE t (id INT)");
    let checksum = migration.checksum();
    assert!(checksum.chars().all(|c| c.is_ascii_hexdigit()));
    assert!(!checksum.is_empty());
}

#[test]
fn migration_clone() {
    let migration = Migration::new(2, "add_column", "ALTER TABLE users ADD email TEXT");
    let cloned = migration.clone();
    assert_eq!(cloned.version, 2);
    assert_eq!(cloned.name, "add_column");
    assert_eq!(cloned.sql, migration.sql);
}

#[test]
fn migration_debug_format() {
    let migration = Migration::new(3, "debug_test", "DROP TABLE IF EXISTS tmp");
    let debug = format!("{migration:?}");
    assert!(debug.contains("debug_test"));
    assert!(debug.contains("3"));
}

#[test]
fn migration_new_defaults_no_down_and_transactional() {
    let migration = Migration::new(1, "tx_default", "SELECT 1");
    assert!(migration.down.is_none());
    assert!(!migration.no_transaction);
}

#[test]
fn migration_with_down_records_revert_sql() {
    let migration = Migration::with_down(
        7,
        "add_email",
        "ALTER TABLE users ADD COLUMN email TEXT",
        "ALTER TABLE users DROP COLUMN email",
    );
    assert_eq!(migration.version, 7);
    assert_eq!(migration.name, "add_email");
    assert_eq!(migration.sql, "ALTER TABLE users ADD COLUMN email TEXT");
    assert_eq!(migration.down, Some("ALTER TABLE users DROP COLUMN email"));
    assert!(!migration.no_transaction);
}

#[test]
fn migration_new_no_transaction_sets_opt_out_flag() {
    let migration = Migration::new_no_transaction(
        9,
        "concurrent_index",
        "CREATE INDEX CONCURRENTLY idx_x ON t (x)",
    );
    assert!(migration.no_transaction);
    assert!(migration.down.is_none());
}

#[test]
fn migration_checksum_only_depends_on_up_sql() {
    let plain = Migration::new(1, "n", "SELECT 1");
    let with_down = Migration::with_down(1, "n", "SELECT 1", "SELECT 2");
    assert_eq!(plain.checksum(), with_down.checksum());
}
