//! Translation of agent OAuth configuration into A2A agent-card security
//! schemes and requirements, including resolving relative OAuth endpoint URLs
//! against the API's external base URL.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::hash::BuildHasher;
use systemprompt_manifest::AgentOAuthConfig;

use crate::models::a2a::{
    AuthorizationCodeOAuthFlow, OAuth2SecurityScheme, OAuthFlows, SecurityRequirement,
    SecurityScheme,
};

type SecurityConfig = (
    Option<HashMap<String, SecurityScheme>>,
    Option<Vec<SecurityRequirement>>,
);

pub fn oauth_to_security_config(
    oauth: &AgentOAuthConfig,
    api_external_url: &str,
) -> SecurityConfig {
    if !oauth.required {
        return (None, None);
    }

    let flows = OAuthFlows::AuthorizationCode(AuthorizationCodeOAuthFlow {
        authorization_url: format!("{api_external_url}/api/v1/core/oauth/authorize"),
        token_url: format!("{api_external_url}/api/v1/core/oauth/token"),
        refresh_url: Some(format!("{api_external_url}/api/v1/core/oauth/token")),
        scopes: oauth
            .scopes
            .iter()
            .map(|s| (s.to_string(), format!("{s} access")))
            .collect(),
        pkce_required: false,
    });

    let scheme = SecurityScheme::OAuth2(OAuth2SecurityScheme {
        description: Some(format!(
            "OAuth 2.0 authentication for {} audience",
            oauth.audience
        )),
        flows,
        oauth2_metadata_url: None,
    });

    let schemes = HashMap::from([("oauth2".to_owned(), scheme)]);
    let requirements = vec![SecurityRequirement::scheme(
        "oauth2",
        oauth.scopes.iter().map(ToString::to_string).collect(),
    )];

    (Some(schemes), Some(requirements))
}

#[must_use]
pub fn requirements_from_config<S: BuildHasher>(
    requirements: &[HashMap<String, Vec<String>, S>],
) -> Vec<SecurityRequirement> {
    requirements
        .iter()
        .map(|requirement| {
            let mut out = SecurityRequirement::default();
            for (name, scopes) in requirement {
                out.schemes
                    .extend(SecurityRequirement::scheme(name.clone(), scopes.clone()).schemes);
            }
            out
        })
        .collect()
}

fn absolutize(url: &mut String, api_external_url: &str) {
    if url.starts_with('/') {
        *url = format!("{api_external_url}{url}");
    }
}

pub fn override_oauth_urls<S: BuildHasher>(
    schemes: &mut HashMap<String, SecurityScheme, S>,
    api_external_url: &str,
) {
    let Some(SecurityScheme::OAuth2(oauth2)) = schemes.get_mut("oauth2") else {
        return;
    };
    match &mut oauth2.flows {
        OAuthFlows::AuthorizationCode(flow) => {
            absolutize(&mut flow.authorization_url, api_external_url);
            absolutize(&mut flow.token_url, api_external_url);
            if let Some(refresh) = flow.refresh_url.as_mut() {
                absolutize(refresh, api_external_url);
            }
        },
        OAuthFlows::ClientCredentials(flow) => {
            absolutize(&mut flow.token_url, api_external_url);
            if let Some(refresh) = flow.refresh_url.as_mut() {
                absolutize(refresh, api_external_url);
            }
        },
        OAuthFlows::DeviceCode(flow) => {
            absolutize(&mut flow.device_authorization_url, api_external_url);
            absolutize(&mut flow.token_url, api_external_url);
            if let Some(refresh) = flow.refresh_url.as_mut() {
                absolutize(refresh, api_external_url);
            }
        },
    }
}
