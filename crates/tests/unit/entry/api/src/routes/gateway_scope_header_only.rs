//! Header-only scope dimensions: a provider whose `default_attribution` is
//! `false` attributes nothing to a request that names no value for it, and
//! still verifies a header value against what the caller holds.

use std::sync::Arc;

use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use systemprompt_api::routes::gateway::messages::extract::RejectionPartial;
use systemprompt_api::routes::gateway::messages::extract::scope::{
    ScopeHeaders, ScopeResolution, resolve_scope_attribution,
};
use systemprompt_identifiers::UserId;
use systemprompt_models::attribution::{AttributionSource, RequestAttribution};
use systemprompt_models::origin::{
    ClientAttestation, ClientKind, InboundWireProtocol, RequestOrigin,
};
use systemprompt_security::authz::{
    AuthzError, RuleType, SubjectAttributeProvider, SubjectDimension, SubjectProviderSet,
};

const RUN: &str = "workflowrun";

#[derive(Debug)]
struct HeaderOnlyProvider;

#[async_trait::async_trait]
impl SubjectAttributeProvider for HeaderOnlyProvider {
    fn dimension(&self) -> SubjectDimension {
        SubjectDimension {
            rule_type: RuleType::extension_static(RUN),
            label: "Workflow run",
            precedence: 310,
        }
    }

    async fn values_for(&self, _user_id: &UserId) -> Result<Vec<String>, AuthzError> {
        Ok(vec!["run-open".to_owned()])
    }

    fn default_attribution(&self) -> bool {
        false
    }
}

async fn resolve(
    header: Option<&str>,
) -> Result<RequestAttribution, systemprompt_api::routes::gateway::messages::error::RejectionError>
{
    let providers = SubjectProviderSet::from_providers(vec![Arc::new(HeaderOnlyProvider)]);
    let mut map = HeaderMap::new();
    if let Some(value) = header {
        map.insert(
            HeaderName::from_bytes(format!("x-systemprompt-scope-{RUN}").as_bytes())
                .expect("header name"),
            HeaderValue::from_str(value).expect("header value"),
        );
    }
    let headers = ScopeHeaders::capture(&map)?;
    let user = UserId::new("header-only-user");
    let mut partial = RejectionPartial::new(RequestOrigin::gateway(
        ClientKind::Other,
        InboundWireProtocol::AnthropicMessages,
        ClientAttestation::None,
    ));
    resolve_scope_attribution(
        ScopeResolution {
            providers: &providers,
            user_id: &user,
            headers: &headers,
            key_bindings: &[],
            api_key_id: None,
            require: &[],
        },
        &mut partial,
    )
    .await
}

#[tokio::test]
async fn no_header_attributes_nothing_for_a_header_only_dimension() {
    let attribution = resolve(None).await.expect("resolves");
    assert!(attribution.value_for(RUN).is_none());
}

#[tokio::test]
async fn a_header_on_a_header_only_dimension_is_attributed_and_verified() {
    let attribution = resolve(Some("run-open")).await.expect("resolves");
    let entry = attribution
        .entries
        .iter()
        .find(|e| e.dimension.as_str() == RUN)
        .expect("attributed");
    assert_eq!(entry.value, "run-open");
    assert_eq!(entry.source, AttributionSource::Header);

    let Err(rejection) = resolve(Some("run-not-held")).await else {
        panic!("a value the caller does not hold must be rejected");
    };
    assert_eq!(rejection.status, StatusCode::FORBIDDEN);
}
