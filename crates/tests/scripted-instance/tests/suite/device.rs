use std::time::Duration;

use chrono::Utc;
use serde_json::Value;
use systemprompt_device::client::{ClientError, ManifestFetch, open_manifest};
use systemprompt_device::protocol::{OneTimeCode, Pat};
use systemprompt_models::ApiPaths;
use systemprompt_models::bridge::heartbeat::{ClientState, HeartbeatRequest};
use systemprompt_scripted_instance::{
    Event, ModelScript, RecordedBody, Route, ScriptedInstanceError,
};

use crate::support::{client, instance, signed_in};

#[tokio::test]
async fn the_device_client_signs_in_and_fetches_the_manifest() {
    let instance = instance(ModelScript::empty()).await;
    let client = client(&instance);

    let probe = client.probe().await.unwrap();
    assert!(probe.discovery.discovered);
    assert!(probe.capabilities.offers("pat"));
    assert!(probe.version.is_some());

    let pat = Pat::new(instance.pat().as_str()).unwrap();
    let session = client.mint_session(&pat, None).await.unwrap();
    assert!(session.session_id.is_some());
    assert!(session.expires_at > Utc::now());

    let key = client.pubkey().await.unwrap();
    let fetched = client
        .manifest(&session.jwt, &key, None, false)
        .await
        .unwrap();
    let ManifestFetch::Fetched {
        envelope,
        manifest,
        etag,
    } = fetched
    else {
        panic!("the first fetch carries a manifest");
    };
    assert_eq!(manifest.user_id, instance.user().user_id);
    open_manifest(&envelope, &key).unwrap();

    let again = client
        .manifest(&session.jwt, &key, etag.as_deref(), false)
        .await
        .unwrap();
    assert!(matches!(again, ManifestFetch::NotModified));

    let routes: Vec<Route> = instance.requests().iter().map(|r| r.route).collect();
    assert!(routes.contains(&Route::PatSession));
    assert_eq!(routes.iter().filter(|r| **r == Route::Manifest).count(), 2);
    let record = instance.wait_for(Event::ManifestFetch).await.unwrap();
    assert!(record.headers.contains_key("authorization"));
    assert!(!record.unscripted);
}

#[tokio::test]
async fn a_session_continues_the_session_it_names() {
    let instance = instance(ModelScript::empty()).await;
    let (client, first) = signed_in(&instance).await;
    let pat = Pat::new(instance.pat().as_str()).unwrap();
    let second = client
        .mint_session(&pat, first.session_id.as_ref())
        .await
        .unwrap();
    assert_eq!(second.session_id, first.session_id);
}

#[tokio::test]
async fn the_paste_code_flow_completes_without_a_browser() {
    let instance = instance(ModelScript::empty()).await;
    let client = client(&instance);
    let page = client
        .discover()
        .await
        .unwrap()
        .device_link_url(None)
        .unwrap();
    let shown: Value = reqwest::get(page).await.unwrap().json().await.unwrap();
    let code = OneTimeCode::new(shown["code"].as_str().unwrap()).unwrap();

    let pat = client
        .exchange_code(&code, Some("a test device"))
        .await
        .unwrap();
    assert_eq!(pat.expose(), instance.pat().as_str());
    let reused = client.exchange_code(&code, None).await.unwrap_err();
    assert!(reused.is_unauthorized());
    assert!(
        instance
            .wait_for(Event::Route(Route::CodeExchange))
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn the_loopback_flow_returns_the_code_to_the_redirect() {
    let instance = instance(ModelScript::empty()).await;
    let discovery = client(&instance).discover().await.unwrap();
    let redirect = "http://127.0.0.1:9/callback?state=s3".parse().unwrap();
    let page = discovery.device_link_url(Some(&redirect)).unwrap();
    let http = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let answer = http.get(page).send().await.unwrap();
    assert_eq!(answer.status().as_u16(), 303);
    let location = answer.headers()["location"].to_str().unwrap();
    assert!(location.starts_with("http://127.0.0.1:9/callback?state=s3&code="));

    let foreign = format!(
        "{}{}?redirect=http://example.test/cb",
        instance.base_url(),
        ApiPaths::BRIDGE_DEVICE_LINK_FALLBACK
    );
    assert_eq!(
        http.get(foreign).send().await.unwrap().status().as_u16(),
        400
    );
}

#[tokio::test]
async fn identity_catalog_profile_and_heartbeat_answer_a_session() {
    let instance = instance(ModelScript::empty()).await;
    let (client, session) = signed_in(&instance).await;

    let whoami = client.whoami(&session.jwt).await.unwrap();
    assert_eq!(whoami.user_id.as_ref(), Some(&instance.user().user_id));
    let catalog = client.models(&session.jwt).await.unwrap();
    assert!(catalog.get("m-a").is_some() && catalog.get("m-b").is_some());
    let profile = client.profile(&session.jwt).await.unwrap();
    assert_eq!(profile.models.len(), 2);
    assert_eq!(profile.model_limits.len(), 2);
    assert_eq!(catalog.get("m-a").unwrap().max_input_tokens, Some(200_000));
    assert_eq!(profile.inference_gateway_base_url, instance.base_url());

    let beat = HeartbeatRequest {
        session_id: session.session_id.clone().unwrap(),
        bridge_version: "0.0.0".to_owned(),
        os: "test".to_owned(),
        hostname: "test".to_owned(),
        last_activity_at: None,
        forwarded_total: 0,
        tokens_in_total: 0,
        tokens_out_total: 0,
        release_version: "0.0.0".to_owned(),
        state: ClientState::Active,
    };
    let answer = client.heartbeat(&session.jwt, &beat).await.unwrap();
    assert!(!answer.unsupported());
    let record = instance.wait_for(Event::Heartbeat).await.unwrap();
    let RecordedBody::Json(body) = record.body else {
        panic!("the heartbeat body is recorded as JSON");
    };
    assert_eq!(body["session_id"], session.session_id.unwrap().as_str());
}

#[tokio::test]
async fn a_credential_the_instance_did_not_issue_is_refused() {
    let instance = instance(ModelScript::empty()).await;
    let client = client(&instance);
    let stranger = Pat::new("sp-live-other.pat").unwrap();
    let refused = client.mint_session(&stranger, None).await.unwrap_err();
    assert!(refused.is_unauthorized());

    let other = crate::support::instance(ModelScript::empty()).await;
    let (_, foreign) = signed_in(&other).await;
    let refused: ClientError = client.whoami(&foreign.jwt).await.unwrap_err();
    assert!(refused.is_unauthorized());
}

#[tokio::test]
async fn an_unserved_route_and_a_plan_without_a_device_are_recorded_and_refused() {
    let instance = instance(ModelScript::empty()).await;
    let url = format!("{}/not/a/route?x=1", instance.base_url());
    assert_eq!(reqwest::get(url).await.unwrap().status().as_u16(), 404);
    let plan = format!(
        "{}{}/r/publications/p/bundle",
        instance.base_url(),
        ApiPaths::CONSUMER_RESOURCES
    );
    assert_eq!(reqwest::get(plan).await.unwrap().status().as_u16(), 401);

    let requests = instance.requests();
    assert_eq!(requests[0].route, Route::Unrouted);
    assert_eq!(requests[0].query.as_deref(), Some("x=1"));
    assert_eq!(requests[0].body, RecordedBody::Empty);
    assert_eq!(requests[1].route, Route::Plan);
    assert_eq!((requests[0].sequence, requests[1].sequence), (0, 1));
    assert!(instance.unscripted_requests().is_empty());
    instance.shutdown().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn a_wait_nothing_meets_times_out() {
    let instance = instance(ModelScript::empty()).await;
    let waited = instance.wait_for(Event::Receipt).await.unwrap_err();
    let ScriptedInstanceError::WaitTimedOut { event, waited } = waited else {
        panic!("an unmet wait is a timeout");
    };
    assert_eq!(event, Event::Receipt);
    assert!(waited >= Duration::from_secs(30));
}
