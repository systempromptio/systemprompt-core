//! JWT JTI (token identifier) revocation. Logout writes a row; the request
//! authenticator consults this table through
//! `systemprompt_database::JtiRevocationChecker` (the `TokenRevocations` the
//! kernel seeds) on every authenticated request. The `exp` column carries the
//! JWT's original expiry so cleanup can drop rows that are no longer
//! load-bearing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::OAuthRepository;
use crate::error::OauthResult;
use chrono::{DateTime, Utc};
use systemprompt_identifiers::{AccessTokenId, UserId};

impl OAuthRepository {
    pub async fn revoke_jti(
        &self,
        jti: &AccessTokenId,
        user_id: &UserId,
        exp: DateTime<Utc>,
    ) -> OauthResult<()> {
        sqlx::query!(
            "INSERT INTO oauth_jti_revocations (jti, user_id, exp)
             VALUES ($1, $2, $3)
             ON CONFLICT (jti) DO NOTHING",
            jti as _,
            user_id as _,
            exp,
        )
        .execute(self.write_pool_ref())
        .await?;
        Ok(())
    }

    pub async fn is_jti_revoked(&self, jti: &AccessTokenId) -> OauthResult<bool> {
        Ok(systemprompt_database::jti_revoked(self.write_pool_ref(), jti).await?)
    }

    pub async fn revoke_jtis_for_user(
        &self,
        user_id: &UserId,
        jtis: &[AccessTokenId],
        exp_floor: DateTime<Utc>,
    ) -> OauthResult<u64> {
        let mut inserted: u64 = 0;
        for jti in jtis {
            let result = sqlx::query!(
                "INSERT INTO oauth_jti_revocations (jti, user_id, exp)
                 VALUES ($1, $2, $3)
                 ON CONFLICT (jti) DO NOTHING",
                jti as _,
                user_id as _,
                exp_floor,
            )
            .execute(self.write_pool_ref())
            .await?;
            inserted += result.rows_affected();
        }
        Ok(inserted)
    }
}
