use std::sync::Arc;
use std::time::Duration;

use systemprompt_bridge::config::{Config, shared_from_config};
use systemprompt_bridge::drive::run::{DriveDeps, run_loop};
use systemprompt_bridge::drive::{DriveHandle, DriveStatus};
use systemprompt_bridge::gateway::types::HelperOutput;
use systemprompt_bridge::ids::BearerToken;
use systemprompt_bridge::proxy::drive_signal::{
    DocumentNotice, DriveSignal, DriveSignals, NoticeKind,
};
use systemprompt_bridge::proxy::forward::ForwardError;
use systemprompt_bridge::proxy::token_cache::{RefreshFn, TokenCache};
use systemprompt_identifiers::{
    ContentHash, DocumentId, DocumentRevisionId, DriveDeviceId, UserId, ValidatedUrl,
};
use systemprompt_models::ApiPaths;
use systemprompt_models::documents::{
    ActorKind, DocumentPresenceEvent, DocumentRevisionCreatedEvent, PresenceSource, PresenceState,
    RevisionOriginKind,
};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support::{Fixture, manifest_json};

fn token_ok() -> RefreshFn {
    Arc::new(|_| {
        Box::pin(async {
            Ok(HelperOutput {
                token: BearerToken::new("loop-bearer"),
                ttl: 3600,
                headers: Default::default(),
                device_token: Some(BearerToken::new("device-bearer")),
            })
        })
    })
}

fn token_refused() -> RefreshFn {
    Arc::new(|_| Box::pin(async { Err(ForwardError::Auth("no credential".into())) }))
}

fn sandboxed<F: std::future::Future<Output = ()>>(f: F) {
    let temp = tempfile::tempdir().expect("config tempdir");
    let config = temp.path().join("config.toml");
    std::fs::write(&config, "").expect("config");
    temp_env::with_vars(
        [
            ("SP_BRIDGE_CONFIG", Some(config.into_os_string())),
            ("XDG_CONFIG_HOME", Some(temp.path().as_os_str().to_owned())),
            ("SP_BRIDGE_PAT", Some("sp-live-a.b".into())),
        ],
        || {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("runtime")
                .block_on(f)
        },
    );
}

struct Running {
    handle: Arc<DriveHandle>,
    signals: DriveSignals,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Running {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn spawn(fx: &Fixture, server: &MockServer, refresh: RefreshFn) -> Running {
    let handle = DriveHandle::new();
    let (signals, receiver) = DriveSignals::channel();
    let cfg = Config {
        gateway_url: Some(ValidatedUrl::try_new(server.uri()).expect("url")),
        ..Config::default()
    };
    let deps = DriveDeps {
        handle: Arc::clone(&handle),
        root: fx.root.clone(),
        runtime_config: shared_from_config(&cfg),
        token_cache: Arc::new(TokenCache::new(refresh)),
        http: reqwest::Client::new(),
        device_id: DriveDeviceId::try_new("device-loop").expect("device"),
        host_name: "laptop".to_owned(),
        signals: receiver,
    };
    assert!(format!("{deps:?}").contains("DriveDeps"));
    Running {
        handle,
        signals,
        task: tokio::spawn(run_loop(deps)),
    }
}

async fn until(handle: &DriveHandle, what: &str, pred: impl Fn(&DriveStatus) -> bool) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    while !pred(&handle.status()) {
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for {what}: {:?}",
            handle.status()
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

/// The Drive sync token the loop trades its bridge token for before any
/// other Drive call.
async fn serve_sync_token(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path(ApiPaths::BRIDGE_DRIVE_TOKEN))
        .and(wiremock::matchers::header(
            "authorization",
            "Bearer device-bearer",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "token": "drive-sync",
            "expires_at": "2999-01-01T00:00:00Z",
        })))
        .mount(server)
        .await;
}

async fn serve(server: &MockServer, route: &'static str, status: u16, body: serde_json::Value) {
    Mock::given(method("GET"))
        .and(path(route))
        .respond_with(ResponseTemplate::new(status).set_body_json(body))
        .mount(server)
        .await;
}

#[test]
fn the_loop_syncs_and_folds_proxy_signals_into_the_status() {
    sandboxed(async {
        let server = MockServer::start().await;
        let fx = Fixture::new(&server.uri());
        let doc = DocumentId::generate();
        let rev = DocumentRevisionId::generate();
        fx.write("plan.md", b"# plan");
        serve_sync_token(&server).await;
        serve(
            &server,
            ApiPaths::BRIDGE_DRIVE_MANIFEST,
            200,
            manifest_json(&[(&doc, "plan.md", &rev, b"# plan")]),
        )
        .await;
        serve(
            &server,
            ApiPaths::BRIDGE_DRIVE_CHANGES,
            200,
            serde_json::json!({"changes": [], "next_cursor": "c-next"}),
        )
        .await;
        serve(
            &server,
            ApiPaths::BRIDGE_DRIVE_PROPOSALS,
            200,
            serde_json::json!({"proposals": []}),
        )
        .await;
        Mock::given(method("PUT"))
            .and(path(ApiPaths::BRIDGE_DRIVE_PRESENCE))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let run = spawn(&fx, &server, token_ok());
        until(&run.handle, "first cycle", |s| s.synced_at.is_some()).await;
        let status = run.handle.status();
        assert!(status.enabled);
        assert_eq!(status.last_error, None);
        assert_eq!(fx.state().entries["plan.md"].revision_id, rev);

        run.signals
            .send(DriveSignal::Notice(Box::new(DocumentNotice {
                kind: NoticeKind::ProposalOpened,
                document_id: doc.clone(),
                from: UserId::new("bob"),
                from_name: None,
                document_name: None,
                proposal_id: None,
                detail: Some("tighten".to_owned()),
                message: None,
            })));
        run.signals
            .send(DriveSignal::Presence(Box::new(DocumentPresenceEvent {
                document_id: doc.clone(),
                user_id: UserId::new("bob"),
                state: PresenceState::Editing,
                source_kind: PresenceSource::Web,
                source_ref: "tab".to_owned(),
                cursor: None,
            })));
        until(&run.handle, "notice and presence", |s| {
            s.notices.len() == 1 && s.editors.len() == 1
        })
        .await;
        let status = run.handle.status();
        assert_eq!(status.editors[0].path.as_deref(), Some("plan.md"));
        assert_eq!(status.notices[0].notice.kind, NoticeKind::ProposalOpened);

        run.signals
            .send(DriveSignal::Touched(fx.root.dir().join("plan.md")));
        run.signals.send(DriveSignal::Touched(
            std::env::temp_dir().join("elsewhere.md"),
        ));
        run.signals.send(DriveSignal::RevisionCreated(Box::new(
            DocumentRevisionCreatedEvent {
                document_id: doc.clone(),
                revision_id: DocumentRevisionId::generate(),
                parent_revision_id: None,
                content_sha256: ContentHash::try_new("0".repeat(64)).expect("hash"),
                author_id: UserId::new("bob"),
                actor_kind: ActorKind::User,
                origin_kind: RevisionOriginKind::WebSession,
            },
        )));

        let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        loop {
            let beacons = server
                .received_requests()
                .await
                .expect("recorded")
                .into_iter()
                .filter(|r| r.method.as_str() == "PUT")
                .count();
            if beacons > 0 {
                break;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "no editing beacon sent"
            );
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        assert_eq!(fx.state().cursor.as_deref(), Some("c-next"));
    });
}

#[test]
fn a_server_without_the_document_store_turns_the_status_off_without_an_error() {
    sandboxed(async {
        let server = MockServer::start().await;
        let fx = Fixture::new(&server.uri());
        serve_sync_token(&server).await;
        serve(
            &server,
            ApiPaths::BRIDGE_DRIVE_MANIFEST,
            404,
            serde_json::json!({}),
        )
        .await;

        let run = spawn(&fx, &server, token_ok());
        run.handle.update(|s| {
            s.enabled = true;
            s.last_error = Some("earlier".to_owned());
        });
        until(&run.handle, "disabled", |s| {
            !s.enabled && s.last_error.is_none()
        })
        .await;
        assert!(run.handle.status().synced_at.is_none());
    });
}

#[test]
fn a_rejected_bearer_is_recorded_as_the_last_error() {
    sandboxed(async {
        let server = MockServer::start().await;
        let fx = Fixture::new(&server.uri());
        serve_sync_token(&server).await;
        serve(
            &server,
            ApiPaths::BRIDGE_DRIVE_MANIFEST,
            401,
            serde_json::json!({}),
        )
        .await;

        let run = spawn(&fx, &server, token_ok());
        until(&run.handle, "an error", |s| s.last_error.is_some()).await;
        assert!(run.handle.status().synced_at.is_none());
    });
}

#[test]
fn a_token_that_cannot_be_minted_is_recorded_and_no_request_is_sent() {
    sandboxed(async {
        let server = MockServer::start().await;
        let fx = Fixture::new(&server.uri());

        let run = spawn(&fx, &server, token_refused());
        until(&run.handle, "an error", |s| s.last_error.is_some()).await;
        assert!(
            server
                .received_requests()
                .await
                .expect("recorded")
                .is_empty()
        );
    });
}

#[test]
fn the_drive_routes_run_under_the_traded_sync_token() {
    sandboxed(async {
        let server = MockServer::start().await;
        let fx = Fixture::new(&server.uri());
        serve_sync_token(&server).await;
        Mock::given(method("GET"))
            .and(path(ApiPaths::BRIDGE_DRIVE_MANIFEST))
            .and(wiremock::matchers::header(
                "authorization",
                "Bearer drive-sync",
            ))
            .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({})))
            .mount(&server)
            .await;

        let _run = spawn(&fx, &server, token_ok());
        let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        let manifest = loop {
            let requests = server.received_requests().await.expect("recorded");
            if let Some(found) = requests
                .into_iter()
                .find(|r| r.url.path() == ApiPaths::BRIDGE_DRIVE_MANIFEST)
            {
                break found;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "the manifest was never asked for"
            );
            tokio::time::sleep(Duration::from_millis(25)).await;
        };
        assert_eq!(
            manifest
                .headers
                .get("authorization")
                .and_then(|v| v.to_str().ok()),
            Some("Bearer drive-sync"),
            "not the bridge token"
        );
    });
}
