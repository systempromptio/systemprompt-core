//! Cloud Storage JSON API calls: media upload, media download, delete and
//! object resource reads.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use reqwest::{RequestBuilder, Response};
use serde::Deserialize;
use url::Url;

use super::{GcsError, GcsFileStorage};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ObjectResource {
    pub size: Option<String>,
    pub content_type: Option<String>,
    pub time_created: Option<DateTime<Utc>>,
    pub updated: Option<DateTime<Utc>>,
}

pub(crate) enum Lookup<T> {
    Found(T),
    Missing,
}

fn encode(segment: &str) -> String {
    urlencoding::encode(segment).into_owned()
}

impl GcsFileStorage {
    fn object_url(&self, name: &str) -> Result<Url, GcsError> {
        Ok(self.params.api_base.join(&format!(
            "storage/v1/b/{}/o/{}",
            encode(&self.params.bucket),
            encode(name)
        ))?)
    }

    async fn send(&self, request: RequestBuilder) -> Result<Response, GcsError> {
        let bearer = self.tokens.bearer().await?;
        let response = request.bearer_auth(bearer).send().await?;
        let status = response.status();
        if status.is_success() {
            return Ok(response);
        }
        Err(GcsError::Status {
            status: status.as_u16(),
            body: response.text().await?,
        })
    }

    pub(crate) async fn upload(
        &self,
        name: &str,
        content_type: &str,
        content: &[u8],
    ) -> Result<(), GcsError> {
        let mut url = self.params.upload_base.join(&format!(
            "upload/storage/v1/b/{}/o",
            encode(&self.params.bucket)
        ))?;
        url.query_pairs_mut()
            .append_pair("uploadType", "media")
            .append_pair("name", name);
        self.send(
            self.http
                .post(url)
                .header(reqwest::header::CONTENT_TYPE, content_type)
                .body(content.to_vec()),
        )
        .await?;
        Ok(())
    }

    pub(crate) async fn download(&self, name: &str) -> Result<Vec<u8>, GcsError> {
        let mut url = self.object_url(name)?;
        url.query_pairs_mut().append_pair("alt", "media");
        let response = self.send(self.http.get(url)).await?;
        Ok(response.bytes().await?.to_vec())
    }

    pub(crate) async fn remove(&self, name: &str) -> Result<(), GcsError> {
        let url = self.object_url(name)?;
        self.send(self.http.delete(url)).await?;
        Ok(())
    }

    pub(crate) async fn resource(&self, name: &str) -> Result<Lookup<ObjectResource>, GcsError> {
        let url = self.object_url(name)?;
        match self.send(self.http.get(url)).await {
            Ok(response) => Ok(Lookup::Found(serde_json::from_slice(
                &response.bytes().await?,
            )?)),
            Err(GcsError::Status { status: 404, .. }) => Ok(Lookup::Missing),
            Err(other) => Err(other),
        }
    }
}
