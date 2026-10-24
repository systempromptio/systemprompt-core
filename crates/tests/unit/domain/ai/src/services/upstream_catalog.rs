//! An upstream target takes every fact from the catalog object that owns it.
//!
//! Wire, URL and hosting come from the endpoint, so one provider with two
//! endpoints resolves to two wires; headers, user agent and the forwarded
//! header allowlist come from the provider (with `{install}` expanded); the
//! token cache key comes from the credential, so two endpoints of one
//! credential share it and two members' personal keys never do.

use systemprompt_ai::{EndpointBinding, UpstreamTarget, UpstreamTargetError};
use systemprompt_identifiers::UserId;
use systemprompt_manifest::services::catalog::EndpointRef;
use systemprompt_test_fixtures::catalog::{
    OPENCODE_GO_CHAT_URL, OPENCODE_GO_MESSAGES_URL, opencode_go,
};
use systemprompt_wire::{Hosting, WireProtocol};

fn personal(endpoint: &str, user: &str) -> UpstreamTarget {
    let catalog = opencode_go().install("example").build();
    let binding =
        EndpointBinding::of_endpoint(&catalog, &EndpointRef::new("opencode-go", endpoint))
            .expect("endpoint in catalog");
    UpstreamTarget::resolve_personal(binding, "sk-member", &UserId::new(user)).expect("resolves")
}

#[test]
fn each_endpoint_of_one_provider_keeps_its_own_wire_and_url() {
    let chat = personal("chat", "alice");
    let messages = personal("messages", "alice");
    assert_eq!(chat.wire(), WireProtocol::OpenAiChat);
    assert_eq!(chat.endpoint(), OPENCODE_GO_CHAT_URL);
    assert_eq!(messages.wire(), WireProtocol::Anthropic);
    assert_eq!(messages.endpoint(), OPENCODE_GO_MESSAGES_URL);
    assert_eq!(chat.hosting(), Hosting::FirstParty);
    assert_eq!(
        messages.accepted_betas().map(|betas| betas.len()),
        Some(0),
        "the messages endpoint declares an empty beta list; chat declares none"
    );
    assert!(chat.accepted_betas().is_none());
}

#[tokio::test]
async fn provider_headers_and_user_agent_reach_every_endpoint() {
    for endpoint in ["chat", "messages"] {
        let target = personal(endpoint, "alice");
        let call = target.call().await.expect("api key needs no mint");
        let headers = call.headers_forwarding(
            target.wire(),
            &[("x-opencode-session".to_owned(), "ses_1".to_owned())],
        );
        assert!(
            headers.contains(&(
                "user-agent".to_owned(),
                "systemprompt-gateway (opencode-go; example)".to_owned()
            )),
            "{endpoint}: {headers:?}"
        );
        assert!(headers.contains(&("x-opencode-session".to_owned(), "ses_1".to_owned())));
    }
}

#[test]
fn one_credential_keys_one_token_across_endpoints_and_members_apart() {
    let alice_chat = personal("chat", "alice");
    let alice_messages = personal("messages", "alice");
    let bob_chat = personal("chat", "bob");
    assert_eq!(alice_chat.cache_key(), alice_messages.cache_key());
    assert_ne!(alice_chat.cache_key(), bob_chat.cache_key());
    assert_eq!(alice_chat.credential_id().as_str(), "opencode-go");
}

#[test]
fn a_personal_credential_has_no_deployment_secret() {
    let catalog = opencode_go().build();
    let binding = EndpointBinding::of_endpoint(&catalog, &EndpointRef::new("opencode-go", "chat"))
        .expect("endpoint in catalog");
    match UpstreamTarget::from_secrets(binding) {
        Err(UpstreamTargetError::PersonalCredential { credential }) => {
            assert_eq!(credential, "opencode-go");
        },
        other => panic!("expected PersonalCredential, got {other:?}"),
    }
}

#[test]
fn an_unknown_endpoint_is_refused() {
    let catalog = opencode_go().build();
    let err = EndpointBinding::of_endpoint(&catalog, &EndpointRef::new("opencode-go", "nope"))
        .unwrap_err();
    assert!(matches!(err, UpstreamTargetError::UnknownEndpoint { .. }));
}
