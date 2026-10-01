//! Building OAuth error responses from failures that carry a cause.
//!
//! An OAuth `error_description` is client-visible and may be copied into a
//! third-party `redirect_uri`, so a cause never becomes description text: the
//! response carries an authored description and the cause chain goes to the
//! log, once, here.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::error::Error;

use systemprompt_oauth::OauthError;
use systemprompt_traits::BoxedSource;

use super::{OAuthErrorCode, OAuthHttpError};

pub fn server_error(context: &'static str, source: impl Into<BoxedSource>) -> OAuthHttpError {
    let source = source.into();
    tracing::error!(context, cause = %cause_chain(&*source), "OAuth internal failure");
    OAuthHttpError::server_error(context)
}

pub fn rejected(error: OAuthHttpError, source: impl Into<BoxedSource>) -> OAuthHttpError {
    let source = source.into();
    tracing::warn!(
        error = error.code().as_str(),
        description = error.description(),
        cause = %cause_chain(&*source),
        "OAuth request rejected"
    );
    error
}

pub fn is_conflict(error: &OauthError) -> bool {
    matches!(error, OauthError::Repository(source) if source.is_conflict())
}

pub fn classify_validation(
    error: OauthError,
    as_client_error: fn(String) -> OAuthHttpError,
) -> OAuthHttpError {
    match error {
        OauthError::Validation(message) | OauthError::InvalidClientMetadata(message) => {
            as_client_error(message)
        },
        other => OAuthHttpError::from(other),
    }
}

pub fn reclassify(
    error: OauthError,
    as_client_error: fn(String) -> OAuthHttpError,
) -> OAuthHttpError {
    let http = OAuthHttpError::from(error);
    if http.code() == OAuthErrorCode::ServerError {
        return http;
    }
    as_client_error(http.description().to_owned())
}

pub fn client_metadata_error(error: OauthError) -> OAuthHttpError {
    classify_validation(error, OAuthHttpError::invalid_client_metadata)
}

pub fn cause_chain(error: &(dyn Error + 'static)) -> String {
    let mut chain = error.to_string();
    let mut next = error.source();
    while let Some(cause) = next {
        chain.push_str(": ");
        chain.push_str(&cause.to_string());
        next = cause.source();
    }
    chain
}
