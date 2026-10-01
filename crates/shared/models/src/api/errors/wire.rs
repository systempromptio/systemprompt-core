//! Redacting wire serialisation of [`ApiError`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::{Serialize, Serializer};

use super::{ApiError, ErrorCode, ValidationError};

#[derive(Serialize)]
struct WireApiError<'a> {
    code: ErrorCode,
    message: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_key: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<&'a str>,
    #[serde(skip_serializing_if = "no_validation_errors")]
    validation_errors: &'a [ValidationError],
    timestamp: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    trace_id: Option<&'a str>,
}

fn no_validation_errors(errors: &&[ValidationError]) -> bool {
    errors.is_empty()
}

impl Serialize for ApiError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let server_error = self.code.is_server_error();
        let message: &str = if server_error {
            self.code.public_server_message()
        } else {
            self.message.as_str()
        };
        let details = if server_error {
            None
        } else {
            self.details.as_deref()
        };
        let validation_errors: &[ValidationError] = if server_error {
            <&[ValidationError]>::default()
        } else {
            self.validation_errors.as_slice()
        };
        WireApiError {
            code: self.code,
            message,
            details,
            error_key: self.error_key.as_deref(),
            path: self.path.as_deref(),
            validation_errors,
            timestamp: self.timestamp,
            trace_id: self.trace_id.as_deref(),
        }
        .serialize(serializer)
    }
}
