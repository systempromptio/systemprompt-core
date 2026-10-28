use systemprompt_bridge::drive::run::ensure_enabled;
use systemprompt_bridge::drive::{DriveError, DriveHandle};
use systemprompt_models::ApiPaths;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support::Fixture;

async fn manifest_answers(server: &MockServer, status: u16) {
    Mock::given(method("GET"))
        .and(path(ApiPaths::BRIDGE_DRIVE_MANIFEST))
        .respond_with(ResponseTemplate::new(status))
        .mount(server)
        .await;
}

#[tokio::test]
async fn a_404_manifest_marks_drive_disabled_without_an_error() {
    let server = MockServer::start().await;
    let fx = Fixture::new(&server.uri());
    std::fs::remove_dir_all(fx.root.dir()).expect("remove drive dir");
    manifest_answers(&server, 404).await;
    let handle = DriveHandle::new();
    handle.update(|status| {
        status.enabled = true;
        status.last_error = Some("earlier".to_owned());
    });

    let enabled = ensure_enabled(fx.cycle(), &handle)
        .await
        .expect("a disabled server is not a failure");

    assert!(!enabled);
    let status = handle.status();
    assert!(!status.enabled);
    assert_eq!(status.last_error, None);
    assert!(!fx.root.dir().exists(), "no folder is created");

    let generation = *handle.subscribe().borrow();
    for _ in 0..2 {
        assert!(
            !ensure_enabled(fx.cycle(), &handle)
                .await
                .expect("still disabled")
        );
    }
    assert_eq!(
        *handle.subscribe().borrow(),
        generation,
        "a disabled server bumps the status only on the first observation"
    );
}

#[tokio::test]
async fn an_existing_folder_does_not_mark_drive_enabled_before_the_server_answers() {
    let server = MockServer::start().await;
    let fx = Fixture::new(&server.uri());
    let handle = DriveHandle::new();
    let generation = *handle.subscribe().borrow();

    assert!(
        ensure_enabled(fx.cycle(), &handle)
            .await
            .expect("folder exists")
    );

    assert!(!handle.status().enabled);
    assert_eq!(*handle.subscribe().borrow(), generation);
}

#[tokio::test]
async fn the_client_reports_a_404_manifest_as_server_disabled() {
    let server = MockServer::start().await;
    let fx = Fixture::new(&server.uri());
    manifest_answers(&server, 404).await;

    let err = fx
        .client
        .manifest(&fx.bearer)
        .await
        .expect_err("404 is not a manifest");

    assert!(matches!(err, DriveError::ServerDisabled { .. }), "{err:?}");
}
