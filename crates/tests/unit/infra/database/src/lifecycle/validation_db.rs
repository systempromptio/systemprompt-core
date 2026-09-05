//! The pre-flight validators the boot path calls before it trusts a database:
//! connection liveness, primary-vs-standby, table presence and column presence.

use systemprompt_database::{
    PostgresProvider, validate_column_exists, validate_database_connection, validate_table_exists,
    validate_write_pool_is_primary,
};

use crate::services::db_helper::pool_or_skip;

async fn provider_or_skip() -> Option<PostgresProvider> {
    let db = pool_or_skip().await?;
    let pg = db.write_pool_arc().ok()?;
    Some(PostgresProvider::from_pool(pg))
}

#[tokio::test]
async fn a_live_pool_passes_the_connection_check() {
    let Some(provider) = provider_or_skip().await else {
        return;
    };

    validate_database_connection(&provider)
        .await
        .expect("a pool that answers must pass the pre-flight connection check");
}

#[tokio::test]
async fn a_writable_primary_passes_the_standby_check() {
    let Some(db) = pool_or_skip().await else {
        return;
    };

    validate_write_pool_is_primary(&db)
        .await
        .expect("a pool on a primary must not be rejected as a standby");
}

#[tokio::test]
async fn table_presence_distinguishes_a_migrated_table_from_an_absent_one() {
    let Some(provider) = provider_or_skip().await else {
        return;
    };

    assert!(
        validate_table_exists(&provider, "extension_migrations")
            .await
            .expect("the probe must not error for a table that exists"),
        "a table the migrations created must be reported present"
    );

    assert!(
        !validate_table_exists(&provider, "table_that_was_never_created")
            .await
            .expect("an absent table is a false result, not an error"),
        "an absent table must be reported absent rather than raising"
    );
}

#[tokio::test]
async fn column_presence_is_checked_within_the_named_table_only() {
    let Some(provider) = provider_or_skip().await else {
        return;
    };

    assert!(
        validate_column_exists(&provider, "extension_migrations", "checksum")
            .await
            .expect("probe"),
        "a column the schema declares must be reported present"
    );

    assert!(
        !validate_column_exists(&provider, "extension_migrations", "no_such_column")
            .await
            .expect("probe"),
        "a column absent from the table must be reported absent"
    );

    // `checksum` exists on `extension_migrations` but not on `users`: the probe
    // must be scoped to the table it was given, not to the schema at large.
    assert!(
        !validate_column_exists(&provider, "users", "checksum")
            .await
            .expect("probe"),
        "a column that exists on a different table must not count as present here"
    );
}

#[tokio::test]
async fn probing_a_column_on_an_absent_table_reports_absent() {
    let Some(provider) = provider_or_skip().await else {
        return;
    };

    assert!(
        !validate_column_exists(&provider, "table_that_was_never_created", "id")
            .await
            .expect("an absent table yields a false column probe, not an error"),
        "the column probe must not raise just because the table is gone"
    );
}

#[tokio::test]
async fn replica_status_reports_a_primary_with_no_lag() {
    let Some(provider) = provider_or_skip().await else {
        return;
    };

    let status = systemprompt_database::replica_status(&provider)
        .await
        .expect("replica status probe on a live primary");
    assert!(!status.in_recovery, "the fixture database is a primary");
    assert!(
        status.replay_lag_secs.is_none(),
        "a primary replays nothing, so it reports no lag"
    );
}
