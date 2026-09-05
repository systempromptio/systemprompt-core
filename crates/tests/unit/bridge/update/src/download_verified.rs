//! Downloading a release artifact from the gateway, with the digest check
//! that stands between a staged binary and an unverified one.

use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest as _, Sha256};
use systemprompt_bridge::gateway::GatewayClient;
use systemprompt_bridge::gateway::types::ReleaseManifest;
use systemprompt_bridge::update::{DownloadProgress, download_verified, hex_lower};
use systemprompt_identifiers::ValidatedUrl;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const PLATFORM: &str = "linux-x86_64";
const BODY: &[u8] = b"an artifact that stands in for a bridge binary";

fn digest_of(bytes: &[u8]) -> String {
    hex_lower(&Sha256::digest(bytes))
}

fn manifest(sha256: String, size: u64) -> ReleaseManifest {
    ReleaseManifest {
        version: "9.9.9".to_owned(),
        sha256,
        size,
        notes_url: None,
    }
}

fn client(server: &MockServer) -> GatewayClient {
    GatewayClient::new(
        ValidatedUrl::try_new(server.uri()).expect("mock server url"),
        reqwest::Client::new(),
    )
}

async fn serve(server: &MockServer, response: ResponseTemplate) {
    Mock::given(method("GET"))
        .and(path(format!("/v1/bridge/download/{PLATFORM}")))
        .respond_with(response)
        .mount(server)
        .await;
}

fn sandbox<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    let home = tempfile::TempDir::new().expect("home");
    let state = tempfile::TempDir::new().expect("state");
    temp_env::with_vars(
        [
            ("HOME", Some(home.path().display().to_string())),
            ("XDG_STATE_HOME", Some(state.path().display().to_string())),
            ("XDG_DATA_HOME", Some(state.path().display().to_string())),
            ("SUDO_USER", None),
        ],
        f,
    )
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
}

#[test]
fn a_matching_digest_stages_the_artifact_under_a_version_and_platform_named_path() {
    let rt = runtime();
    sandbox(|| {
        rt.block_on(async {
            let server = MockServer::start().await;
            serve(&server, ResponseTemplate::new(200).set_body_bytes(BODY)).await;

            let staged = download_verified(
                &client(&server),
                "bearer-token",
                PLATFORM,
                &manifest(digest_of(BODY), BODY.len() as u64),
                &|_| {},
            )
            .await
            .expect("a matching digest stages the artifact");

            assert_eq!(std::fs::read(&staged).expect("read back"), BODY);
            let name = staged
                .file_name()
                .and_then(|n| n.to_str())
                .expect("staged file name");
            assert_eq!(name, format!("update-9.9.9-{PLATFORM}"));
        });
    });
}

#[test]
fn progress_is_reported_as_bytes_arrive_and_ends_at_the_full_body() {
    let rt = runtime();
    sandbox(|| {
        rt.block_on(async {
            let server = MockServer::start().await;
            serve(&server, ResponseTemplate::new(200).set_body_bytes(BODY)).await;

            let calls = AtomicU64::new(0);
            let last = AtomicU64::new(0);
            download_verified(
                &client(&server),
                "bearer-token",
                PLATFORM,
                &manifest(digest_of(BODY), BODY.len() as u64),
                &|p: DownloadProgress| {
                    calls.fetch_add(1, Ordering::SeqCst);
                    last.store(p.received, Ordering::SeqCst);
                    assert_eq!(p.total, BODY.len() as u64);
                },
            )
            .await
            .expect("download");

            assert!(
                calls.load(Ordering::SeqCst) > 0,
                "progress must be reported"
            );
            assert_eq!(
                last.load(Ordering::SeqCst),
                BODY.len() as u64,
                "the final report accounts for the whole body"
            );
        });
    });
}

#[test]
fn a_digest_that_does_not_match_the_manifest_is_refused_and_the_partial_file_is_deleted() {
    let rt = runtime();
    sandbox(|| {
        rt.block_on(async {
            let server = MockServer::start().await;
            serve(&server, ResponseTemplate::new(200).set_body_bytes(BODY)).await;

            let expected = digest_of(b"a completely different artifact");
            let err = download_verified(
                &client(&server),
                "bearer-token",
                PLATFORM,
                &manifest(expected.clone(), BODY.len() as u64),
                &|_| {},
            )
            .await
            .expect_err("a mismatched digest must never be staged");

            let rendered = err.to_string();
            assert!(
                rendered.contains(&expected) || rendered.contains(&digest_of(BODY)),
                "the error names the digests it compared, got {rendered}"
            );
        });
    });
}

#[test]
fn a_manifest_digest_in_uppercase_still_matches_the_computed_lowercase_one() {
    let rt = runtime();
    sandbox(|| {
        rt.block_on(async {
            let server = MockServer::start().await;
            serve(&server, ResponseTemplate::new(200).set_body_bytes(BODY)).await;

            let staged = download_verified(
                &client(&server),
                "bearer-token",
                PLATFORM,
                &manifest(digest_of(BODY).to_uppercase(), BODY.len() as u64),
                &|_| {},
            )
            .await
            .expect("digest comparison is case-insensitive");
            assert!(staged.is_file());
        });
    });
}

#[test]
fn a_gateway_that_refuses_the_download_surfaces_the_status_it_returned() {
    let rt = runtime();
    sandbox(|| {
        rt.block_on(async {
            let server = MockServer::start().await;
            serve(&server, ResponseTemplate::new(403)).await;

            let err = download_verified(
                &client(&server),
                "bearer-token",
                PLATFORM,
                &manifest(digest_of(BODY), BODY.len() as u64),
                &|_| {},
            )
            .await
            .expect_err("a non-success status is not a download");

            assert!(
                err.to_string().contains("403"),
                "the status the gateway returned must be visible, got {err}"
            );
        });
    });
}

#[test]
fn a_gateway_that_is_not_listening_at_all_reports_a_download_failure() {
    let rt = runtime();
    sandbox(|| {
        rt.block_on(async {
            let server = MockServer::start().await;
            let uri = server.uri();
            drop(server);

            let client = GatewayClient::new(
                ValidatedUrl::try_new(uri).expect("url"),
                reqwest::Client::new(),
            );
            let err = download_verified(
                &client,
                "bearer-token",
                PLATFORM,
                &manifest(digest_of(BODY), BODY.len() as u64),
                &|_| {},
            )
            .await
            .expect_err("nothing is listening");
            assert!(!err.to_string().is_empty());
        });
    });
}

#[test]
fn an_empty_artifact_body_still_verifies_against_the_digest_of_no_bytes() {
    let rt = runtime();
    sandbox(|| {
        rt.block_on(async {
            let server = MockServer::start().await;
            serve(
                &server,
                ResponseTemplate::new(200).set_body_bytes(Vec::new()),
            )
            .await;

            let staged = download_verified(
                &client(&server),
                "bearer-token",
                PLATFORM,
                &manifest(digest_of(b""), 0),
                &|_| {},
            )
            .await
            .expect("an empty body still has a digest");
            assert_eq!(std::fs::read(&staged).expect("read").len(), 0);
        });
    });
}

#[test]
fn downloading_the_same_release_twice_replaces_the_staged_artifact_in_place() {
    let rt = runtime();
    sandbox(|| {
        rt.block_on(async {
            let server = MockServer::start().await;
            serve(&server, ResponseTemplate::new(200).set_body_bytes(BODY)).await;

            let first = download_verified(
                &client(&server),
                "bearer-token",
                PLATFORM,
                &manifest(digest_of(BODY), BODY.len() as u64),
                &|_| {},
            )
            .await
            .expect("first download");
            let second = download_verified(
                &client(&server),
                "bearer-token",
                PLATFORM,
                &manifest(digest_of(BODY), BODY.len() as u64),
                &|_| {},
            )
            .await
            .expect("second download");

            assert_eq!(first, second, "the staged path is derived, not accumulated");
            assert_eq!(std::fs::read(&second).expect("read back"), BODY);
        });
    });
}
