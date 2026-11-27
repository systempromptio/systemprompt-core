use std::sync::atomic::Ordering;
use systemprompt_identifiers::engine::IdClock;

use systemprompt_device::protocol::DeviceCredential;
use systemprompt_engine::run::{Governor, Verdict};
use systemprompt_identifiers::engine::SessionId;

use crate::governance::support::{rig, tool};

#[tokio::test]
async fn hook_tokens_are_reused_until_near_expiry() {
    let rig = rig(true, 3_600);
    let session = SessionId::mint(&IdClock::new());
    assert_eq!(
        rig.governance.tool_before(&tool(&session)).await,
        Verdict::Allow
    );
    assert_eq!(
        rig.governance.tool_before(&tool(&session)).await,
        Verdict::Allow
    );
    let tokens = rig.api.verdict_tokens();
    assert_eq!(tokens.len(), 2);
    assert_eq!(tokens[0], tokens[1]);
    assert_eq!(rig.api.provisions.load(Ordering::SeqCst), 1);
    assert_eq!(rig.api.mints.load(Ordering::SeqCst), 1);

    let short = crate::governance::support::rig(true, 120);
    assert_eq!(
        short.governance.tool_before(&tool(&session)).await,
        Verdict::Allow
    );
    assert_eq!(
        short.governance.tool_before(&tool(&session)).await,
        Verdict::Allow
    );
    let tokens = short.api.verdict_tokens();
    assert_eq!(tokens.len(), 2);
    assert_ne!(tokens[0], tokens[1]);
    assert_eq!(short.api.provisions.load(Ordering::SeqCst), 1);
    assert_eq!(short.api.mints.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn a_refused_mint_provisions_the_client_once_more() {
    let rig = rig(true, 120);
    let session = SessionId::mint(&IdClock::new());
    assert_eq!(
        rig.governance.tool_before(&tool(&session)).await,
        Verdict::Allow
    );
    assert_eq!(rig.api.provisions.load(Ordering::SeqCst), 1);
    assert_eq!(rig.api.mints.load(Ordering::SeqCst), 1);

    rig.api.refuse_mints.store(1, Ordering::SeqCst);
    assert_eq!(
        rig.governance.tool_before(&tool(&session)).await,
        Verdict::Allow
    );
    assert_eq!(rig.api.provisions.load(Ordering::SeqCst), 2);
    assert_eq!(rig.api.mints.load(Ordering::SeqCst), 3);
    assert_eq!(rig.api.asked.load(Ordering::SeqCst), 2);

    rig.api.refuse_mints.store(2, Ordering::SeqCst);
    assert_eq!(
        rig.governance.tool_before(&tool(&session)).await,
        Verdict::Allow
    );
    assert_eq!(rig.api.provisions.load(Ordering::SeqCst), 3);
    assert_eq!(rig.api.asked.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn a_new_instance_drops_the_hook_credentials() {
    let rig = rig(true, 3_600);
    let session = SessionId::mint(&IdClock::new());
    assert_eq!(
        rig.governance.tool_before(&tool(&session)).await,
        Verdict::Allow
    );
    rig.reinstall();
    assert_eq!(
        rig.governance.tool_before(&tool(&session)).await,
        Verdict::Allow
    );
    assert_eq!(rig.api.provisions.load(Ordering::SeqCst), 2);
    assert_eq!(rig.api.mints.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn debug_output_names_no_secret() {
    let rig = rig(true, 3_600);
    rig.governance.set_device(Some(
        DeviceCredential::new("sp_device_secretvalue").unwrap(),
    ));
    assert_eq!(
        rig.governance
            .tool_before(&tool(&SessionId::mint(&IdClock::new())))
            .await,
        Verdict::Allow
    );
    assert_eq!(rig.api.verdict_tokens(), ["hook-1"]);
    let shown = format!("{:?} {:?}", rig.governance, rig.parts(true, None));
    assert!(shown.contains("sp.example.com"));
    assert!(!shown.contains("secretvalue"));
    assert!(!shown.contains("hook-1"));
    assert!(!shown.contains("client-secret"));
}
