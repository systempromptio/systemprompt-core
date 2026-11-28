use chrono::{TimeDelta, Utc};
use systemprompt_device::client::SpClient;
use systemprompt_device::protocol::Pat;
use systemprompt_device::protocol::targets::rule_targets_host;
use systemprompt_harness::governance::UsageTotals;
use systemprompt_harness::sync::HeartbeatState;
use systemprompt_harness::sync::heartbeat::{Beat, Reporter, Tracker};
use systemprompt_models::bridge::host::HostKind;
use systemprompt_scripted_instance::{Event, ModelScript};

use crate::fixture::{Signed, http};

#[tokio::test(flavor = "multi_thread")]
async fn the_instance_is_discovered_and_a_pat_becomes_a_session() {
    let signed = Signed::start(ModelScript::empty()).await;
    let client = SpClient::new(signed.instance.base_url(), http()).unwrap();
    let probe = client.probe().await.unwrap();
    assert!(
        probe.discovery.discovered,
        "the instance serves no discovery document"
    );
    assert!(
        rule_targets_host(&probe.discovery.enabled_hosts, HostKind::Harness),
        "discovery does not enable the harness: {:?}",
        probe.discovery.enabled_hosts
    );

    let pat = Pat::new(signed.instance.pat().as_str()).unwrap();
    let session = client.mint_session(&pat, None).await.unwrap();
    assert!(!session.jwt.is_expired(Utc::now(), TimeDelta::zero()));
    assert!(
        session.session_id.is_some(),
        "the session token names no session"
    );
    let whoami = client.whoami(&session.jwt).await.unwrap();
    assert_eq!(
        whoami.user_id.as_ref(),
        Some(&signed.instance.user().user_id)
    );

    assert!(signed.report.status.is_success());
    let account = signed.accounts.get(&signed.account).unwrap();
    assert!(account.active);
    assert_eq!(
        signed.accounts.user_id(&signed.account).unwrap(),
        whoami.user_id
    );
    signed.accounts.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn the_heartbeat_is_accepted() {
    let signed = Signed::start(ModelScript::empty()).await;
    let answer = Reporter::new("0.0.0")
        .beat(&signed.accounts, &signed.account, UsageTotals::default())
        .await
        .unwrap();
    assert!(
        !answer.unsupported(),
        "the heartbeat reports this version unsupported: {answer:?}"
    );
    let mut tracker = Tracker::default();
    tracker.record(Beat::Answered(answer));
    assert_eq!(tracker.state(), &HeartbeatState::Online);
    signed.instance.wait_for(Event::Heartbeat).await.unwrap();
    signed.accounts.shutdown();
}
