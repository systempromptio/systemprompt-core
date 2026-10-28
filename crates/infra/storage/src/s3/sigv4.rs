//! AWS Signature Version 4 for single S3 object requests.
//!
//! Signs `host`, `x-amz-content-sha256`, `x-amz-date` and, for temporary
//! credentials, `x-amz-security-token`. The canonical query is passed in
//! already sorted and encoded; only the multipart calls carry one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use hmac::{Hmac, KeyInit, Mac};
use sha2::{Digest, Sha256};

use super::{S3Credentials, S3Error};

type HmacSha256 = Hmac<Sha256>;

pub(crate) struct SignedHeaders {
    pub amz_date: String,
    pub content_sha256: String,
    pub authorization: String,
}

pub(crate) struct SigningInput<'a> {
    pub method: &'a str,
    pub host: &'a str,
    pub canonical_uri: &'a str,
    pub canonical_query: &'a str,
    pub payload: &'a [u8],
    pub region: &'a str,
    pub now: DateTime<Utc>,
}

#[must_use]
pub(crate) fn sha256_hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

fn hmac(key: &[u8], data: &[u8]) -> Result<Vec<u8>, S3Error> {
    let mut mac = HmacSha256::new_from_slice(key).map_err(S3Error::SigningKey)?;
    mac.update(data);
    Ok(mac.finalize().into_bytes().to_vec())
}

pub(crate) fn sign(
    credentials: &S3Credentials,
    input: &SigningInput<'_>,
) -> Result<SignedHeaders, S3Error> {
    let amz_date = input.now.format("%Y%m%dT%H%M%SZ").to_string();
    let date = input.now.format("%Y%m%d").to_string();
    let content_sha256 = sha256_hex(input.payload);

    let mut headers = vec![
        ("host", input.host.to_owned()),
        ("x-amz-content-sha256", content_sha256.clone()),
        ("x-amz-date", amz_date.clone()),
    ];
    if let Some(token) = credentials.session_token.as_deref() {
        headers.push(("x-amz-security-token", token.to_owned()));
    }
    let canonical_headers = headers
        .iter()
        .fold(String::new(), |mut acc, (name, value)| {
            acc.push_str(name);
            acc.push(':');
            acc.push_str(value.trim());
            acc.push('\n');
            acc
        });
    let signed_headers = headers
        .iter()
        .map(|(name, _)| *name)
        .collect::<Vec<_>>()
        .join(";");
    let canonical_request = format!(
        "{}\n{}\n{}\n{canonical_headers}\n{signed_headers}\n{content_sha256}",
        input.method, input.canonical_uri, input.canonical_query
    );
    let scope = format!("{date}/{}/s3/aws4_request", input.region);
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}",
        sha256_hex(canonical_request.as_bytes())
    );
    let k_date = hmac(
        format!("AWS4{}", credentials.secret_access_key).as_bytes(),
        date.as_bytes(),
    )?;
    let k_region = hmac(&k_date, input.region.as_bytes())?;
    let k_service = hmac(&k_region, b"s3")?;
    let k_signing = hmac(&k_service, b"aws4_request")?;
    let signature = hex::encode(hmac(&k_signing, string_to_sign.as_bytes())?);
    Ok(SignedHeaders {
        amz_date,
        content_sha256,
        authorization: format!(
            "AWS4-HMAC-SHA256 Credential={}/{scope}, SignedHeaders={signed_headers}, \
             Signature={signature}",
            credentials.access_key_id
        ),
    })
}
