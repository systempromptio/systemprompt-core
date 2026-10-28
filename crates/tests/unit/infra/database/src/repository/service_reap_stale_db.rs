//! `ServiceRepository::reap_stale` with a fake liveness: `error` rows, pid-less
//! `running` rows and `running` rows whose pid is not ours are reaped; the row
//! whose pid the liveness check owns survives. Also the `services.status`
//! CHECK constraint.

use systemprompt_database::{CreateServiceInput, ServiceModule, ServiceRepository, ServiceStatus};
use systemprompt_identifiers::{InstanceId, ServiceName};
use systemprompt_test_fixtures::{LivePids, test_db_pool};

fn unique(prefix: &str) -> String {
    format!("{prefix}_{}", uuid::Uuid::new_v4().simple())
}

async fn seed(
    repo: &ServiceRepository,
    prefix: &str,
    module: ServiceModule,
    status: ServiceStatus,
    pid: Option<i32>,
) -> ServiceName {
    let name = ServiceName::new(unique(prefix));
    repo.create_service(CreateServiceInput {
        name: &name,
        module_name: module,
        status,
        port: 7000,
        binary_mtime: None,
    })
    .await
    .expect("create service");
    if let Some(pid) = pid {
        repo.update_service_pid(&name, pid).await.expect("set pid");
    }
    name
}

#[tokio::test]
async fn only_the_row_whose_pid_is_ours_survives() {
    let db = test_db_pool().await;
    let repo = ServiceRepository::new(&db, InstanceId::new(unique("reap")));
    let errored = seed(
        &repo,
        "err",
        ServiceModule::Mcp,
        ServiceStatus::Error,
        Some(41),
    )
    .await;
    let pidless = seed(
        &repo,
        "nopid",
        ServiceModule::Mcp,
        ServiceStatus::Running,
        None,
    )
    .await;
    let dead = seed(
        &repo,
        "dead",
        ServiceModule::Agent,
        ServiceStatus::Running,
        Some(42),
    )
    .await;
    let live = seed(
        &repo,
        "live",
        ServiceModule::Mcp,
        ServiceStatus::Running,
        Some(43),
    )
    .await;
    let stopped = seed(
        &repo,
        "stopped",
        ServiceModule::Mcp,
        ServiceStatus::Stopped,
        None,
    )
    .await;

    let reaped = repo.reap_stale(&LivePids::of([43])).await.expect("reap");

    assert_eq!(
        reaped.stale_rows, 2,
        "the error row and the pid-less running row"
    );
    assert_eq!(reaped.dead.len(), 1);
    assert_eq!(reaped.dead[0].name, dead);
    assert_eq!(reaped.dead[0].module_name, ServiceModule::Agent);
    assert_eq!(reaped.dead[0].pid, 42);
    assert_eq!(reaped.total(), 3);

    for gone in [&errored, &pidless, &dead] {
        assert!(repo.find_service_by_name(gone).await.unwrap().is_none());
    }
    assert!(repo.find_service_by_name(&live).await.unwrap().is_some());
    assert!(
        repo.find_service_by_name(&stopped).await.unwrap().is_some(),
        "a stopped row claims no process and is not stale"
    );

    let again = repo.reap_stale(&LivePids::of([43])).await.expect("reap");
    assert_eq!(again.total(), 0);
}

#[tokio::test]
async fn reaping_never_crosses_instances() {
    let db = test_db_pool().await;
    let mine = ServiceRepository::new(&db, InstanceId::new(unique("mine")));
    let theirs = ServiceRepository::new(&db, InstanceId::new(unique("theirs")));
    let their_dead = seed(
        &theirs,
        "dead",
        ServiceModule::Mcp,
        ServiceStatus::Running,
        Some(99),
    )
    .await;

    let reaped = mine.reap_stale(&LivePids::none()).await.expect("reap");
    assert_eq!(reaped.total(), 0);
    assert!(
        theirs
            .find_service_by_name(&their_dead)
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(
        theirs.reap_stale(&LivePids::none()).await.unwrap().total(),
        1
    );
}

#[tokio::test]
async fn the_status_column_refuses_values_outside_the_vocabulary() {
    let db = test_db_pool().await;
    let instance = unique("check");
    let err = sqlx::query(
        "INSERT INTO services (instance_id, name, module_name, status, port)
         VALUES ($1, $2, 'mcp', 'bogus', 7001)",
    )
    .bind(&instance)
    .bind(unique("svc"))
    .execute(db.write_pool().as_ref())
    .await
    .expect_err("a status outside the CHECK set must be refused");
    let db_err = err.as_database_error().expect("database error");
    assert_eq!(
        db_err.constraint(),
        Some("services_status_check"),
        "{db_err}"
    );
}

#[tokio::test]
async fn a_running_row_with_a_negative_pid_is_reaped_as_pid_zero() {
    let db = test_db_pool().await;
    let repo = ServiceRepository::new(&db, InstanceId::new(unique("negpid")));
    let name = seed(
        &repo,
        "neg",
        ServiceModule::Mcp,
        ServiceStatus::Running,
        Some(-7),
    )
    .await;

    let reaped = repo.reap_stale(&LivePids::of([7])).await.expect("reap");

    assert_eq!(reaped.dead.len(), 1);
    assert_eq!(reaped.dead[0].name, name);
    assert_eq!(reaped.dead[0].pid, 0);
    assert!(repo.find_service_by_name(&name).await.unwrap().is_none());
}

#[tokio::test]
async fn an_unknown_module_name_stops_the_reap_with_a_decode_error() {
    let db = test_db_pool().await;
    let instance = unique("badmod");
    let repo = ServiceRepository::new(&db, InstanceId::new(instance.clone()));
    let name = unique("badmod_svc");
    sqlx::query(
        "INSERT INTO services (instance_id, name, module_name, pid, port, status) VALUES ($1, $2, \
         'not-a-module', 55, 7000, 'running')",
    )
    .bind(&instance)
    .bind(&name)
    .execute(&*db.write_pool())
    .await
    .expect("seed row");

    let err = repo
        .reap_stale(&LivePids::of([]))
        .await
        .expect_err("undecodable module");

    assert!(
        format!("{err:?}").contains("services.module_name"),
        "{err:?}"
    );
    let _ = sqlx::query("DELETE FROM services WHERE instance_id = $1")
        .bind(&instance)
        .execute(&*db.write_pool())
        .await;
}
