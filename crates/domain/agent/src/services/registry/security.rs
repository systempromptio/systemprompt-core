use std::collections::HashMap;
use systemprompt_models::AgentOAuthConfig;

use crate::models::a2a::{OAuth2Flow, OAuth2Flows, SecurityScheme};

pub fn convert_json_security_to_struct(
    security_schemes: Option<&serde_json::Value>,
    security: Option<&Vec<serde_json::Value>>,
) -> (
    Option<HashMap<String, SecurityScheme>>,
    Option<Vec<HashMap<String, Vec<String>>>>,
) {
    let schemes = security_schemes.and_then(|schemes_json| {
        serde_json::from_value::<HashMap<String, SecurityScheme>>(schemes_json.clone()).ok()
    });

    let security_reqs = security.and_then(|sec_vec| {
        let reqs: Result<Vec<HashMap<String, Vec<String>>>, _> = sec_vec
            .iter()
            .map(|v| serde_json::from_value::<HashMap<String, Vec<String>>>(v.clone()))
            .collect();
        reqs.ok()
    });

    (schemes, security_reqs)
}

pub fn oauth_to_security_config(
    oauth: &AgentOAuthConfig,
    api_external_url: &str,
) -> (
    Option<HashMap<String, SecurityScheme>>,
    Option<Vec<HashMap<String, Vec<String>>>>,
) {
    if !oauth.required {
        return (None, None);
    }

    let flows = OAuth2Flows {
        authorization_code: Some(OAuth2Flow {
            authorization_url: Some(format!("{}/api/v1/core/oauth/authorize", api_external_url)),
            token_url: Some(format!("{}/api/v1/core/oauth/token", api_external_url)),
            refresh_url: Some(format!("{}/api/v1/core/oauth/token", api_external_url)),
            scopes: oauth
                .scopes
                .iter()
                .map(|s| (s.to_string(), format!("{} access", s)))
                .collect(),
        }),
        implicit: None,
        password: None,
        client_credentials: None,
    };

    let scheme = SecurityScheme::OAuth2 {
        flows: Box::new(flows),
        description: Some(format!(
            "OAuth 2.0 authentication for {} audience",
            oauth.audience
        )),
    };

    let mut schemes = HashMap::new();
    schemes.insert("oauth2".to_string(), scheme);

    let mut requirement = HashMap::new();
    requirement.insert(
        "oauth2".to_string(),
        oauth.scopes.iter().map(|s| s.to_string()).collect(),
    );
    let requirements = vec![requirement];

    (Some(schemes), Some(requirements))
}

pub fn override_oauth_urls(schemes: &mut HashMap<String, SecurityScheme>, api_external_url: &str) {
    if let Some(SecurityScheme::OAuth2 { flows, .. }) = schemes.get_mut("oauth2") {
        if let Some(auth_code) = flows.authorization_code.as_mut() {
            auth_code.authorization_url = auth_code.authorization_url.as_ref().map(|url| {
                if url.starts_with('/') {
                    format!("{api_external_url}{url}")
                } else {
                    url.clone()
                }
            });

            auth_code.token_url = auth_code.token_url.as_ref().map(|url| {
                if url.starts_with('/') {
                    format!("{api_external_url}{url}")
                } else {
                    url.clone()
                }
            });

            auth_code.refresh_url = auth_code.refresh_url.as_ref().map(|url| {
                if url.starts_with('/') {
                    format!("{api_external_url}{url}")
                } else {
                    url.clone()
                }
            });
        }
    }
}
