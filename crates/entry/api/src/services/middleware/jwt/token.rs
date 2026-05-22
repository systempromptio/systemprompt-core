use anyhow::{Result, anyhow};
use jsonwebtoken::{Algorithm, Validation, decode, decode_header};

use systemprompt_identifiers::{Actor, ClientId, SessionId, UserId};
use systemprompt_models::auth::UserType;
use systemprompt_oauth::models::JwtClaims;
use systemprompt_security::keys::authority;

#[derive(Debug, Clone)]
pub struct JwtUserContext {
    pub user_id: UserId,
    pub session_id: SessionId,
    pub role: systemprompt_models::auth::Permission,
    pub user_type: UserType,
    pub client_id: Option<ClientId>,
    pub roles: Vec<String>,
    pub department: Option<String>,
    pub act_chain: Vec<Actor>,
    pub jti: String,
    pub exp: i64,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct JwtExtractor;

impl JwtExtractor {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    fn build_validation() -> Validation {
        let mut validation = Validation::new(Algorithm::RS256);
        validation.validate_exp = true;
        validation.validate_aud = false;
        validation
    }

    fn decoding_key_for(token: &str) -> Result<&'static jsonwebtoken::DecodingKey, String> {
        let header = decode_header(token).map_err(|e| format!("invalid header: {e}"))?;
        if header.alg != Algorithm::RS256 {
            return Err("JWT must be RS256-signed".to_string());
        }
        let kid = header
            .kid
            .as_deref()
            .ok_or_else(|| "JWT missing `kid` header".to_string())?;
        authority::decoding_key_for_kid(kid)
            .map_err(|e| format!("key lookup: {e}"))?
            .ok_or_else(|| format!("unknown `kid` `{kid}`"))
    }

    #[allow(clippy::unused_self)]
    pub fn validate_token(&self, token: &str) -> Result<(), String> {
        let key = Self::decoding_key_for(token)?;
        match decode::<JwtClaims>(token, key, &Self::build_validation()) {
            Ok(_) => Ok(()),
            Err(err) => {
                let reason = err.to_string();
                if reason.contains("InvalidSignature") || reason.contains("invalid signature") {
                    Err("Invalid signature".to_string())
                } else if reason.contains("ExpiredSignature") || reason.contains("token expired") {
                    Err("Token expired".to_string())
                } else if reason.contains("MissingRequiredClaim") || reason.contains("missing") {
                    Err("Missing required claim".to_string())
                } else {
                    Err("Invalid token".to_string())
                }
            },
        }
    }

    #[allow(clippy::unused_self)]
    pub fn extract_user_context(&self, token: &str) -> Result<JwtUserContext> {
        let key = Self::decoding_key_for(token).map_err(|e| anyhow!(e))?;
        let token_data = decode::<JwtClaims>(token, key, &Self::build_validation())?;

        let session_id_str = token_data
            .claims
            .session_id
            .ok_or_else(|| anyhow!("JWT must contain session_id claim"))?;

        let role = *token_data
            .claims
            .scope
            .first()
            .ok_or_else(|| anyhow!("JWT must contain valid scope claim"))?;

        let client_id = token_data.claims.client_id.map(ClientId::new);

        // Defence-in-depth: the `user_type` claim is set at mint time from the
        // permission set; re-derive it here and reject any token whose claim
        // disagrees, so a forged or mis-minted type cannot ride past the gate.
        let derived_type = UserType::from_permissions(&token_data.claims.scope);
        if derived_type != token_data.claims.user_type {
            return Err(anyhow!(
                "user_type claim '{}' does not match permissions (derived '{}')",
                token_data.claims.user_type,
                derived_type
            ));
        }

        let act_chain = token_data
            .claims
            .act
            .as_ref()
            .map(systemprompt_models::auth::ActClaim::flatten_to_chain)
            .unwrap_or_default();

        Ok(JwtUserContext {
            user_id: UserId::new(token_data.claims.sub),
            session_id: SessionId::new(session_id_str),
            role,
            user_type: derived_type,
            client_id,
            roles: token_data.claims.roles,
            department: token_data.claims.department,
            act_chain,
            jti: token_data.claims.jti,
            exp: token_data.claims.exp,
        })
    }
}
