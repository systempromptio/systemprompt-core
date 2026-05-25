use axum::http::HeaderMap;

use crate::routes::oauth::OAuthHttpError;

pub fn validate_registration_token(headers: &HeaderMap) -> Result<String, OAuthHttpError> {
    let auth_header = headers
        .get("authorization")
        .ok_or_else(|| OAuthHttpError::invalid_token("Missing authorization header"))?
        .to_str()
        .map_err(|_e| OAuthHttpError::invalid_token("Invalid authorization header format"))?;

    let token = auth_header.strip_prefix("Bearer ").ok_or_else(|| {
        OAuthHttpError::invalid_token("Authorization header must use Bearer scheme")
    })?;

    if !token.starts_with("reg_") {
        return Err(OAuthHttpError::invalid_token(
            "Invalid registration access token format",
        ));
    }

    Ok(token.to_owned())
}
