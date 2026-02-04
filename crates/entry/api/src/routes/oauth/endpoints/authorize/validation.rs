use super::AuthorizeQuery;
use anyhow::Result;
use systemprompt_oauth::repository::OAuthRepository;

pub async fn validate_authorize_request(
    params: &AuthorizeQuery,
    repo: &OAuthRepository,
) -> Result<String> {
    if params.response_type != "code" {
        return Err(anyhow::anyhow!(
            "Unsupported response_type. Only 'code' is supported"
        ));
    }

    let client = repo
        .find_client_by_id(params.client_id.as_str())
        .await?
        .ok_or_else(|| anyhow::anyhow!("Invalid client_id"))?;

    if let Some(redirect_uri) = &params.redirect_uri {
        let is_valid = client.redirect_uris.contains(redirect_uri);

        if !is_valid {
            return Err(anyhow::anyhow!(
                "redirect_uri '{}' not registered for client '{}'",
                redirect_uri,
                params.client_id
            ));
        }
    }

    let scope = if let Some(scope_param) = params.scope.as_deref() {
        scope_param.to_string()
    } else {
        if client.scopes.is_empty() {
            return Err(anyhow::anyhow!(
                "Client has no registered scopes and none provided in request"
            ));
        }
        client.scopes.join(" ")
    };

    let requested_scopes = OAuthRepository::parse_scopes(&scope);

    let valid_scopes = OAuthRepository::validate_scopes(&requested_scopes)
        .map_err(|e| anyhow::anyhow!("Invalid scopes requested: {e}"))?;

    for requested_scope in &valid_scopes {
        if !client.scopes.contains(requested_scope) {
            return Err(anyhow::anyhow!(
                "Scope '{}' not allowed for client '{}'",
                requested_scope,
                params.client_id
            ));
        }
    }

    Ok(scope)
}

pub fn validate_oauth_parameters(params: &AuthorizeQuery) -> Result<(), String> {
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

    if let Some(code_challenge) = &params.code_challenge {
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
            return Err(
                "code_challenge must be base64url encoded (A-Z, a-z, 0-9, -, _)".to_string(),
            );
        }

        if is_low_entropy_challenge(code_challenge) {
            return Err(
                "code_challenge appears to have insufficient entropy for security".to_string(),
            );
        }

        let method = params.code_challenge_method.as_deref().ok_or_else(|| {
            "code_challenge_method is required when code_challenge is provided".to_string()
        })?;

        match method {
            "S256" => {},
            "plain" => {
                return Err(
                    "PKCE method 'plain' is not allowed. Use 'S256' for security.".to_string(),
                );
            },
            _ => {
                return Err(format!(
                    "Unsupported code_challenge_method '{method}'. Only 'S256' is allowed."
                ));
            },
        }
    }

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

    if let Some(max_age) = params.max_age {
        if max_age < 0 {
            return Err("max_age must be a non-negative integer".to_string());
        }
    }

    if let Some(resource) = &params.resource {
        validate_resource_uri(resource)?;
    }

    Ok(())
}

fn validate_resource_uri(resource: &str) -> Result<(), String> {
    let url = reqwest::Url::parse(resource)
        .map_err(|_| format!("Invalid resource URI: '{resource}' is not a valid absolute URI"))?;

    if url.scheme() != "https" && url.scheme() != "http" {
        return Err(format!(
            "Resource URI must use https or http scheme, got '{}'",
            url.scheme()
        ));
    }

    if url.fragment().is_some() {
        return Err("Resource URI must not contain a fragment".to_string());
    }

    Ok(())
}

fn is_low_entropy_challenge(challenge: &str) -> bool {
    let Some(first_char) = challenge.chars().next() else {
        return true;
    };

    if challenge.chars().all(|c| c == first_char) {
        return true;
    }

    if has_repeating_pattern(challenge) {
        return true;
    }

    if has_sequential_run(challenge) {
        return true;
    }

    if has_low_diversity(challenge) {
        return true;
    }

    false
}

fn has_repeating_pattern(challenge: &str) -> bool {
    for pattern_length in 2..=4 {
        if challenge.len() >= pattern_length * 3 {
            let pattern = &challenge[..pattern_length];
            let repetitions = challenge.len() / pattern_length;
            if repetitions >= 3 {
                let repeated = pattern.repeat(repetitions);
                if challenge.starts_with(&repeated) {
                    return true;
                }
            }
        }
    }
    false
}

fn has_sequential_run(challenge: &str) -> bool {
    use systemprompt_oauth::constants::validation::MIN_SEQUENTIAL_RUN;

    let chars: Vec<char> = challenge.chars().collect();
    if chars.len() < MIN_SEQUENTIAL_RUN {
        return false;
    }

    let mut ascending_count = 1;
    let mut descending_count = 1;

    for i in 1..chars.len() {
        if let (Some(prev), Some(curr)) = (chars[i - 1].to_digit(36), chars[i].to_digit(36)) {
            if curr == prev.wrapping_add(1) {
                ascending_count += 1;
                if ascending_count >= MIN_SEQUENTIAL_RUN {
                    return true;
                }
            } else {
                ascending_count = 1;
            }

            if prev == curr.wrapping_add(1) {
                descending_count += 1;
                if descending_count >= MIN_SEQUENTIAL_RUN {
                    return true;
                }
            } else {
                descending_count = 1;
            }
        }
    }
    false
}

fn has_low_diversity(challenge: &str) -> bool {
    use systemprompt_oauth::constants::validation::{DIVERSITY_THRESHOLD, MIN_UNIQUE_CHARS};

    let unique_chars: std::collections::HashSet<char> = challenge.chars().collect();
    let entropy_ratio = unique_chars.len() as f64 / challenge.len() as f64;

    if entropy_ratio < DIVERSITY_THRESHOLD {
        return true;
    }

    let min_unique_for_length = challenge.len() / 2;
    unique_chars.len() < min_unique_for_length.min(MIN_UNIQUE_CHARS)
}
