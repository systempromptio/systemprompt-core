use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use systemprompt_extension::notifications::{
    DevicePushToken, PushTokenDirectory, PushTokenDirectoryError,
};
use systemprompt_identifiers::{DeviceId, NotificationId, UserId};
use systemprompt_models::devices::AppIdentifier;
use systemprompt_models::notifications::{
    ActionPrompt, Attention, AttentionReason, DeliveryState, Notification, PushProvider, PushToken,
};
use systemprompt_notifications::channels::push::{
    ForegroundPresence, ProviderBearer, ProviderCredential, ProviderHttp, ProviderRequest,
    ProviderResponse, PushError,
};
use systemprompt_traits::{
    ExtensionSecret, ReloadOutcome, SECRET_SEED_BYTES, SecretStore, SecretStoreError, SecretsStatus,
};

use crate::fixtures::{at, kind};

pub(crate) const TITLE: &str = "Quarterly figures need your approval";
pub(crate) const BODY: &str = "The agent wants to run a shell command";
pub(crate) const BEARER: &str = "provider-bearer";
pub(crate) const APP: &str = "io.example.app";

pub(crate) struct StubHttp {
    status: u16,
    body: String,
    requests: Mutex<Vec<ProviderRequest>>,
}

impl StubHttp {
    pub(crate) fn answering(status: u16, body: &str) -> Arc<Self> {
        Arc::new(Self {
            status,
            body: body.to_owned(),
            requests: Mutex::new(Vec::new()),
        })
    }

    pub(crate) fn requests(&self) -> Vec<ProviderRequest> {
        self.requests
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

#[async_trait]
impl ProviderHttp for StubHttp {
    async fn post(&self, request: ProviderRequest) -> Result<ProviderResponse, PushError> {
        self.requests
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(request);
        Ok(ProviderResponse {
            status: self.status,
            body: self.body.clone(),
        })
    }
}

pub(crate) struct StubCredential;

#[async_trait]
impl ProviderCredential for StubCredential {
    async fn bearer(&self) -> Result<ProviderBearer, PushError> {
        Ok(ProviderBearer::new(BEARER.to_owned()))
    }
}

pub(crate) struct StubDirectory {
    tokens: Vec<DevicePushToken>,
    revoked: Mutex<Vec<DevicePushToken>>,
}

impl StubDirectory {
    pub(crate) fn holding(tokens: Vec<DevicePushToken>) -> Arc<Self> {
        Arc::new(Self {
            tokens,
            revoked: Mutex::new(Vec::new()),
        })
    }

    pub(crate) fn revoked(&self) -> Vec<(DeviceId, PushProvider)> {
        self.revoked_tokens()
            .into_iter()
            .map(|token| (token.device_id, token.provider))
            .collect()
    }

    pub(crate) fn revoked_tokens(&self) -> Vec<DevicePushToken> {
        self.revoked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

#[async_trait]
impl PushTokenDirectory for StubDirectory {
    async fn tokens_for(
        &self,
        _user: &UserId,
    ) -> Result<Vec<DevicePushToken>, PushTokenDirectoryError> {
        Ok(self.tokens.clone())
    }

    async fn revoke(&self, token: &DevicePushToken) -> Result<(), PushTokenDirectoryError> {
        self.revoked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(token.clone());
        Ok(())
    }
}

pub(crate) struct StubPresence(pub(crate) bool);

#[async_trait]
impl ForegroundPresence for StubPresence {
    async fn is_foreground(&self, _user: &UserId) -> bool {
        self.0
    }
}

#[derive(Debug, Default)]
pub(crate) struct StubSecrets(HashMap<String, String>);

impl StubSecrets {
    pub(crate) fn holding(entries: &[(&str, &str)]) -> Arc<Self> {
        Arc::new(Self(
            entries
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                .collect(),
        ))
    }
}

fn unsupported(message: &'static str) -> SecretStoreError {
    SecretStoreError::new(message, std::io::Error::other("not held by the stub"))
}

#[async_trait]
impl SecretStore for StubSecrets {
    fn secret(&self, name: &str) -> Option<ExtensionSecret> {
        self.secret_exact(name)
    }

    fn secret_exact(&self, name: &str) -> Option<ExtensionSecret> {
        self.0
            .get(name)
            .map(|value| ExtensionSecret::new(value.clone()))
    }

    fn key_names(&self) -> Result<Vec<String>, SecretStoreError> {
        Ok(self.0.keys().cloned().collect())
    }

    fn manifest_signing_secret_seed(&self) -> Result<[u8; SECRET_SEED_BYTES], SecretStoreError> {
        Err(unsupported("the manifest signing seed"))
    }

    fn encryption_master_key(&self) -> Result<[u8; SECRET_SEED_BYTES], SecretStoreError> {
        Err(unsupported("the encryption master key"))
    }

    fn status(&self) -> SecretsStatus {
        SecretsStatus {
            source: "stub".to_owned(),
            generation: 0,
            initialised_at: Utc::now(),
            last_reload_at: None,
            last_error: None,
        }
    }

    fn refresh_interval(&self) -> Option<Duration> {
        None
    }

    async fn reload(&self) -> Result<ReloadOutcome, SecretStoreError> {
        Err(unsupported("reloading"))
    }
}

pub(crate) fn notification(kind_name: &str, attention: Attention) -> Notification {
    let actor = UserId::new("actor");
    Notification {
        id: NotificationId::generate(),
        recipient: UserId::new("recipient"),
        kind: kind(kind_name),
        stream: None,
        actor: actor.clone(),
        actor_is_agent: true,
        via_agent: None,
        subjects: Vec::new(),
        attention,
        attention_reason: AttentionReason::Default,
        action: ActionPrompt {
            prompt: BODY.to_owned(),
            open_url: Some("https://instance.example/runs/1".to_owned()),
            document_id: None,
            comment_id: None,
            kind: kind(kind_name),
            actor,
        },
        title: TITLE.to_owned(),
        body: BODY.to_owned(),
        coalesce_count: 1,
        first_event_at: at(1_000_000),
        last_event_at: at(1_000_000),
        created_at: at(1_000_000),
        read_at: None,
        escalated_from: None,
        escalation_note: None,
        held_until: None,
        delivery_state: DeliveryState::Pending,
    }
}

pub(crate) fn device(id: &str) -> DeviceId {
    DeviceId::try_new(id).expect("device id")
}

pub(crate) fn device_token(
    device: &str,
    provider: PushProvider,
    token: &str,
    app: &str,
) -> DevicePushToken {
    DevicePushToken {
        device_id: self::device(device),
        provider,
        token: PushToken::try_new(token).expect("push token"),
        app_id: AppIdentifier::try_new(app).expect("app identifier"),
    }
}
