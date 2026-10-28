use std::time::{Duration, Instant};

use systemprompt_bridge::drive::presence::{
    PRESENCE_WINDOW, PresenceThrottle, document_for, send_editing,
};
use systemprompt_identifiers::{DocumentId, DocumentRevisionId};
use systemprompt_models::ApiPaths;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support::Fixture;

#[test]
fn throttle_admits_one_beacon_per_document_per_window() {
    let mut throttle = PresenceThrottle::default();
    let a = DocumentId::generate();
    let b = DocumentId::generate();
    let t0 = Instant::now();
    assert!(throttle.admit(&a, t0));
    assert!(!throttle.admit(&a, t0 + Duration::from_secs(9)));
    assert!(throttle.admit(&b, t0 + Duration::from_secs(1)));
    assert!(throttle.admit(&a, t0 + PRESENCE_WINDOW));
}

#[tokio::test]
async fn touched_drive_file_sends_one_editing_beacon_within_the_window() {
    let server = MockServer::start().await;
    let fx = Fixture::new(&server.uri());
    let doc = DocumentId::generate();
    fx.write("sheet.xlsx", b"cells");
    fx.synced("sheet.xlsx", &doc, &DocumentRevisionId::generate());
    Mock::given(method("PUT"))
        .and(path(ApiPaths::BRIDGE_DRIVE_PRESENCE))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let state = fx.state();
    let mapped = document_for(&fx.root, &state, &fx.root.dir().join("sheet.xlsx")).expect("mapped");
    assert_eq!(mapped, doc);
    assert!(
        document_for(
            &fx.root,
            &state,
            std::path::Path::new("/elsewhere/sheet.xlsx")
        )
        .is_none()
    );

    let mut throttle = PresenceThrottle::default();
    let now = Instant::now();
    let first = send_editing(
        fx.cycle(),
        &mut throttle,
        vec![doc.clone(), doc.clone()],
        now,
    )
    .await
    .expect("send");
    let second = send_editing(
        fx.cycle(),
        &mut throttle,
        vec![doc.clone()],
        now + Duration::from_secs(5),
    )
    .await
    .expect("send");
    assert_eq!((first, second), (1, 0));

    let requests = server.received_requests().await.expect("requests");
    let beacon: serde_json::Value = serde_json::from_slice(&requests[0].body).expect("json");
    assert_eq!(beacon["state"], "editing");
    assert_eq!(beacon["source_kind"], "bridge");
    assert_eq!(beacon["document_id"], doc.as_str());
}
