//! Device client info, push tokens and the admin device list on the user
//! repository.
//!
//! A push token row belongs to one device record and one provider. Writing a
//! token records what the device reported about itself and when, in the same
//! transaction, and only for a device that is still enrolled: a revoked
//! device writes nothing. A provider's token addresses one app install, so a
//! registration takes the token from any other device holding it and replaces
//! whatever the device held before.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_extension::notifications::DevicePushToken;
use systemprompt_identifiers::{DeviceCertId, DeviceId, UserId};
use systemprompt_models::devices::{AdminDeviceRow, DeviceClientInfo};
use systemprompt_models::notifications::{PushProvider, PushToken};
use systemprompt_traits::{BoxedSource, RepositoryError};

use crate::error::{Result, UserError};
use crate::repository::UserRepository;

#[derive(Debug)]
pub struct UpsertDevicePushTokenParams<'a> {
    pub device_id: &'a DeviceCertId,
    pub user_id: &'a UserId,
    pub provider: PushProvider,
    pub token: &'a PushToken,
    pub client: &'a DeviceClientInfo,
}

fn stored<T, E: Into<BoxedSource>>(
    context: &'static str,
    value: std::result::Result<T, E>,
) -> Result<T> {
    value.map_err(|source| {
        UserError::Repository(RepositoryError::Decode {
            context: context.to_owned(),
            source: source.into(),
        })
    })
}

pub(crate) fn device_id_of(cert: &DeviceCertId) -> Result<DeviceId> {
    stored("device id", DeviceId::try_new(cert.as_str()))
}

impl UserRepository {
    pub async fn upsert_device_push_token(
        &self,
        params: UpsertDevicePushTokenParams<'_>,
    ) -> Result<Option<DateTime<Utc>>> {
        let app_version = params.client.app_version.to_string();
        let mut tx = self.write_pool.begin().await?;
        let enrolled = sqlx::query!(
            r#"
            UPDATE user_device_certs
            SET platform = $3, os_version = $4, app_version = $5, app_id = $6,
                last_seen_at = CURRENT_TIMESTAMP
            WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL
            "#,
            params.device_id as _,
            params.user_id as _,
            params.client.platform.as_str(),
            params.client.os_version,
            app_version,
            &params.client.app_id as _,
        )
        .execute(&mut *tx)
        .await?;
        if enrolled.rows_affected() == 0 {
            return Ok(None);
        }
        sqlx::query!(
            r#"
            DELETE FROM user_device_push_tokens
            WHERE (device_id = $1 AND provider <> $2)
               OR (provider = $2 AND md5(token) = md5($3) AND token = $3 AND device_id <> $1)
            "#,
            params.device_id as _,
            params.provider.as_str(),
            params.token.expose(),
        )
        .execute(&mut *tx)
        .await?;
        let registered_at = sqlx::query_scalar!(
            r#"
            INSERT INTO user_device_push_tokens (device_id, user_id, provider, token, app_id)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (device_id, provider) DO UPDATE
            SET token = EXCLUDED.token, app_id = EXCLUDED.app_id,
                registered_at = CURRENT_TIMESTAMP
            RETURNING registered_at
            "#,
            params.device_id as _,
            params.user_id as _,
            params.provider.as_str(),
            params.token.expose(),
            &params.client.app_id as _,
        )
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(Some(registered_at))
    }

    pub async fn delete_device_push_tokens(
        &self,
        device_id: &DeviceCertId,
        user_id: &UserId,
    ) -> Result<bool> {
        let result = sqlx::query!(
            "DELETE FROM user_device_push_tokens WHERE device_id = $1 AND user_id = $2",
            device_id as _,
            user_id as _,
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_device_push_token_for_provider(
        &self,
        device_id: &DeviceId,
        provider: PushProvider,
    ) -> Result<bool> {
        let result = sqlx::query!(
            "DELETE FROM user_device_push_tokens WHERE device_id = $1 AND provider = $2",
            device_id as _,
            provider.as_str(),
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn list_live_push_tokens_for_user(
        &self,
        user_id: &UserId,
    ) -> Result<Vec<DevicePushToken>> {
        let rows = sqlx::query!(
            r#"
            SELECT t.device_id, t.provider, t.token, t.app_id
            FROM user_device_push_tokens t
            JOIN user_device_certs d ON d.id = t.device_id
            WHERE t.user_id = $1 AND d.revoked_at IS NULL
            ORDER BY t.registered_at, t.device_id
            "#,
            user_id as _,
        )
        .fetch_all(&*self.write_pool)
        .await?;
        rows.into_iter()
            .map(|row| {
                Ok(DevicePushToken {
                    device_id: stored("push token device id", DeviceId::try_new(row.device_id))?,
                    provider: stored("push token provider", row.provider.parse())?,
                    token: stored("push token", PushToken::try_new(row.token))?,
                    app_id: stored("push token app id", row.app_id.parse())?,
                })
            })
            .collect()
    }

    pub async fn list_admin_devices(
        &self,
        user_id: Option<&UserId>,
        limit: i64,
    ) -> Result<Vec<AdminDeviceRow>> {
        let rows = sqlx::query!(
            r#"
            SELECT d.id, d.user_id AS "user_id: UserId", d.label, d.platform, d.os_version,
                   d.app_version, d.app_id, d.enrolled_at, d.last_seen_at, d.revoked_at,
                   (SELECT t.provider FROM user_device_push_tokens t
                     WHERE t.device_id = d.id
                     ORDER BY t.registered_at DESC LIMIT 1) AS push_provider
            FROM user_device_certs d
            WHERE ($1::text IS NULL OR d.user_id = $1)
            ORDER BY d.enrolled_at DESC, d.id
            LIMIT $2
            "#,
            user_id as _,
            limit,
        )
        .fetch_all(&*self.pool)
        .await?;
        rows.into_iter()
            .map(|row| {
                Ok(AdminDeviceRow {
                    device_id: stored("device id", DeviceId::try_new(row.id))?,
                    user_id: row.user_id,
                    label: row.label,
                    platform: stored("device platform", row.platform.parse())?,
                    os_version: row.os_version,
                    app_version: stored(
                        "device app version",
                        row.app_version.as_deref().map(str::parse).transpose(),
                    )?,
                    app_id: stored(
                        "device app id",
                        row.app_id.as_deref().map(str::parse).transpose(),
                    )?,
                    push_provider: stored(
                        "device push provider",
                        row.push_provider.as_deref().map(str::parse).transpose(),
                    )?,
                    enrolled_at: row.enrolled_at,
                    last_seen_at: row.last_seen_at,
                    revoked_at: row.revoked_at,
                })
            })
            .collect()
    }
}
