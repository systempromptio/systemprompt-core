//! The one parsed form of a loopback request target.
//!
//! A request path is accepted only when it is already canonical: no `.` or
//! `..` segment (raw or percent-encoded), no encoded separator, no backslash
//! and no empty interior segment. Because nothing the upstream URL parser
//! would normalise survives [`ProxyPath::parse`], the route class decided
//! from this value and the upstream URL built from it name the same resource.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::ValidatedUrl;
use systemprompt_models::ApiPaths;
use thiserror::Error;

const ENCODED_FORBIDDEN: [&str; 4] = ["%2e", "%2f", "%5c", "%00"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PathRejection {
    #[error("request target is not an origin-form path")]
    NotOriginForm,
    #[error("request path contains a backslash")]
    Backslash,
    #[error("request path contains a percent-encoded dot, separator or NUL")]
    EncodedReserved,
    #[error("request path contains a `.` or `..` segment")]
    DotSegment,
    #[error("request path contains an empty segment")]
    EmptySegment,
    #[error("request path does not survive URL normalisation unchanged")]
    NotCanonical,
}

#[derive(Debug, Error)]
pub enum UpstreamUrlError {
    #[error("upstream url could not be built: {0}")]
    Parse(#[from] url::ParseError),
    #[error("upstream url path {actual} differs from the routed path {expected}")]
    Normalised { expected: String, actual: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyPath {
    path: String,
    query: Option<String>,
}

impl ProxyPath {
    pub fn parse(uri: &http::Uri) -> Result<Self, PathRejection> {
        let path = uri.path();
        let Some(rest) = path.strip_prefix('/') else {
            return Err(PathRejection::NotOriginForm);
        };
        if path.contains('\\') {
            return Err(PathRejection::Backslash);
        }
        let lowered = path.to_ascii_lowercase();
        if ENCODED_FORBIDDEN.iter().any(|enc| lowered.contains(enc)) {
            return Err(PathRejection::EncodedReserved);
        }
        if rest.split('/').any(|segment| matches!(segment, "." | "..")) {
            return Err(PathRejection::DotSegment);
        }
        if path.contains("//") {
            return Err(PathRejection::EmptySegment);
        }
        let canonical = url::Url::parse(&format!("http://loopback.invalid{path}"))
            .is_ok_and(|probe| probe.path() == path);
        if !canonical {
            return Err(PathRejection::NotCanonical);
        }
        Ok(Self {
            path: path.to_owned(),
            query: uri.query().map(str::to_owned),
        })
    }

    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    #[must_use]
    pub fn query(&self) -> Option<&str> {
        self.query.as_deref()
    }

    pub fn query_param(&self, name: &str) -> Option<&str> {
        self.query.as_deref()?.split('&').find_map(|kv| {
            let (k, v) = kv.split_once('=')?;
            (k == name).then_some(v)
        })
    }

    pub fn gateway_url(&self, gateway_base: &ValidatedUrl) -> Result<url::Url, UpstreamUrlError> {
        let base = url::Url::parse(gateway_base.as_str())?;
        let routed = if ApiPaths::is_otel(&self.path) {
            format!("{}{}", ApiPaths::GATEWAY_BASE, self.path)
        } else {
            self.path.clone()
        };
        let expected = format!("{}{routed}", base.path().trim_end_matches('/'));
        let mut upstream = base;
        upstream.set_path(&expected);
        upstream.set_query(self.query.as_deref());
        if upstream.path() != expected {
            return Err(UpstreamUrlError::Normalised {
                expected,
                actual: upstream.path().to_owned(),
            });
        }
        Ok(upstream)
    }
}
