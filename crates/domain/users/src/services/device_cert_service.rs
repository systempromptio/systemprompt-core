use systemprompt_database::DbPool;
use systemprompt_identifiers::{DeviceCertId, UserId};

use crate::error::{Result, UserError};
use crate::models::UserDeviceCert;
use crate::repository::{EnrollDeviceCertParams, UserRepository};

const FINGERPRINT_LEN: usize = 64;

#[derive(Debug, Clone)]
pub struct EnrollParams<'a> {
    pub user_id: &'a UserId,
    pub fingerprint: &'a str,
    pub label: &'a str,
}

#[derive(Debug, Clone)]
pub struct DeviceCertService {
    repository: UserRepository,
}

impl DeviceCertService {
    pub fn new(db: &DbPool) -> anyhow::Result<Self> {
        Ok(Self {
            repository: UserRepository::new(db)?,
        })
    }

    pub async fn enroll(&self, params: EnrollParams<'_>) -> Result<UserDeviceCert> {
        let label = params.label.trim();
        if label.is_empty() {
            return Err(UserError::Validation(
                "device cert label must not be empty".into(),
            ));
        }
        let fingerprint = normalize_fingerprint(params.fingerprint)?;
        let id = DeviceCertId::generate();
        self.repository
            .enroll_device_cert(EnrollDeviceCertParams {
                id: &id,
                user_id: params.user_id,
                fingerprint: &fingerprint,
                label,
            })
            .await
    }

    pub async fn verify(&self, fingerprint: &str) -> Result<Option<UserDeviceCert>> {
        let normalized = normalize_fingerprint(fingerprint)?;
        self.repository
            .find_active_device_cert_by_fingerprint(&normalized)
            .await
    }

    pub async fn list_for_user(&self, user_id: &UserId) -> Result<Vec<UserDeviceCert>> {
        self.repository.list_device_certs_for_user(user_id).await
    }

    pub async fn revoke(&self, id: &DeviceCertId, user_id: &UserId) -> Result<bool> {
        self.repository.revoke_device_cert(id, user_id).await
    }
}

fn normalize_fingerprint(fingerprint: &str) -> Result<String> {
    let trimmed = fingerprint.trim().to_ascii_lowercase();
    if trimmed.len() != FINGERPRINT_LEN {
        return Err(UserError::Validation(format!(
            "device cert fingerprint must be {FINGERPRINT_LEN} hex chars (SHA-256)",
        )));
    }
    if !trimmed.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(UserError::Validation(
            "device cert fingerprint must be hex".into(),
        ));
    }
    Ok(trimmed)
}
