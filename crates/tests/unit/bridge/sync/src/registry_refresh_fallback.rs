use systemprompt_bridge::gateway::GatewayError;
use systemprompt_bridge::sync::{KeptStored, RefreshFailure, SyncError, classify_refresh_failure};

async fn connect_error() -> reqwest::Error {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    drop(listener);
    reqwest::Client::new()
        .get(format!("http://127.0.0.1:{port}/v1/bridge/manifest"))
        .send()
        .await
        .expect_err("nothing listens on a released port")
}

#[tokio::test]
async fn an_unreachable_gateway_keeps_the_stored_envelope() {
    for error in [
        SyncError::Gateway(GatewayError::ManifestFetch(Box::new(connect_error().await))),
        SyncError::Gateway(GatewayError::PubkeyFetch(Box::new(connect_error().await))),
    ] {
        assert_eq!(
            classify_refresh_failure(&error),
            RefreshFailure::Kept(KeptStored::Unreachable),
            "{error}"
        );
    }
}

#[test]
fn a_device_without_a_credential_keeps_the_stored_envelope() {
    let error = SyncError::NoCredential { bin: "bridge" };
    assert_eq!(
        classify_refresh_failure(&error),
        RefreshFailure::Kept(KeptStored::NoCredential)
    );
}

#[test]
fn a_disabled_bridge_keeps_the_stored_envelope() {
    let error = SyncError::Disabled { bin: "bridge" };
    assert_eq!(
        classify_refresh_failure(&error),
        RefreshFailure::Kept(KeptStored::Disabled)
    );
}

#[test]
fn a_manifest_that_fails_verification_stops_the_repair() {
    for error in [
        SyncError::PubkeyNotPinned,
        SyncError::ReplayedManifest {
            last: "2".to_owned(),
            incoming: "1".to_owned(),
        },
        SyncError::ManifestSkew {
            not_before: "2030-01-01T00:00:00Z".to_owned(),
            now: "2026-01-01T00:00:00Z".to_owned(),
        },
        SyncError::Gateway(GatewayError::PubkeyMissing),
        SyncError::Gateway(GatewayError::ManifestNotModifiedWithoutCache),
    ] {
        assert_eq!(
            classify_refresh_failure(&error),
            RefreshFailure::Rejected,
            "{error}"
        );
    }
}
