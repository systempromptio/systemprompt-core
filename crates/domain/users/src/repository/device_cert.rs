use systemprompt_identifiers::{DeviceCertId, UserId};

use crate::error::Result;
use crate::models::UserDeviceCert;
use crate::repository::UserRepository;

pub struct EnrollDeviceCertParams<'a> {
    pub id: &'a DeviceCertId,
    pub user_id: &'a UserId,
    pub fingerprint: &'a str,
    pub label: &'a str,
}

impl UserRepository {
    pub async fn enroll_device_cert(
        &self,
        params: EnrollDeviceCertParams<'_>,
    ) -> Result<UserDeviceCert> {
        let row = sqlx::query_as!(
            UserDeviceCert,
            r#"
            INSERT INTO user_device_certs (id, user_id, fingerprint, label)
            VALUES ($1, $2, $3, $4)
            RETURNING id, user_id, fingerprint, label, enrolled_at, revoked_at
            "#,
            params.id.as_str(),
            params.user_id.as_str(),
            params.fingerprint,
            params.label,
        )
        .fetch_one(&*self.write_pool)
        .await?;
        Ok(row)
    }

    pub async fn find_active_device_cert_by_fingerprint(
        &self,
        fingerprint: &str,
    ) -> Result<Option<UserDeviceCert>> {
        let row = sqlx::query_as!(
            UserDeviceCert,
            r#"
            SELECT id, user_id, fingerprint, label, enrolled_at, revoked_at
            FROM user_device_certs
            WHERE fingerprint = $1 AND revoked_at IS NULL
            "#,
            fingerprint,
        )
        .fetch_optional(&*self.pool)
        .await?;
        Ok(row)
    }

    pub async fn list_device_certs_for_user(
        &self,
        user_id: &UserId,
    ) -> Result<Vec<UserDeviceCert>> {
        let rows = sqlx::query_as!(
            UserDeviceCert,
            r#"
            SELECT id, user_id, fingerprint, label, enrolled_at, revoked_at
            FROM user_device_certs
            WHERE user_id = $1
            ORDER BY enrolled_at DESC
            "#,
            user_id.as_str(),
        )
        .fetch_all(&*self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn revoke_device_cert(&self, id: &DeviceCertId, user_id: &UserId) -> Result<bool> {
        let result = sqlx::query!(
            r#"
            UPDATE user_device_certs
            SET revoked_at = CURRENT_TIMESTAMP
            WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL
            "#,
            id.as_str(),
            user_id.as_str(),
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }
}
