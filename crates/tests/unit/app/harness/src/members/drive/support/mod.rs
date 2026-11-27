//! A fake instance behind the Drive routes and a context over a throwaway
//! directory.

use std::time::Duration;

use async_trait::async_trait;
use serde_json::json;
use systemprompt_device::client::{CredentialError, CredentialSource, Freshness, HttpClient};
use systemprompt_device::protocol::Jwt;
use systemprompt_identifiers::{DocumentId, DocumentRevisionId};

mod context;
mod instance;

pub(crate) use context::{Gate, applied, context, tool_context};
pub(crate) use instance::{FakeInstance, FakeResponse};

pub(crate) const DOC: &str = "6f1c2b8e-1d2a-4c3b-9e8f-0a1b2c3d4e5f";
pub(crate) const REV: &str = "11111111-2222-4333-8444-555555555555";
pub(crate) const USER: &str = "1ecce4e3-bc87-441d-b374-ea29bab88da8";

pub(crate) fn jwt() -> Jwt {
    Jwt::new("header.claims.signature").unwrap()
}

pub(crate) fn http() -> HttpClient {
    HttpClient::new("sp-drive-tests/0.0.0").unwrap()
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

pub(crate) async fn eventually(what: &str, mut check: impl FnMut() -> bool) {
    for _ in 0..400 {
        if check() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("this never happened: {what}");
}
