//! An authorization code minted for a device link. The code is bound to the
//! device key the link issued, and the refresh tokens that descend from it
//! carry that key, so the session it opens can be refused once the key no
//! longer stands.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{ApiKeyId, AuthorizationCode};

use super::MintAuthCodeParams;
use crate::error::{OauthError, OauthResult, ValidationError};
use crate::repository::oauth::OAuthRepository;

impl OAuthRepository {
    pub async fn mint_device_authorization_code(
        &self,
        params: MintAuthCodeParams<'_>,
        device_key: &ApiKeyId,
    ) -> OauthResult<AuthorizationCode> {
        let code = self.mint_authorization_code(params).await?;
        let code_hash = self.hash_at_rest(code.as_str())?;
        let bound = sqlx::query!(
            "UPDATE oauth_auth_codes SET device_key_id = $2 WHERE code = $1 AND used_at IS NULL",
            code_hash,
            device_key as _
        )
        .execute(self.write_pool_ref())
        .await?;
        if bound.rows_affected() != 1 {
            return Err(OauthError::Validation(ValidationError::Rejected {
                reason: "Invalid authorization code",
            }));
        }
        Ok(code)
    }
}
