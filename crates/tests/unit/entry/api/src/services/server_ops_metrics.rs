use systemprompt_api::services::server::metrics::install_recorder;
use systemprompt_api::services::server::ops_metrics::OpsSampler;
use systemprompt_identifiers::InstanceId;
use systemprompt_loader::BootContext;
use systemprompt_test_fixtures::{ensure_test_bootstrap, test_db_pool};

fn boot_context() -> BootContext {
    ensure_test_bootstrap().boot.clone()
}

#[tokio::test]
async fn one_sample_records_the_acquire_probe_and_the_scheduler_backlog() {
    let handle =
        install_recorder(&InstanceId::new("fixture-instance"), &[]).expect("recorder installs");
    let db = test_db_pool().await;
    let projection = format!("ops_sampler_probe_{}", uuid::Uuid::new_v4().simple());
    let keys = vec!["k1".to_owned(), "k2".to_owned(), "k3".to_owned()];
    systemprompt_database::projection::queue(db.write_pool().as_ref(), &projection, &keys)
        .await
        .expect("backlog queues");

    let mut sampler = OpsSampler::new(
        boot_context(),
        &db,
        true,
        systemprompt_logging::LogSink::default(),
    );
    sampler.sample().await;
    let rendered = handle.render();

    sqlx::query("DELETE FROM projection_backlog WHERE projection = $1")
        .bind(&projection)
        .execute(db.write_pool().as_ref())
        .await
        .expect("backlog cleans up");

    assert!(
        rendered.contains("db_pool_acquire_seconds") && rendered.contains("pool=\"write\""),
        "the write pool probe records its wait; got: {rendered}"
    );
    let gauge_line = rendered
        .lines()
        .find(|line| {
            line.starts_with("projection_backlog_depth")
                && line.contains(&format!("projection=\"{projection}\""))
        })
        .unwrap_or_else(|| panic!("a scheduler node samples the backlog; got: {rendered}"));
    assert!(
        gauge_line.trim_end().ends_with(" 3"),
        "the gauge carries the queued key count: {gauge_line}"
    );
    assert!(rendered.contains("log_entries_dropped_total"));
}

#[tokio::test]
async fn a_non_scheduler_node_never_samples_the_backlog() {
    let handle =
        install_recorder(&InstanceId::new("fixture-instance"), &[]).expect("recorder installs");
    let db = test_db_pool().await;
    let projection = format!("ops_sampler_skip_{}", uuid::Uuid::new_v4().simple());
    systemprompt_database::projection::queue(
        db.write_pool().as_ref(),
        &projection,
        &["k1".to_owned()],
    )
    .await
    .expect("backlog queues");

    OpsSampler::new(
        boot_context(),
        &db,
        false,
        systemprompt_logging::LogSink::default(),
    )
    .sample()
    .await;
    let rendered = handle.render();

    sqlx::query("DELETE FROM projection_backlog WHERE projection = $1")
        .bind(&projection)
        .execute(db.write_pool().as_ref())
        .await
        .expect("backlog cleans up");

    assert!(
        !rendered.contains(&format!("projection=\"{projection}\"")),
        "only the scheduler node drains, so only it reports depth"
    );
}
