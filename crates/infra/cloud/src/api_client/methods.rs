//! Low-level HTTP verb helpers for [`super::CloudApiClient`].
//!
//! These methods are `pub(super)` because callers should use the
//! domain-specific methods in `endpoints.rs` and `tenant_api.rs`
//! rather than constructing paths by hand.

use serde::Serialize;
use serde::de::DeserializeOwned;

use super::CloudApiClient;
use crate::error::CloudResult;

impl CloudApiClient {
    pub(super) async fn get<T: DeserializeOwned>(&self, path: &str) -> CloudResult<T> {
        let url = format!("{}{}", self.api_url, path);
        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await?;
        self.handle_response(response).await
    }

    pub(super) async fn post<T: DeserializeOwned, B: Serialize + Sync>(
        &self,
        path: &str,
        body: &B,
    ) -> CloudResult<T> {
        let url = format!("{}{}", self.api_url, path);
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .json(body)
            .send()
            .await?;
        self.handle_response(response).await
    }

    pub(super) async fn post_no_response<B: Serialize + Sync>(
        &self,
        path: &str,
        body: &B,
    ) -> CloudResult<()> {
        let url = format!("{}{}", self.api_url, path);
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .json(body)
            .send()
            .await?;
        self.handle_no_content_response(response).await
    }

    pub(super) async fn put<T: DeserializeOwned, B: Serialize + Sync>(
        &self,
        path: &str,
        body: &B,
    ) -> CloudResult<T> {
        let url = format!("{}{}", self.api_url, path);
        let response = self
            .client
            .put(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .json(body)
            .send()
            .await?;
        self.handle_response(response).await
    }

    pub(super) async fn put_no_content<B: Serialize + Sync>(
        &self,
        path: &str,
        body: &B,
    ) -> CloudResult<()> {
        let url = format!("{}{}", self.api_url, path);
        let response = self
            .client
            .put(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .json(body)
            .send()
            .await?;
        self.handle_no_content_response(response).await
    }

    pub(super) async fn delete(&self, path: &str) -> CloudResult<()> {
        let url = format!("{}{}", self.api_url, path);
        let response = self
            .client
            .delete(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await?;
        self.handle_no_content_response(response).await
    }

    pub(super) async fn post_empty<T: DeserializeOwned>(&self, path: &str) -> CloudResult<T> {
        let url = format!("{}{}", self.api_url, path);
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await?;
        self.handle_response(response).await
    }
}
