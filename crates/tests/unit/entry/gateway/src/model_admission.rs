//! The per-caller model admission the `admitted-models` conferral answers
//! with: a model is listed only when the instance's authz hook allows it, so a
//! hook that cannot decide (the deny-all default, whose reason is
//! `HookUnavailable`) refuses every candidate rather than returning them.

use std::sync::Arc;

use systemprompt_catalog_fixtures::CatalogFixture;
use systemprompt_gateway_entry::http::bridge::admission::{AdmissionPrincipal, ModelAdmission};
use systemprompt_governance::authz::{AllowAllHook, DenyAllHook, SharedAuthzHook};
use systemprompt_identifiers::{ModelId, SessionId, UserId};

fn principal() -> AdmissionPrincipal {
    AdmissionPrincipal {
        user_id: UserId::new("admission-user"),
        roles: vec!["user".to_owned()],
        client_id: None,
        session_id: SessionId::new("sess_admission"),
    }
}

fn candidates(fixture: &CatalogFixture) -> Vec<ModelId> {
    fixture
        .document()
        .models
        .iter()
        .map(|(id, _)| id.clone())
        .collect()
}

async fn admitted(hook: SharedAuthzHook) -> (Vec<ModelId>, Vec<ModelId>) {
    let fixture = CatalogFixture::embedded_default();
    let catalog = fixture.build();
    let principal = principal();
    let offered = candidates(&fixture);
    let admission = ModelAdmission {
        catalog: &catalog,
        hook: &hook,
        principal: &principal,
    };
    (offered.clone(), admission.retain(offered).await)
}

#[tokio::test]
async fn an_undecidable_hook_refuses_every_candidate() {
    let (offered, admitted) = admitted(Arc::new(DenyAllHook::null())).await;
    assert!(!offered.is_empty(), "the fixture catalog offers models");
    assert!(
        admitted.is_empty(),
        "a hook that cannot decide must refuse, got {admitted:?}"
    );
}

#[tokio::test]
async fn the_answer_is_a_subset_of_the_candidates() {
    let (offered, admitted) = admitted(Arc::new(AllowAllHook::null())).await;
    assert!(
        admitted.iter().all(|model| offered.contains(model)),
        "admission only removes models: {admitted:?} not within {offered:?}"
    );
}
