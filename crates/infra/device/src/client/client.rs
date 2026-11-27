//! [`SpClient`]: one instance's base URL and the shared HTTP client.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use reqwest::RequestBuilder;
use reqwest::header::ACCEPT;
use sp_protocol::origin::{instance_origin, normalize_instance_url, resolve_instance_path};
use sp_protocol::wire::headers;
use sp_protocol::{HOST_ID, OriginError};
use url::Url;

use crate::error::ClientError;
use crate::http::HttpClient;

/// The client of one systemprompt instance. Cheap to clone; holds no
/// credential.
#[derive(Debug, Clone)]
pub struct SpClient {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    base: String,
    origin: String,
    base_url: Url,
    origin_url: Url,
    http: HttpClient,
}

impl SpClient {
    pub fn new(instance_url: &str, http: HttpClient) -> Result<Self, ClientError> {
        let base = normalize_instance_url(instance_url).map_err(ClientError::Origin)?;
        let origin = instance_origin(&base).map_err(ClientError::Origin)?;
        let base_url =
            Url::parse(&base).map_err(|error| ClientError::Origin(OriginError::Parse(error)))?;
        let origin_url =
            Url::parse(&origin).map_err(|error| ClientError::Origin(OriginError::Parse(error)))?;
        Ok(Self {
            inner: Arc::new(Inner {
                base,
                origin,
                base_url,
                origin_url,
                http,
            }),
        })
    }

    #[must_use]
    pub fn base_url(&self) -> &Url {
        &self.inner.base_url
    }

    #[must_use]
    pub fn base(&self) -> &str {
        &self.inner.base
    }

    #[must_use]
    pub fn origin(&self) -> &Url {
        &self.inner.origin_url
    }

    pub(crate) fn at(&self, path: &str) -> Result<Url, ClientError> {
        resolve_instance_path(&self.inner.base, path).map_err(ClientError::Origin)
    }

    pub(crate) fn at_origin(&self, path: &str) -> Result<Url, ClientError> {
        resolve_instance_path(&self.inner.origin, path).map_err(ClientError::Origin)
    }

    pub(crate) fn get(&self, url: Url) -> RequestBuilder {
        decorate(self.inner.http.reqwest().get(url))
    }

    pub(crate) fn post(&self, url: Url) -> RequestBuilder {
        decorate(self.inner.http.reqwest().post(url))
    }

    pub(crate) fn get_bytes(&self, url: Url) -> RequestBuilder {
        self.inner
            .http
            .reqwest()
            .get(url)
            .header(ACCEPT, "*/*")
            .header(headers::CLIENT, HOST_ID)
    }
}

fn decorate(request: RequestBuilder) -> RequestBuilder {
    request
        .header(ACCEPT, "application/json")
        .header(headers::CLIENT, HOST_ID)
}

pub(crate) fn push_segments<'a>(
    url: &mut Url,
    segments: impl IntoIterator<Item = &'a str>,
) -> Result<(), ClientError> {
    let mut path = url
        .path_segments_mut()
        .map_err(|()| ClientError::InvalidInput {
            what: "the instance URL cannot carry a path",
        })?;
    path.extend(segments);
    Ok(())
}
