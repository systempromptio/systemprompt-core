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
