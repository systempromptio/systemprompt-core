//! Scope attribution seam: a tenant `project` dimension registered through
//! `register_subject_attribute_provider!` is resolved from the
//! `x-systemprompt-scope-project` header, the API key's binding, or the
//! provider's first value, verified against the caller's values, and
//! persisted on the audit row (`ai_request_attributions`, `api_key_id`).

use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use systemprompt_api::routes::gateway::messages::extract::RejectionPartial;
use systemprompt_api::routes::gateway::messages::extract::scope::{
    ScopeHeaders, ScopeResolution, resolve_scope_attribution,
};
use systemprompt_api::routes::gateway::messages::rejection::persist_rejection;
use systemprompt_gateway::service::GatewayService;
use systemprompt_identifiers::{AiRequestId, ApiKeyId, ScopeDimension, UserId};
use systemprompt_models::attribution::{AttributionSource, RequestAttribution, ScopeBinding};
use systemprompt_models::origin::{
    ClientAttestation, ClientKind, InboundWireProtocol, RequestOrigin,
};
use systemprompt_models::providers::ApiSurface;
use systemprompt_security::authz::{
    AuthzError, RuleType, SubjectAttributeProvider, SubjectDimension, SubjectProviderSet,
};
use systemprompt_test_fixtures::seed_admin_credential;
use systemprompt_wire::WireProtocol;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::common::setup_ctx;
use super::gateway_pipeline::{
    MODEL, PROVIDER, canonical_request, gateway_config, gw_repos, inputs, install_provider_api_key,
    provider_registry,
};

const PROJECT: &str = "project";
const SCOPED_USER_PREFIX: &str = "scope-attr-";
const PROJECT_RULE_TYPE: RuleType = RuleType::extension_static(PROJECT);

#[derive(Debug)]
struct ProjectProvider;

#[async_trait::async_trait]
impl SubjectAttributeProvider for ProjectProvider {
    fn dimension(&self) -> SubjectDimension {
        SubjectDimension {
            rule_type: PROJECT_RULE_TYPE,
            label: "Project",
            precedence: 300,
        }
    }

    async fn values_for(&self, user_id: &UserId) -> Result<Vec<String>, AuthzError> {
        if user_id.as_str().starts_with(SCOPED_USER_PREFIX) {
            return Ok(vec!["p-primary".to_owned(), "p-other".to_owned()]);
        }
        Ok(Vec::new())
    }
}

systemprompt_security::register_subject_attribute_provider!(|_ctx| std::sync::Arc::new(
    ProjectProvider
));

fn partial() -> RejectionPartial {
    RejectionPartial::new(RequestOrigin::gateway(
        ClientKind::Other,
        InboundWireProtocol::AnthropicMessages,
        ClientAttestation::None,
    ))
}

fn scoped_user() -> UserId {
    UserId::new(format!("{SCOPED_USER_PREFIX}{}", uuid::Uuid::new_v4()))
}

fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (name, value) in pairs {
        map.insert(
            HeaderName::from_bytes(name.as_bytes()).expect("header name"),
            HeaderValue::from_str(value).expect("header value"),
        );
    }
    map
}

fn project_only() -> SubjectProviderSet {
    SubjectProviderSet::from_providers(vec![std::sync::Arc::new(ProjectProvider)])
}

fn binding(value: &str) -> ScopeBinding {
    ScopeBinding {
        dimension: ScopeDimension::try_new(PROJECT).expect("dimension"),
        value: value.to_owned(),
    }
}

struct Case<'a> {
    providers: &'a SubjectProviderSet,
    user: &'a UserId,
    header: &'a [(&'a str, &'a str)],
    bindings: &'a [ScopeBinding],
    require: &'a [ScopeDimension],
}

async fn resolve(
    case: Case<'_>,
    partial: &mut RejectionPartial,
) -> Result<RequestAttribution, systemprompt_api::routes::gateway::messages::error::RejectionError>
{
    let scope_headers = ScopeHeaders::capture(&headers(case.header))?;
    let api_key_id = ApiKeyId::new("key-attribution");
    resolve_scope_attribution(
        ScopeResolution {
            providers: case.providers,
            user_id: case.user,
            headers: &scope_headers,
            key_bindings: case.bindings,
            api_key_id: Some(&api_key_id),
            require: case.require,
        },
        partial,
    )
    .await
}

fn project_entry(attribution: &RequestAttribution) -> (&str, AttributionSource) {
    let entry = attribution
        .entries
        .iter()
        .find(|e| e.dimension.as_str() == PROJECT)
        .expect("project attributed");
    (entry.value.as_str(), entry.source)
}

#[tokio::test]
async fn registered_provider_is_discovered_through_the_gateway_repositories() {
    let (pool, _ctx) = setup_ctx().await.expect("ctx");
    let repos = gw_repos(&pool);
    assert!(repos.subject_providers.find(PROJECT).is_some());
    let user = scoped_user();
    let mut partial = partial();
    let attribution = resolve(
        Case {
            providers: &repos.subject_providers,
            user: &user,
            header: &[],
            bindings: &[],
            require: &[],
        },
        &mut partial,
    )
    .await
    .expect("resolves");
    assert_eq!(
        project_entry(&attribution),
        ("p-primary", AttributionSource::Default)
    );
}

#[tokio::test]
async fn no_header_attributes_the_providers_first_value_as_default() {
    let providers = project_only();
    let user = scoped_user();
    let mut partial = partial();
    let attribution = resolve(
        Case {
            providers: &providers,
            user: &user,
            header: &[],
            bindings: &[],
            require: &[],
        },
        &mut partial,
    )
    .await
    .expect("resolves");
    assert_eq!(
        project_entry(&attribution),
        ("p-primary", AttributionSource::Default)
    );
    assert_eq!(
        attribution.api_key_id,
        Some(ApiKeyId::new("key-attribution"))
    );
}

#[tokio::test]
async fn a_header_value_the_caller_holds_is_attributed_as_header() {
    let providers = project_only();
    let user = scoped_user();
    let mut partial = partial();
    let attribution = resolve(
        Case {
            providers: &providers,
            user: &user,
            header: &[("x-systemprompt-scope-project", "p-other")],
            bindings: &[],
            require: &[],
        },
        &mut partial,
    )
    .await
    .expect("resolves");
    assert_eq!(
        project_entry(&attribution),
        ("p-other", AttributionSource::Header)
    );
}

#[tokio::test]
async fn a_header_value_the_caller_does_not_hold_is_forbidden_and_recorded() {
    let providers = project_only();
    let user = scoped_user();
    let mut partial = partial();
    let error = resolve(
        Case {
            providers: &providers,
            user: &user,
            header: &[("x-systemprompt-scope-project", "p-nope")],
            bindings: &[],
            require: &[],
        },
        &mut partial,
    )
    .await
    .expect_err("non-member is refused");
    assert_eq!(error.status, StatusCode::FORBIDDEN);
    assert_eq!(error.message, "not a member of project 'p-nope'");
    assert_eq!(
        project_entry(&partial.attribution),
        ("p-nope", AttributionSource::Header)
    );
}

#[tokio::test]
async fn a_header_for_an_unregistered_dimension_is_a_bad_request() {
    let providers = project_only();
    let user = scoped_user();
    let mut partial = partial();
    let error = resolve(
        Case {
            providers: &providers,
            user: &user,
            header: &[("x-systemprompt-scope-region", "eu")],
            bindings: &[],
            require: &[],
        },
        &mut partial,
    )
    .await
    .expect_err("unknown dimension is refused");
    assert_eq!(error.status, StatusCode::BAD_REQUEST);
    assert!(
        error.message.contains("unknown scope dimension 'region'"),
        "{}",
        error.message
    );
}

#[tokio::test]
async fn a_malformed_scope_header_name_is_a_bad_request() {
    let error = ScopeHeaders::capture(&headers(&[("x-systemprompt-scope-9bad", "x")]))
        .expect_err("malformed dimension");
    assert_eq!(error.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_required_dimension_no_provider_registers_is_a_bad_request() {
    let providers = project_only();
    let user = scoped_user();
    let mut partial = partial();
    let require = [ScopeDimension::try_new("cost_centre").expect("dimension")];
    let error = resolve(
        Case {
            providers: &providers,
            user: &user,
            header: &[],
            bindings: &[],
            require: &require,
        },
        &mut partial,
    )
    .await
    .expect_err("required scope missing");
    assert_eq!(error.status, StatusCode::BAD_REQUEST);
    assert!(
        error.message.starts_with("scope_required"),
        "{}",
        error.message
    );
    assert!(error.message.contains("cost_centre"), "{}", error.message);
}

#[tokio::test]
async fn a_key_binding_is_attributed_as_api_key() {
    let providers = project_only();
    let user = scoped_user();
    let mut partial = partial();
    let bindings = [binding("p-other")];
    let attribution = resolve(
        Case {
            providers: &providers,
            user: &user,
            header: &[],
            bindings: &bindings,
            require: &[],
        },
        &mut partial,
    )
    .await
    .expect("resolves");
    assert_eq!(
        project_entry(&attribution),
        ("p-other", AttributionSource::ApiKey)
    );
}

#[tokio::test]
async fn a_header_beats_the_key_binding() {
    let providers = project_only();
    let user = scoped_user();
    let mut partial = partial();
    let bindings = [binding("p-other")];
    let attribution = resolve(
        Case {
            providers: &providers,
            user: &user,
            header: &[("x-systemprompt-scope-project", "p-primary")],
            bindings: &bindings,
            require: &[],
        },
        &mut partial,
    )
    .await
    .expect("resolves");
    assert_eq!(
        project_entry(&attribution),
        ("p-primary", AttributionSource::Header)
    );
}

#[tokio::test]
async fn a_key_bound_to_a_value_the_user_no_longer_holds_is_forbidden() {
    let providers = project_only();
    let user = scoped_user();
    let mut partial = partial();
    let bindings = [binding("p-retired")];
    let error = resolve(
        Case {
            providers: &providers,
            user: &user,
            header: &[],
            bindings: &bindings,
            require: &[],
        },
        &mut partial,
    )
    .await
    .expect_err("stale binding is refused");
    assert_eq!(error.status, StatusCode::FORBIDDEN);
}

async fn attribution_rows(
    pool: &systemprompt_database::DbPool,
    id: &AiRequestId,
) -> Vec<(String, String, String)> {
    sqlx::query_as(
        "SELECT dimension, value, source FROM ai_request_attributions \
         WHERE request_id = $1 ORDER BY dimension",
    )
    .bind(id.as_str())
    .fetch_all(pool.pool().as_ref())
    .await
    .expect("attribution rows")
}

#[tokio::test]
async fn a_rejected_request_persists_its_attribution() -> anyhow::Result<()> {
    let (pool, _ctx) = setup_ctx().await?;
    let cred = seed_admin_credential(&pool, "scope-reject@example.invalid").await?;
    let repos = gw_repos(&pool);
    let id = AiRequestId::generate();
    let mut partial = partial();
    partial.user_id = Some(cred.user_id.clone());
    partial.session_id = Some(cred.session_id.clone());
    partial.attribution = RequestAttribution {
        entries: vec![systemprompt_models::attribution::AttributionEntry {
            dimension: ScopeDimension::try_new(PROJECT)?,
            value: "p-nope".to_owned(),
            source: AttributionSource::Header,
        }],
        api_key_id: Some(ApiKeyId::new("key-rejected")),
    };
    persist_rejection(&repos, &id, &partial, StatusCode::FORBIDDEN, "not a member").await;
    assert_eq!(
        attribution_rows(&pool, &id).await,
        vec![(
            "project".to_owned(),
            "p-nope".to_owned(),
            "header".to_owned()
        )]
    );
    let (api_key_id,): (Option<String>,) =
        sqlx::query_as("SELECT api_key_id FROM ai_requests WHERE id = $1")
            .bind(id.as_str())
            .fetch_one(pool.pool().as_ref())
            .await?;
    assert_eq!(api_key_id.as_deref(), Some("key-rejected"));
    Ok(())
}

#[tokio::test]
async fn a_completed_request_persists_its_attribution_and_api_key() -> anyhow::Result<()> {
    install_provider_api_key();
    let (pool, _ctx) = setup_ctx().await?;
    let cred = seed_admin_credential(&pool, "scope-complete@example.invalid").await?;
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "msg_scope", "type": "message", "role": "assistant", "model": MODEL,
            "content": [{"type": "text", "text": "ok"}], "stop_reason": "end_turn",
            "usage": {"input_tokens": 3, "output_tokens": 2}
        })))
        .mount(&upstream)
        .await;
    let config = gateway_config(PROVIDER);
    let registry = provider_registry(
        &upstream.uri(),
        PROVIDER,
        WireProtocol::Anthropic,
        ApiSurface::Anthropic,
    );
    let mut dispatch = inputs(&cred, canonical_request(MODEL, false), false);
    dispatch.ctx.attribution = RequestAttribution {
        entries: vec![systemprompt_models::attribution::AttributionEntry {
            dimension: ScopeDimension::try_new(PROJECT)?,
            value: "p-primary".to_owned(),
            source: AttributionSource::Default,
        }],
        api_key_id: Some(ApiKeyId::new("key-completed")),
    };
    let id = dispatch.ctx.ai_request_id.clone();
    let repos = gw_repos(&pool);
    let response = GatewayService::dispatch(&config, &registry, &pool, &repos, dispatch)
        .await
        .expect("dispatch succeeds");
    assert_eq!(response.status(), http::StatusCode::OK);
    assert_eq!(
        repos
            .background
            .drain(std::time::Duration::from_secs(30))
            .await,
        systemprompt_traits::DrainOutcome::Drained
    );
    assert_eq!(
        attribution_rows(&pool, &id).await,
        vec![(
            "project".to_owned(),
            "p-primary".to_owned(),
            "default".to_owned()
        )]
    );
    let (api_key_id,): (Option<String>,) =
        sqlx::query_as("SELECT api_key_id FROM ai_requests WHERE id = $1")
            .bind(id.as_str())
            .fetch_one(pool.pool().as_ref())
            .await?;
    assert_eq!(api_key_id.as_deref(), Some("key-completed"));
    let fetched = repos
        .requests
        .attributions_for(std::slice::from_ref(&id))
        .await?;
    assert_eq!(fetched.get(&id).map(Vec::len), Some(1));
    Ok(())
}
