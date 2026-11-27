use std::fmt::Write as _;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::json;
use sha2::{Digest as _, Sha256};
use sp_client::{
    CredentialError, CredentialSource, DeviceTokenSource, Freshness, HttpClient, SpClient,
};
use sp_drive::{DriveStatus, DriveSync, SyncAccount, SyncOptions};
use sp_protocol::Jwt;
use sp_protocol::ids::{DocumentId, DocumentRevisionId, DriveDeviceId};

mod context;
mod drive;
mod instance;

pub(crate) use context::{Gate, applied, context, tool_context};
pub(crate) use drive::{Doc, commits_naming, drive_state, fake_drive};
pub(crate) use instance::{FakeInstance, FakeRequest, FakeResponse};

pub(crate) const DOC: &str = "6f1c2b8e-1d2a-4c3b-9e8f-0a1b2c3d4e5f";
pub(crate) const DOC2: &str = "7a2d3c9f-2e3b-4d4c-8f90-1b2c3d4e5f60";
pub(crate) const DOC3: &str = "8b3e4d0a-3f4c-4e5d-9a01-2c3d4e5f6071";
pub(crate) const REV: &str = "11111111-2222-4333-8444-555555555555";
pub(crate) const REV2: &str = "22222222-3333-4444-8555-666666666666";
pub(crate) const USER: &str = "1ecce4e3-bc87-441d-b374-ea29bab88da8";
pub(crate) const DEVICE: &str = "install-1";

pub(crate) fn jwt() -> Jwt {
    Jwt::new("header.claims.signature").unwrap()
}

pub(crate) fn http() -> HttpClient {
    HttpClient::new("sp-drive-tests/0.0.0").unwrap()
}

pub(crate) fn sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest[..].iter().fold(String::new(), |mut out, byte| {
        write!(out, "{byte:02x}").unwrap();
        out
    })
}

pub(crate) fn document_id(raw: &str) -> DocumentId {
    serde_json::from_value(json!(raw)).unwrap()
}

pub(crate) fn revision_id(raw: &str) -> DocumentRevisionId {
    serde_json::from_value(json!(raw)).unwrap()
}

#[derive(Debug)]
pub(crate) struct StaticJwt;

#[async_trait]
impl CredentialSource for StaticJwt {
    async fn jwt(&self, _freshness: Freshness) -> Result<Jwt, CredentialError> {
        Ok(jwt())
    }

    async fn rejected(&self) {}
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum DeviceToken {
    Present,
    Missing,
    SignedOut,
}

#[async_trait]
impl DeviceTokenSource for DeviceToken {
    async fn device_token(&self) -> Result<Option<Jwt>, CredentialError> {
        match self {
            Self::Present => Ok(Some(jwt())),
            Self::Missing => Ok(None),
            Self::SignedOut => Err(CredentialError::SignedOut),
        }
    }

    async fn device_token_refused(&self) {}
}

pub(crate) fn options() -> SyncOptions {
    SyncOptions {
        poll_interval: Duration::from_secs(3600),
        debounce: Duration::from_secs(600),
        disabled_recheck: Duration::from_secs(3600),
        recent_items: 50,
    }
}

pub(crate) fn account(url: &str, root: &Path, token: DeviceToken) -> SyncAccount {
    SyncAccount {
        instance: SpClient::new(url, http()).unwrap(),
        http: http(),
        credentials: Arc::new(token),
        root: root.to_path_buf(),
        device_id: DriveDeviceId::try_new(DEVICE).unwrap(),
        host_name: Some("pc".to_owned()),
    }
}

pub(crate) async fn start_sync(fake: &FakeInstance, root: &Path) -> DriveSync {
    let sync = DriveSync::new(options());
    sync.start(account(&fake.url, root, DeviceToken::Present))
        .await
        .unwrap();
    sync
}

pub(crate) async fn eventually(what: &str, mut check: impl FnMut() -> bool) {
    for _ in 0..400 {
        if check() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("this never happened: {what}");
}

pub(crate) async fn status_where(
    sync: &DriveSync,
    what: &str,
    check: impl Fn(&DriveStatus) -> bool + Send + Sync,
) -> DriveStatus {
    let mut found = None;
    eventually(what, || {
        let status = sync.status();
        let holds = check(&status);
        if holds {
            found = Some(status);
        }
        holds
    })
    .await;
    found.unwrap()
}
