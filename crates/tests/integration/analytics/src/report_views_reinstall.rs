//! A full schema install keeps the analytics `report_*` session views in
//! place: the `users` phase replaces `v_clean_traffic`, `v_engaged_traffic`
//! and `v_bot_sessions` without dropping them, so the reports built on them
//! never disappear between the `users` and `analytics` transactions. A reader
//! queries all three reports in a tight loop while the schema is installed
//! twice on the full path, and must never see an error.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use systemprompt_database::{MigrationConfig, SchemaName, install_admitted_schemas};
use systemprompt_test_fixtures::{DisposableDb, schema_admitted};

const REPORTS: [&str; 3] = [
    "SELECT count(*) FROM report_clean_traffic",
    "SELECT count(*) FROM report_engaged_traffic",
    "SELECT count(*) FROM report_bot_sessions",
];

#[tokio::test]
async fn a_full_reinstall_never_hides_the_session_reports_from_a_reader() -> anyhow::Result<()> {
    let db = DisposableDb::with_schema("report_views_reinstall").await;
    let admitted = schema_admitted()?;
    let installer = db.test_pool().await;
    let reader = db.test_pool().await.pool();

    let stop = Arc::new(AtomicBool::new(false));
    let queries = Arc::new(AtomicU64::new(0));
    let reading = {
        let stop = Arc::clone(&stop);
        let queries = Arc::clone(&queries);
        tokio::spawn(async move {
            let mut errors = Vec::new();
            while !stop.load(Ordering::Acquire) {
                for report in REPORTS {
                    match sqlx::query_scalar::<_, i64>(report)
                        .fetch_one(reader.as_ref())
                        .await
                    {
                        Ok(_) => {
                            queries.fetch_add(1, Ordering::Relaxed);
                        },
                        Err(error) => errors.push(format!("{report}: {error}")),
                    }
                }
            }
            errors
        })
    };

    let mut installed = Ok(());
    for _ in 0..2 {
        installed = install_admitted_schemas(
            admitted.built(),
            installer.write(),
            &SchemaName::public(),
            MigrationConfig {
                allow_checksum_drift: false,
                force_full_install: true,
            },
        )
        .await
        .map(|_| ());
        if installed.is_err() {
            break;
        }
    }
    stop.store(true, Ordering::Release);
    let errors = reading.await?;
    db.drop_now().await;

    installed?;
    assert!(
        queries.load(Ordering::Relaxed) > 0,
        "the reader ran no query"
    );
    assert!(
        errors.is_empty(),
        "a reader saw the session reports fail during a full install: {errors:?}"
    );
    Ok(())
}
