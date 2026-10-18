//! Signed S3 object calls: `PUT`, `GET`, `DELETE` and `HEAD`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use reqwest::header::{CONTENT_LENGTH, CONTENT_TYPE, ETAG, HeaderMap, LAST_MODIFIED};
use reqwest::{Method, Response};
use systemprompt_traits::ByteRange;
use url::Url;

use super::sigv4::{SigningInput, sign};
use super::{S3Error, S3FileStorage};

pub(crate) struct ObjectHead {
    pub content_type: Option<String>,
    pub size_bytes: Option<i64>,
    pub etag: Option<String>,
    pub last_modified: DateTime<Utc>,
}

pub(crate) enum Lookup<T> {
    Found(T),
    Missing,
}

fn encode_key(name: &str) -> String {
    name.split('/')
        .map(|segment| urlencoding::encode(segment).into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

impl S3FileStorage {
    pub(crate) fn object_url(&self, name: &str) -> Result<(Url, String), S3Error> {
        let endpoint = &self.params.endpoint;
        let host = endpoint
            .host_str()
            .ok_or_else(|| S3Error::EndpointHost(endpoint.to_string()))?
            .to_owned();
        let base_path = endpoint.path().trim_end_matches('/');
        let key = encode_key(name);
        let mut url = endpoint.clone();
        let host = if self.params.path_style {
            url.set_path(&format!(
                "{base_path}/{}/{key}",
                urlencoding::encode(&self.params.bucket)
            ));
            host
        } else {
            let virtual_host = format!("{}.{host}", self.params.bucket);
            url.set_host(Some(&virtual_host))?;
            url.set_path(&format!("{base_path}/{key}"));
            virtual_host
        };
        let host_header = match url.port() {
            Some(port) => format!("{host}:{port}"),
            None => host,
        };
        Ok((url, host_header))
    }

    async fn send(
        &self,
        method: Method,
        name: &str,
        upload: Option<(&str, &[u8])>,
        range: Option<ByteRange>,
    ) -> Result<Response, S3Error> {
        let payload = upload.map_or(&b""[..], |(_, payload)| payload);
        let (url, host) = self.object_url(name)?;
        let signed = sign(
            &self.credentials,
            &SigningInput {
                method: method.as_str(),
                host: &host,
                canonical_uri: url.path(),
                payload,
                region: &self.params.region,
                now: Utc::now(),
            },
        )?;
        let mut request = self
            .http
            .request(method, url)
            .header("x-amz-date", signed.amz_date)
            .header("x-amz-content-sha256", signed.content_sha256)
            .header(reqwest::header::AUTHORIZATION, signed.authorization);
        if let Some(token) = self.credentials.session_token.as_deref() {
            request = request.header("x-amz-security-token", token);
        }
        if let Some(range) = range {
            request = request.header(reqwest::header::RANGE, range.header_value());
        }
        if let Some((content_type, payload)) = upload {
            request = request
                .header(CONTENT_TYPE, content_type)
                .body(payload.to_vec());
        }
        let response = request.send().await?;
        let status = response.status();
        if status.is_success() || status == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
            return Ok(response);
        }
        Err(S3Error::Status {
            status: status.as_u16(),
            body: response.text().await?,
        })
    }

    pub(crate) async fn put(
        &self,
        name: &str,
        content_type: &str,
        content: &[u8],
    ) -> Result<(), S3Error> {
        self.send(Method::PUT, name, Some((content_type, content)), None)
            .await?;
        Ok(())
    }

    pub(crate) async fn get(
        &self,
        name: &str,
        range: Option<ByteRange>,
    ) -> Result<Response, S3Error> {
        self.send(Method::GET, name, None, range).await
    }

    pub(crate) async fn remove(&self, name: &str) -> Result<(), S3Error> {
        self.send(Method::DELETE, name, None, None).await?;
        Ok(())
    }

    pub(crate) async fn head(&self, name: &str) -> Result<Lookup<ObjectHead>, S3Error> {
        match self.send(Method::HEAD, name, None, None).await {
            Ok(response) => Ok(Lookup::Found(parse_head(name, response.headers())?)),
            Err(S3Error::Status { status: 404, .. }) => Ok(Lookup::Missing),
            Err(other) => Err(other),
        }
    }
}

fn parse_head(name: &str, headers: &HeaderMap) -> Result<ObjectHead, S3Error> {
    let text = |header| headers.get(header).and_then(|v| v.to_str().ok());
    let size_bytes = text(CONTENT_LENGTH)
        .map(str::parse::<i64>)
        .transpose()
        .map_err(|source| S3Error::ObjectSize {
            name: name.to_owned(),
            source,
        })?;
    let last_modified = text(LAST_MODIFIED)
        .and_then(|v| DateTime::parse_from_rfc2822(v).ok())
        .map(|v| v.with_timezone(&Utc))
        .ok_or_else(|| S3Error::ObjectHeader {
            name: name.to_owned(),
            header: "last-modified",
        })?;
    Ok(ObjectHead {
        content_type: text(CONTENT_TYPE).map(str::to_owned),
        size_bytes,
        etag: text(ETAG).map(str::to_owned),
        last_modified,
    })
}
