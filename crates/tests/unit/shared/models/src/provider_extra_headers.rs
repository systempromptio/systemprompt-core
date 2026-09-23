//! A provider's `extra_headers` may not set a header the upstream seam sends
//! itself. The request builder appends, so a catalog entry naming the
//! credential, content framing or protocol version would put two values on
//! the wire; the registry refuses it at boot instead.

use std::collections::HashMap;

use systemprompt_identifiers::{ProviderId, SecretName};
use systemprompt_models::services::providers::ProviderRegistryError;
use systemprompt_models::services::{ApiSurface, ProviderEntry, ProviderRegistry, WireProtocol};

fn anthropic_with(headers: &[(&str, &str)]) -> ProviderRegistry {
    ProviderRegistry {
        providers: vec![ProviderEntry {
            name: ProviderId::new("anthropic"),
            display_name: None,
            description: None,
            wire: WireProtocol::Anthropic,
            surface: ApiSurface::Anthropic,
            endpoint: "https://api.anthropic.com/v1".to_owned(),
            api_key_secret: SecretName::new("anthropic"),
            governance: Default::default(),
            extra_headers: headers
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect::<HashMap<_, _>>(),
            accepted_betas: None,
            models: Vec::new(),
        }],
    }
}

#[test]
fn a_reserved_header_is_refused_whatever_its_case() {
    for name in [
        "authorization",
        "X-Api-Key",
        "anthropic-version",
        "Content-Type",
    ] {
        let err = anthropic_with(&[(name, "x")]).validate().unwrap_err();
        match err {
            ProviderRegistryError::ReservedExtraHeader { provider, header } => {
                assert_eq!(provider, "anthropic");
                assert_eq!(header, name);
            },
            other => panic!("expected ReservedExtraHeader for {name}, got {other:?}"),
        }
    }
}

#[test]
fn an_ordinary_extra_header_is_accepted() {
    anthropic_with(&[("anthropic-beta", "context-1m-2025-08-07")])
        .validate()
        .expect("a header the seam does not send is the provider's to set");
}
