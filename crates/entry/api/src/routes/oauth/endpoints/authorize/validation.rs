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
        use systemprompt_oauth::services::validation::validate_redirect_uri;

        validate_redirect_uri(&client.redirect_uris, Some(redirect_uri)).map_err(|_| {
            anyhow::anyhow!(
                "redirect_uri '{}' not registered for client '{}'",
                redirect_uri,
                params.client_id
            )
        })?;
    }

    let resource_scopes = match &params.resource {
        Some(resource) => resolve_resource_scopes(resource).await,
        None => None,
    };

    let scope = if let Some(scope_param) = params.scope.as_deref() {
        scope_param.to_string()
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

    let Some(code_challenge) = &params.code_challenge else {
        return Err("code_challenge is required. PKCE with S256 method must be used.".to_string());
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
        return Err("code_challenge must be base64url encoded (A-Z, a-z, 0-9, -, _)".to_string());
    }

    if is_low_entropy_challenge(code_challenge) {
        return Err("code_challenge appears to have insufficient entropy for security".to_string());
    }

    let method = params.code_challenge_method.as_deref().ok_or_else(|| {
        "code_challenge_method is required when code_challenge is provided".to_string()
    })?;

    match method {
        "S256" => {},
        "plain" => {
            return Err("PKCE method 'plain' is not allowed. Use 'S256' for security.".to_string());
        },
        _ => {
            return Err(format!(
                "Unsupported code_challenge_method '{method}'. Only 'S256' is allowed."
            ));
        },
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

    if let Some(host) = url.host_str() {
        if is_forbidden_host(host) {
            return Err(
                "Resource URI must not target internal or private network addresses".to_string(),
            );
        }
    }

    Ok(())
}

fn is_forbidden_host(host: &str) -> bool {
    let lower = host.to_lowercase();

    if lower == "localhost" || lower == "127.0.0.1" || lower == "::1" || lower == "0.0.0.0" {
        return true;
    }

    if lower.ends_with(".internal")
        || std::path::Path::new(&lower)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("local"))
    {
        return true;
    }

    if lower.starts_with("10.") || lower.starts_with("192.168.") || lower.starts_with("169.254.") {
        return true;
    }

    if lower.starts_with("172.") {
        if let Some(second_octet_str) = lower
            .strip_prefix("172.")
            .and_then(|rest| rest.split('.').next())
        {
            if let Ok(second_octet) = second_octet_str.parse::<u8>() {
                if (16..=31).contains(&second_octet) {
                    return true;
                }
            }
        }
    }

    false
}

fn shannon_entropy(data: &str) -> f64 {
    let len = data.len() as f64;
    if len == 0.0 {
        return 0.0;
    }

    let mut freq = std::collections::HashMap::new();
    for c in data.chars() {
        *freq.entry(c).or_insert(0u64) += 1;
    }

    freq.values().fold(0.0, |entropy, &count| {
        let p = count as f64 / len;
        p.mul_add(-p.log2(), entropy)
    })
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

    if shannon_entropy(challenge) < 3.0 {
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

async fn resolve_resource_scopes(resource: &str) -> Option<String> {
    crate::routes::proxy::mcp::get_mcp_server_scopes_from_resource(resource)
        .await
        .map(|scopes| scopes.join(" "))
}
