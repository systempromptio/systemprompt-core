//! Unit tests for AppliedMigration, MigrationResult, and MigrationStatus structs

use systemprompt_database::{AppliedMigration, MigrationResult, MigrationStatus};

// ============================================================================
// AppliedMigration Tests
// ============================================================================

#[test]
fn test_applied_migration_creation() {
    let migration = AppliedMigration {
        extension_id: "users".to_string(),
        version: 1,
        name: "create_users_table".to_string(),
        checksum: "abc123".to_string(),
    };

    assert_eq!(migration.extension_id, "users");
    assert_eq!(migration.version, 1);
    assert_eq!(migration.name, "create_users_table");
    assert_eq!(migration.checksum, "abc123");
}

#[test]
fn test_applied_migration_debug() {
    let migration = AppliedMigration {
        extension_id: "test".to_string(),
        version: 2,
        name: "add_column".to_string(),
        checksum: "def456".to_string(),
    };

    let debug = format!("{:?}", migration);
    assert!(debug.contains("AppliedMigration"));
    assert!(debug.contains("test"));
    assert!(debug.contains("add_column"));
}

#[test]
fn test_applied_migration_clone() {
    let migration = AppliedMigration {
        extension_id: "original".to_string(),
        version: 5,
        name: "migration_name".to_string(),
        checksum: "checksum123".to_string(),
    };

    let cloned = migration.clone();
    assert_eq!(migration.extension_id, cloned.extension_id);
    assert_eq!(migration.version, cloned.version);
    assert_eq!(migration.name, cloned.name);
    assert_eq!(migration.checksum, cloned.checksum);
}

#[test]
fn test_applied_migration_with_high_version() {
    let migration = AppliedMigration {
        extension_id: "ext".to_string(),
        version: u32::MAX,
        name: "max_version".to_string(),
        checksum: "hash".to_string(),
    };

    assert_eq!(migration.version, u32::MAX);
}

#[test]
fn test_applied_migration_with_empty_strings() {
    let migration = AppliedMigration {
        extension_id: String::new(),
        version: 0,
        name: String::new(),
        checksum: String::new(),
    };

    assert!(migration.extension_id.is_empty());
    assert!(migration.name.is_empty());
    assert!(migration.checksum.is_empty());
}

// ============================================================================
// MigrationResult Tests
// ============================================================================

#[test]
fn test_migration_result_default() {
    let result = MigrationResult::default();
    assert_eq!(result.migrations_run, 0);
    assert_eq!(result.migrations_skipped, 0);
}

#[test]
fn test_migration_result_with_values() {
    let result = MigrationResult {
        migrations_run: 5,
        migrations_skipped: 3,
    };

    assert_eq!(result.migrations_run, 5);
    assert_eq!(result.migrations_skipped, 3);
}

#[test]
fn test_migration_result_debug() {
    let result = MigrationResult {
        migrations_run: 10,
        migrations_skipped: 2,
    };

    let debug = format!("{:?}", result);
    assert!(debug.contains("MigrationResult"));
}

#[test]
fn test_migration_result_copy() {
    let result = MigrationResult {
        migrations_run: 7,
        migrations_skipped: 1,
    };

    let copied = result;
    assert_eq!(result.migrations_run, copied.migrations_run);
    assert_eq!(result.migrations_skipped, copied.migrations_skipped);
}

#[test]
fn test_migration_result_zero_values() {
    let result = MigrationResult {
        migrations_run: 0,
        migrations_skipped: 0,
    };

    assert_eq!(result.migrations_run, 0);
    assert_eq!(result.migrations_skipped, 0);
}

#[test]
fn test_migration_result_large_values() {
    let result = MigrationResult {
        migrations_run: 1_000_000,
        migrations_skipped: 500_000,
    };

    assert_eq!(result.migrations_run, 1_000_000);
    assert_eq!(result.migrations_skipped, 500_000);
}

// ============================================================================
// MigrationStatus Tests
// ============================================================================

#[test]
fn test_migration_status_creation() {
    let status = MigrationStatus {
        extension_id: "content".to_string(),
        total_defined: 10,
        total_applied: 8,
        pending_count: 2,
        pending: vec![],
        applied: vec![],
    };

    assert_eq!(status.extension_id, "content");
    assert_eq!(status.total_defined, 10);
    assert_eq!(status.total_applied, 8);
    assert_eq!(status.pending_count, 2);
}

#[test]
fn test_migration_status_debug() {
    let status = MigrationStatus {
        extension_id: "debug_test".to_string(),
        total_defined: 5,
        total_applied: 5,
        pending_count: 0,
        pending: vec![],
        applied: vec![],
    };

    let debug = format!("{:?}", status);
    assert!(debug.contains("MigrationStatus"));
    assert!(debug.contains("debug_test"));
}

#[test]
fn test_migration_status_all_applied() {
    let status = MigrationStatus {
        extension_id: "fully_migrated".to_string(),
        total_defined: 15,
        total_applied: 15,
        pending_count: 0,
        pending: vec![],
        applied: vec![],
    };

    assert_eq!(status.total_defined, status.total_applied);
    assert_eq!(status.pending_count, 0);
}

#[test]
fn test_migration_status_with_applied_migrations() {
    let applied = vec![
        AppliedMigration {
            extension_id: "test".to_string(),
            version: 1,
            name: "v1".to_string(),
            checksum: "hash1".to_string(),
        },
        AppliedMigration {
            extension_id: "test".to_string(),
            version: 2,
            name: "v2".to_string(),
            checksum: "hash2".to_string(),
        },
    ];

    let status = MigrationStatus {
        extension_id: "test".to_string(),
        total_defined: 3,
        total_applied: 2,
        pending_count: 1,
        pending: vec![],
        applied,
    };

    assert_eq!(status.applied.len(), 2);
    assert_eq!(status.applied[0].version, 1);
    assert_eq!(status.applied[1].version, 2);
}

#[test]
fn test_migration_status_no_migrations() {
    let status = MigrationStatus {
        extension_id: "empty".to_string(),
        total_defined: 0,
        total_applied: 0,
        pending_count: 0,
        pending: vec![],
        applied: vec![],
    };

    assert_eq!(status.total_defined, 0);
    assert_eq!(status.total_applied, 0);
    assert!(status.pending.is_empty());
    assert!(status.applied.is_empty());
}

#[test]
fn test_migration_status_all_pending() {
    let status = MigrationStatus {
        extension_id: "fresh_install".to_string(),
        total_defined: 10,
        total_applied: 0,
        pending_count: 10,
        pending: vec![],
        applied: vec![],
    };

    assert_eq!(status.total_applied, 0);
    assert_eq!(status.pending_count, status.total_defined);
}
