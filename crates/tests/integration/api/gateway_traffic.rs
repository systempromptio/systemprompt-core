//! The built-in `traffic` dimension: every caller is attributed `production`,
//! `evaluation` is reachable only through a key bound to it, the class is
//! stamped on `ai_requests.traffic`, and the hourly rollup keys on it.

use std::sync::Arc;

use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use systemprompt_api::routes::gateway::messages::error::RejectionError;
use systemprompt_api::routes::gateway::messages::extract::RejectionPartial;
use systemprompt_api::routes::gateway::messages::extract::scope::{
    ScopeHeaders, ScopeResolution, resolve_scope_attribution,
};
use systemprompt_api::routes::gateway::messages::rejection::persist_rejection;
use systemprompt_governance::authz::{
    ScopeBindingError, SubjectAttributeProvider, SubjectProviderSet, TrafficProvider,
};
use systemprompt_identifiers::{AiRequestId, ApiKeyId, ScopeDimension, UserId};
use systemprompt_models::attribution::{
    AttributionEntry, AttributionSource, RequestAttribution, ScopeBinding, TrafficClass,
};
use systemprompt_models::origin::{
    ClientAttestation, ClientKind, InboundWireProtocol, RequestOrigin,
};
use systemprompt_test_fixtures::seed_admin_credential;

use super::common::setup_ctx;
use super::gateway_pipeline::gw_repos;

const HEADER: &str = "x-systemprompt-scope-traffic";

fn traffic_only() -> SubjectProviderSet {
    SubjectProviderSet::from_providers(vec![Arc::new(TrafficProvider)])
}

fn partial() -> RejectionPartial {
    RejectionPartial::new(RequestOrigin::gateway(
        ClientKind::Other,
        InboundWireProtocol::AnthropicMessages,
        ClientAttestation::None,
    ))
}

async fn resolve(
    header: Option<&str>,
    bindings: &[ScopeBinding],
    partial: &mut RejectionPartial,
) -> Result<RequestAttribution, RejectionError> {
    let providers = traffic_only();
    let mut map = HeaderMap::new();
    if let Some(value) = header {
        map.insert(
            HeaderName::from_static(HEADER),
            HeaderValue::from_str(value).expect("header value"),
        );
    }
    let scope_headers = ScopeHeaders::capture(&map)?;
    let api_key_id = ApiKeyId::new("key-traffic");
    let subject = providers
        .gather(&UserId::new("traffic-user"))
        .await
        .expect("subject attributes");
    resolve_scope_attribution(
        ScopeResolution {
            providers: &providers,
            subject: &subject,
            headers: &scope_headers,
            key_bindings: bindings,
            api_key_id: Some(&api_key_id),
            require: &[],
        },
        partial,
    )
}

fn traffic_entry(attribution: &RequestAttribution) -> (&str, AttributionSource) {
    let entry = attribution
        .entries
        .iter()
        .find(|e| e.dimension.as_str() == TrafficClass::DIMENSION)
        .expect("traffic attributed");
    (entry.value.as_str(), entry.source)
}

#[tokio::test]
async fn every_user_holds_production_and_evaluation_is_key_bound() {
    let provider = TrafficProvider;
    assert_eq!(provider.dimension().rule_type.as_str(), "traffic");
    assert_eq!(
        provider
            .values_for(&UserId::new("anyone"))
            .await
            .expect("values"),
        vec!["production".to_owned()]
    );
    assert_eq!(provider.key_bound_values(), &["evaluation"]);
    assert!(provider.default_attribution());
}

#[test]
fn traffic_class_parses_only_its_two_values() {
    assert_eq!(
        TrafficClass::parse("production"),
        Some(TrafficClass::Production)
    );
    assert_eq!(
        TrafficClass::parse("evaluation"),
        Some(TrafficClass::Evaluation)
    );
    assert_eq!(TrafficClass::parse("synthetic"), None);
    assert_eq!(
        TrafficClass::of(&RequestAttribution::none()),
        Some(TrafficClass::Production)
    );
    let unknown = RequestAttribution {
        entries: vec![AttributionEntry {
            dimension: ScopeDimension::new("traffic"),
            value: "replay".to_owned(),
            source: AttributionSource::Header,
        }],
        api_key_id: None,
    };
    assert_eq!(TrafficClass::of(&unknown), None);
    assert_eq!(
        ScopeBinding::traffic(TrafficClass::Evaluation),
        ScopeBinding {
            dimension: ScopeDimension::new("traffic"),
            value: "evaluation".to_owned(),
        }
    );
}

#[tokio::test]
async fn the_registered_traffic_provider_is_discovered() {
    let (pool, _ctx) = setup_ctx().await.expect("ctx");
    assert!(gw_repos(&pool).subject_providers.find("traffic").is_some());
}

#[tokio::test]
async fn no_header_and_no_binding_is_production_by_default() {
    let mut partial = partial();
    let attribution = resolve(None, &[], &mut partial).await.expect("resolves");
    assert_eq!(
        traffic_entry(&attribution),
        ("production", AttributionSource::Default)
    );
}

#[tokio::test]
async fn an_unbound_caller_cannot_claim_evaluation_by_header() {
    let mut partial = partial();
    let error = resolve(Some("evaluation"), &[], &mut partial)
        .await
        .expect_err("unbound evaluation claim is refused");
    assert_eq!(error.status, StatusCode::FORBIDDEN);
    assert_eq!(
        TrafficClass::of(&partial.attribution),
        Some(TrafficClass::Production)
    );
}

#[tokio::test]
async fn an_unknown_traffic_value_is_forbidden() {
    let mut partial = partial();
    let error = resolve(Some("replay"), &[], &mut partial)
        .await
        .expect_err("unknown value is refused");
    assert_eq!(error.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn a_key_bound_to_evaluation_is_attributed_evaluation() {
    let bindings = [ScopeBinding::traffic(TrafficClass::Evaluation)];
    let mut partial = partial();
    let attribution = resolve(None, &bindings, &mut partial)
        .await
        .expect("resolves");
    assert_eq!(
        traffic_entry(&attribution),
        ("evaluation", AttributionSource::ApiKey)
    );
    let mut partial = self::partial();
    let attribution = resolve(Some("evaluation"), &bindings, &mut partial)
        .await
        .expect("a header repeating the binding resolves");
    assert_eq!(
        traffic_entry(&attribution),
        ("evaluation", AttributionSource::Header)
    );
}

#[tokio::test]
async fn a_key_bound_to_production_cannot_claim_evaluation() {
    let bindings = [ScopeBinding::traffic(TrafficClass::Production)];
    let mut partial = partial();
    let error = resolve(Some("evaluation"), &bindings, &mut partial)
        .await
        .expect_err("refused");
    assert_eq!(error.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn key_issuance_accepts_evaluation_and_refuses_unknown_values() {
    let providers = traffic_only();
    let owner = UserId::new("any-owner");
    providers
        .verify_scope_bindings(&owner, &[ScopeBinding::traffic(TrafficClass::Evaluation)])
        .await
        .expect("evaluation binds on the issuer's authority");
    let refused = providers
        .verify_scope_bindings(
            &owner,
            &[ScopeBinding {
                dimension: ScopeDimension::new("traffic"),
                value: "replay".to_owned(),
            }],
        )
        .await
        .expect_err("unknown value refused");
    assert!(matches!(refused, ScopeBindingError::NotAMember { .. }));
}

async fn persist(attribution: RequestAttribution) -> anyhow::Result<String> {
    let (pool, _ctx) = setup_ctx().await?;
    let cred = seed_admin_credential(&pool, "traffic-stamp@example.invalid").await?;
    let repos = gw_repos(&pool);
    let id = AiRequestId::generate();
    let mut partial = partial();
    partial.user_id = Some(cred.user_id.clone());
    partial.session_id = Some(cred.session_id.clone());
    partial.attribution = attribution;
    persist_rejection(
        &repos,
        &id,
        &partial,
        StatusCode::TOO_MANY_REQUESTS,
        "quota",
    )
    .await;
    let (traffic,): (String,) = sqlx::query_as("SELECT traffic FROM ai_requests WHERE id = $1")
        .bind(id.as_str())
        .fetch_one(pool.pool().as_ref())
        .await?;
    Ok(traffic)
}

#[tokio::test]
async fn a_request_attributed_evaluation_is_stamped_evaluation() -> anyhow::Result<()> {
    let traffic = persist(RequestAttribution {
        entries: vec![AttributionEntry {
            dimension: ScopeDimension::new("traffic"),
            value: "evaluation".to_owned(),
            source: AttributionSource::ApiKey,
        }],
        api_key_id: Some(ApiKeyId::new("key-eval")),
    })
    .await?;
    assert_eq!(traffic, "evaluation");
    Ok(())
}

#[tokio::test]
async fn a_request_with_no_traffic_attribution_is_stamped_production() -> anyhow::Result<()> {
    assert_eq!(persist(RequestAttribution::none()).await?, "production");
    Ok(())
}
