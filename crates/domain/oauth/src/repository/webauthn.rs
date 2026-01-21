use anyhow::Result;
use chrono::{DateTime, Utc};
use systemprompt_identifiers::UserId;

#[derive(Debug, Clone)]
pub struct WebAuthnCredential {
    pub id: String,
    pub user_id: UserId,
    pub credential_id: Vec<u8>,
    pub public_key: Vec<u8>,
    pub counter: u32,
    pub display_name: String,
    pub device_type: String,
    pub transports: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

#[derive(Debug)]
pub struct WebAuthnCredentialParams<'a> {
    pub id: &'a str,
    pub user_id: &'a str,
    pub credential_id: &'a [u8],
    pub public_key: &'a [u8],
    pub counter: u32,
    pub display_name: &'a str,
    pub device_type: &'a str,
    pub transports: &'a [String],
}

#[derive(Debug)]
pub struct WebAuthnCredentialParamsBuilder<'a> {
    id: &'a str,
    user_id: &'a str,
    credential_id: &'a [u8],
    public_key: &'a [u8],
    counter: u32,
    display_name: &'a str,
    device_type: &'a str,
    transports: &'a [String],
}

impl<'a> WebAuthnCredentialParamsBuilder<'a> {
    pub const fn new(
        id: &'a str,
        user_id: &'a str,
        credential_id: &'a [u8],
        public_key: &'a [u8],
        counter: u32,
    ) -> Self {
        Self {
            id,
            user_id,
            credential_id,
            public_key,
            counter,
            display_name: "",
            device_type: "",
            transports: &[],
        }
    }

    pub const fn with_display_name(mut self, display_name: &'a str) -> Self {
        self.display_name = display_name;
        self
    }

    pub const fn with_device_type(mut self, device_type: &'a str) -> Self {
        self.device_type = device_type;
        self
    }

    pub const fn with_transports(mut self, transports: &'a [String]) -> Self {
        self.transports = transports;
        self
    }

    pub const fn build(self) -> WebAuthnCredentialParams<'a> {
        WebAuthnCredentialParams {
            id: self.id,
            user_id: self.user_id,
            credential_id: self.credential_id,
            public_key: self.public_key,
            counter: self.counter,
            display_name: self.display_name,
            device_type: self.device_type,
            transports: self.transports,
        }
    }
}

impl<'a> WebAuthnCredentialParams<'a> {
    pub const fn builder(
        id: &'a str,
        user_id: &'a str,
        credential_id: &'a [u8],
        public_key: &'a [u8],
        counter: u32,
    ) -> WebAuthnCredentialParamsBuilder<'a> {
        WebAuthnCredentialParamsBuilder::new(id, user_id, credential_id, public_key, counter)
    }
}

impl crate::repository::OAuthRepository {
    pub async fn store_webauthn_credential(
        &self,
        params: WebAuthnCredentialParams<'_>,
    ) -> Result<()> {
        let transports_json = serde_json::to_string(params.transports)?;
        let counter_i32 = i32::try_from(params.counter)
            .map_err(|_| anyhow::anyhow!("Counter exceeds i32::MAX"))?;
        let now = Utc::now();

        sqlx::query!(
            "INSERT INTO webauthn_credentials
             (id, user_id, credential_id, public_key, counter, display_name, device_type,
             transports, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
            params.id,
            params.user_id,
            params.credential_id,
            params.public_key,
            counter_i32,
            params.display_name,
            params.device_type,
            transports_json,
            now
        )
        .execute(self.pool_ref())
        .await?;

        Ok(())
    }

    pub async fn get_webauthn_credentials(&self, user_id: &str) -> Result<Vec<WebAuthnCredential>> {
        let rows = sqlx::query!(
            "SELECT id, user_id, credential_id, public_key, counter, display_name,
                    device_type, transports, created_at, last_used_at
             FROM webauthn_credentials WHERE user_id = $1 ORDER BY created_at DESC",
            user_id
        )
        .fetch_all(self.pool_ref())
        .await?;

        rows.into_iter()
            .map(|row| {
                let transports: Vec<String> = serde_json::from_str(&row.transports)?;
                let counter = u32::try_from(row.counter)
                    .map_err(|_| anyhow::anyhow!("Invalid counter value: {}", row.counter))?;
                Ok(WebAuthnCredential {
                    id: row.id,
                    user_id: row.user_id.into(),
                    credential_id: row.credential_id,
                    public_key: row.public_key,
                    counter,
                    display_name: row.display_name,
                    device_type: row.device_type,
                    transports,
                    created_at: row.created_at,
                    last_used_at: row.last_used_at,
                })
            })
            .collect()
    }

    pub async fn update_webauthn_credential_counter(
        &self,
        credential_id: &[u8],
        counter: u32,
    ) -> Result<()> {
        let counter_i32 =
            i32::try_from(counter).map_err(|_| anyhow::anyhow!("Counter exceeds i32::MAX"))?;
        let now = Utc::now();

        sqlx::query!(
            "UPDATE webauthn_credentials SET counter = $1, last_used_at = $2
             WHERE credential_id = $3",
            counter_i32,
            now,
            credential_id
        )
        .execute(self.pool_ref())
        .await?;

        Ok(())
    }
}
