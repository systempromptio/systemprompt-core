use systemprompt_models::{AuthError, GrantType, ResponseType};

#[derive(Debug)]
pub struct CsrfToken(String);

impl CsrfToken {
    const MIN_STATE_LENGTH: usize = 16;

    pub fn new(state: impl Into<String>) -> Result<Self, AuthError> {
        let state = state.into();

        if state.is_empty() {
            return Err(AuthError::MissingState);
        }

        if state.len() < Self::MIN_STATE_LENGTH {
            return Err(AuthError::InvalidRequest {
                reason: format!(
                    "State must be at least {} characters for security",
                    Self::MIN_STATE_LENGTH
                ),
            });
        }

        if !state.chars().all(|c| c.is_ascii_graphic()) {
            return Err(AuthError::InvalidRequest {
                reason: "State must contain only printable ASCII characters".to_string(),
            });
        }

        Ok(Self(state))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

#[derive(Debug)]
pub struct ValidatedClientRegistration {
    pub client_name: String,
    pub redirect_uris: Vec<String>,
    pub grant_types: Vec<GrantType>,
    pub response_types: Vec<ResponseType>,
}

pub fn required_param(value: Option<&str>, param_name: &str) -> Result<String, AuthError> {
    value
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AuthError::InvalidRequest {
            reason: format!("{param_name} parameter is required"),
        })
        .map(ToString::to_string)
}

pub fn optional_param(value: Option<&str>) -> Option<String> {
    value.filter(|s| !s.is_empty()).map(ToString::to_string)
}

pub fn scope_param(value: Option<&str>) -> Result<Vec<String>, AuthError> {
    let scope_str = required_param(value, "scope")?;

    let scopes: Vec<String> = scope_str
        .split_whitespace()
        .map(ToString::to_string)
        .collect();

    if scopes.is_empty() {
        return Err(AuthError::InvalidScope { scope: scope_str });
    }

    Ok(scopes)
}

pub fn get_audit_user(user_id: Option<&str>) -> Result<String, AuthError> {
    user_id
        .filter(|id| !id.is_empty())
        .ok_or_else(|| AuthError::InvalidRequest {
            reason: "Authenticated user required for this operation".to_string(),
        })
        .map(ToString::to_string)
}
