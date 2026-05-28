mod entropy;
mod resource;

use super::AuthorizeQuery;
use anyhow::Result;
use systemprompt_oauth::repository::OAuthRepository;
use url::Origin;

/// Origin pair the `resource` self-origin carve-out matches against.
///
/// `primary` is derived from `api_external_url`; `request` is derived from the
/// (allowlisted) Host header so that RFC 9728 dual-self-identity flows — where
/// one gateway answers on both `127.0.0.1` and `localhost` — accept resource
/// URIs constructed from either advertised identity.
#[derive(Debug, Clone)]
pub struct SelfOrigins {
    primary: Origin,
    request: Origin,
}

impl SelfOrigins {
    #[must_use]
    pub const fn new(primary: Origin, request: Origin) -> Self {
        Self { primary, request }
    }

    pub fn matches(&self, other: &Origin) -> bool {
        &self.primary == other || &self.request == other
    }
}

pub async fn validate_authorize_request(
    state: &systemprompt_oauth::OAuthState,
    params: &AuthorizeQuery,
    repo: &OAuthRepository,
) -> Result<String> {
    if params.response_type != "code" {
        return Err(anyhow::anyhow!(
            "Unsupported response_type. Only 'code' is supported"
        ));
    }

    let client = repo
        .find_client_by_id(&params.client_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Invalid client_id"))?;

    if let Some(redirect_uri) = &params.redirect_uri {
        use systemprompt_oauth::services::validation::validate_redirect_uri;

        validate_redirect_uri(&client.redirect_uris, Some(redirect_uri)).map_err(|_e| {
            anyhow::anyhow!(
                "redirect_uri '{}' not registered for client '{}'",
                redirect_uri,
                params.client_id
            )
        })?;
    }

    let resource_scopes = match &params.resource {
        Some(resource) => resource::resolve_resource_scopes(state, resource).await,
        None => None,
    };

    let scope = if let Some(scope_param) = params.scope.as_deref() {
        scope_param.to_owned()
    } else if let Some(ref rs) = resource_scopes {
        rs.clone()
    } else if client.scopes.is_empty() {
        return Err(anyhow::anyhow!(
            "Client has no registered scopes and none provided in request"
        ));
    } else {
        client.scopes.join(" ")
    };

    let requested_scopes = OAuthRepository::parse_scopes(&scope);

    OAuthRepository::validate_scopes(&requested_scopes)
        .map_err(|e| anyhow::anyhow!("Invalid scopes requested: {e}"))?;

    Ok(scope)
}

pub fn validate_oauth_parameters(
    params: &AuthorizeQuery,
    self_origins: &SelfOrigins,
) -> Result<(), String> {
    if params.response_type != "code" {
        return Err(format!(
            "Unsupported response_type '{}'. Only 'code' is supported.",
            params.response_type
        ));
    }

    if let Some(response_mode) = &params.response_mode {
        if response_mode != "query" {
            return Err(format!(
                "Unsupported response_mode '{response_mode}'. Only 'query' mode is supported."
            ));
        }
    }

    validate_pkce(params)?;
    validate_display_and_prompt(params)?;

    if let Some(max_age) = params.max_age {
        if max_age < 0 {
            return Err("max_age must be a non-negative integer".to_owned());
        }
    }

    if let Some(resource) = &params.resource {
        resource::validate_resource_uri(resource, self_origins)?;
    }

    Ok(())
}

fn validate_pkce(params: &AuthorizeQuery) -> Result<(), String> {
    let Some(code_challenge) = &params.code_challenge else {
        return Err("code_challenge is required. PKCE with S256 method must be used.".to_owned());
    };

    if code_challenge.len() < systemprompt_oauth::constants::pkce::CODE_CHALLENGE_MIN_LENGTH {
        return Err(format!(
            "code_challenge too short. Must be at least {} characters for security.",
            systemprompt_oauth::constants::pkce::CODE_CHALLENGE_MIN_LENGTH
        ));
    }
    if code_challenge.len() > systemprompt_oauth::constants::pkce::CODE_CHALLENGE_MAX_LENGTH {
        return Err(format!(
            "code_challenge too long. Must be at most {} characters.",
            systemprompt_oauth::constants::pkce::CODE_CHALLENGE_MAX_LENGTH
        ));
    }

    let is_valid_base64url = code_challenge
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');

    if !is_valid_base64url {
        return Err("code_challenge must be base64url encoded (A-Z, a-z, 0-9, -, _)".to_owned());
    }

    if entropy::is_low_entropy_challenge(code_challenge) {
        return Err("code_challenge appears to have insufficient entropy for security".to_owned());
    }

    let method = params.code_challenge_method.as_deref().ok_or_else(|| {
        "code_challenge_method is required when code_challenge is provided".to_owned()
    })?;

    match method {
        "S256" => Ok(()),
        "plain" => Err("PKCE method 'plain' is not allowed. Use 'S256' for security.".to_owned()),
        _ => Err(format!(
            "Unsupported code_challenge_method '{method}'. Only 'S256' is allowed."
        )),
    }
}

fn validate_display_and_prompt(params: &AuthorizeQuery) -> Result<(), String> {
    if let Some(display) = &params.display {
        match display.as_str() {
            "page" | "popup" | "touch" | "wap" => {},
            _ => {
                return Err(format!(
                    "Unsupported display value '{display}'. Supported values: page, popup, touch, \
                     wap."
                ));
            },
        }
    }

    if let Some(prompt) = &params.prompt {
        for prompt_value in prompt.split_whitespace() {
            match prompt_value {
                "none" | "login" | "consent" | "select_account" => {},
                _ => {
                    return Err(format!(
                        "Unsupported prompt value '{prompt_value}'. Supported values: none, \
                         login, consent, select_account."
                    ));
                },
            }
        }
    }

    Ok(())
}
