//! The upstream an outbound adapter is handed, built the way
//! `UpstreamTarget::call` builds one for an API-key credential: the hosting is
//! read off the endpoint and the key is sent verbatim.

use systemprompt_ai::UpstreamCall;
use systemprompt_models::services::Hosting;
use systemprompt_security::credential::{AuthHeader, AuthScheme};

pub(super) fn api_key_call(endpoint: &str, key: &str) -> UpstreamCall {
    UpstreamCall::new(
        Hosting::of(endpoint),
        endpoint.to_owned(),
        AuthHeader {
            scheme: AuthScheme::ApiKey,
            value: key.to_owned(),
        },
        Vec::new(),
    )
}
