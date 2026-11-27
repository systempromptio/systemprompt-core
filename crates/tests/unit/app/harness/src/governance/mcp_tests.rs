use std::sync::atomic::Ordering;

use systemprompt_engine::run::Governor;

use crate::governance::support::{header, rig};

#[tokio::test]
async fn the_bearer_goes_only_to_flagged_servers() {
    let rig = rig(true, 3_600);
    let servers = rig.governance.mcp_servers();
    assert_eq!(servers.len(), 2);
    let docs = servers[0].headers.headers().await.unwrap();
    assert_eq!(header(&docs, "x-team"), Some("alpha"));
    assert!(
        header(&docs, "authorization")
            .unwrap()
            .starts_with("Bearer ")
    );
    let other = servers[1].headers.headers().await.unwrap();
    assert_eq!(header(&other, "x-team"), Some("alpha"));
    assert!(header(&other, "authorization").is_none());

    rig.credentials.signed_out.store(true, Ordering::SeqCst);
    let without = servers[0].headers.headers().await.unwrap();
    assert!(header(&without, "authorization").is_none());
    rig.credentials.signed_out.store(false, Ordering::SeqCst);
    rig.credentials.expired.store(true, Ordering::SeqCst);
    let stale = servers[0].headers.headers().await.unwrap();
    assert!(header(&stale, "authorization").is_none());
    assert!(!format!("{:?}", servers[0]).contains("alpha"));
}
