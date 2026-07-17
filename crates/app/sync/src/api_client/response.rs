//! Response-handling helpers for [`crate::api_client::SyncApiClient`]:
//! converts HTTP failures into typed [`SyncError`] variants and decodes
//! JSON / binary success bodies.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use reqwest::StatusCode;
use serde::de::DeserializeOwned;

use crate::error::{SyncError, SyncResult};

pub(super) async fn handle_json<T: DeserializeOwned>(response: reqwest::Response) -> SyncResult<T> {
    let status = response.status();
    if status == StatusCode::UNAUTHORIZED {
        return Err(SyncError::Unauthorized);
    }
    if !status.is_success() {
        let message = response.text().await?;
        return Err(SyncError::ApiError {
            status: status.as_u16(),
            message,
        });
    }
    Ok(response.json().await?)
}

pub(super) async fn handle_binary(response: reqwest::Response) -> SyncResult<Vec<u8>> {
    let status = response.status();
    if !status.is_success() {
        let message = response
            .text()
            .await
            .unwrap_or_else(|e| format!("(body unreadable: {e})"));
        return Err(SyncError::ApiError {
            status: status.as_u16(),
            message,
        });
    }
    Ok(response.bytes().await?.to_vec())
}
