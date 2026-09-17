//! `observability.otlp` profile checks.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::super::{OtlpExportConfig, OtlpProtocol, Profile};

impl Profile {
    pub(crate) fn validate_observability(&self, errors: &mut Vec<String>) {
        let Some(otlp) = self.observability.otlp() else {
            return;
        };
        validate_otlp(otlp, errors);
    }
}

pub(crate) fn validate_otlp(otlp: &OtlpExportConfig, errors: &mut Vec<String>) {
    match url::Url::parse(&otlp.endpoint) {
        Ok(parsed)
            if matches!(parsed.scheme(), "http" | "https") && parsed.host_str().is_some() => {},
        Ok(_) => errors.push(format!(
            "observability.otlp.endpoint must be an http:// or https:// URL with a host: {}",
            otlp.endpoint
        )),
        Err(e) => errors.push(format!(
            "observability.otlp.endpoint is not a valid URL ({e}): {}",
            otlp.endpoint
        )),
    }

    if otlp.protocol == OtlpProtocol::Grpc {
        errors.push(
            "observability.otlp.protocol 'grpc' is not yet supported — the exporter speaks \
             OTLP/HTTP (protobuf); point it at your collector's HTTP receiver (default port \
             4318) and set protocol: http"
                .to_owned(),
        );
    }

    if otlp.signals.is_empty() {
        errors
            .push("observability.otlp.signals must name at least one of: traces, logs".to_owned());
    }
    let mut seen = Vec::with_capacity(otlp.signals.len());
    for signal in &otlp.signals {
        if seen.contains(signal) {
            errors.push(format!(
                "observability.otlp.signals lists '{signal}' more than once"
            ));
        }
        seen.push(*signal);
    }

    if !(OtlpExportConfig::MIN_BATCH_SECONDS..=OtlpExportConfig::MAX_BATCH_SECONDS)
        .contains(&otlp.batch_seconds)
    {
        errors.push(format!(
            "observability.otlp.batch_seconds must be between {} and {} (got {})",
            OtlpExportConfig::MIN_BATCH_SECONDS,
            OtlpExportConfig::MAX_BATCH_SECONDS,
            otlp.batch_seconds
        ));
    }

    for (name, value) in &otlp.headers {
        if name.trim().is_empty() || name.chars().any(|c| c.is_whitespace() || c == ':') {
            errors.push(format!(
                "observability.otlp.headers has an invalid header name: {name:?}"
            ));
        }
        if value.chars().any(|c| c == '\r' || c == '\n') {
            errors.push(format!(
                "observability.otlp.headers.{name} must not contain line breaks"
            ));
        }
    }
}
