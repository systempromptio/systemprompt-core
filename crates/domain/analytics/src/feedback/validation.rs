//! Canonical deduplication identities and bounded normalized facts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::{AnalyticsError, Result};
use chrono::{DateTime, Datelike, Utc};
use systemprompt_models::feedback::analytics::{
    AnalyticsChange, AnalyticsChangeOperation, AnalyticsFactKey, AnalyticsFactKind,
    NormalizedAnalyticsFact, RecordedSpend,
};

pub(super) const fn kind(value: AnalyticsFactKind) -> &'static str {
    match value {
        AnalyticsFactKind::Invocation => "invocation",
        AnalyticsFactKind::Request => "request",
        AnalyticsFactKind::Assessment => "assessment",
        AnalyticsFactKind::ResourceAssociation => "resource_association",
        AnalyticsFactKind::Artifact => "artifact",
    }
}

pub(super) fn validate(change: &AnalyticsChange) -> Result<()> {
    change.validate().map_err(|_error| invalid())?;
    if !(1..=9999).contains(&change.occurred_at.year())
        || !(1..=9999).contains(&change.recorded_at.year())
    {
        return Err(invalid());
    }
    if change.revision > i64::MAX as u64 || change.key.source.chars().any(char::is_control) {
        return Err(invalid());
    }
    reference(&change.key)?;
    if serde_json::to_vec(change)?.len() > 64 * 1024 {
        return Err(invalid());
    }
    let AnalyticsChangeOperation::Replace { fact } = &change.operation else {
        return Ok(());
    };
    let occurred_at = fact_occurred_at(fact, &change.key)?;
    if occurred_at != change.occurred_at {
        return Err(invalid());
    }
    Ok(())
}

pub(super) fn bounded(value: Option<u64>) -> Result<Option<i64>> {
    value
        .map(|value| i64::try_from(value).map_err(|_error| invalid()))
        .transpose()
}

pub(super) fn invalid() -> AnalyticsError {
    AnalyticsError::invalid_argument("Invalid or conflicting analytics evidence")
}

fn reference(key: &AnalyticsFactKey) -> Result<()> {
    if key.id.as_str().is_empty()
        || key.id.as_str().len() > 512
        || key.source.is_empty()
        || key.source.len() > 128
        || key.source.chars().any(char::is_control)
    {
        return Err(invalid());
    }
    Ok(())
}

pub(super) fn parse_kind(value: &str) -> Result<AnalyticsFactKind> {
    match value {
        "invocation" => Ok(AnalyticsFactKind::Invocation),
        "request" => Ok(AnalyticsFactKind::Request),
        "assessment" => Ok(AnalyticsFactKind::Assessment),
        "resource_association" => Ok(AnalyticsFactKind::ResourceAssociation),
        "artifact" => Ok(AnalyticsFactKind::Artifact),
        _ => Err(invalid()),
    }
}

pub(super) fn parse_state(value: &str) -> Result<super::FactChangeState> {
    use super::FactChangeState;
    match value {
        "pending" => Ok(FactChangeState::Pending),
        "leased" => Ok(FactChangeState::Leased),
        "applied" => Ok(FactChangeState::Applied),
        "superseded" => Ok(FactChangeState::Superseded),
        _ => Err(invalid()),
    }
}

fn spend_is_well_formed(spend: &RecordedSpend) -> Result<()> {
    if let RecordedSpend::Known {
        currency,
        amount_micros,
    } = spend
    {
        if currency.len() != 3 || !currency.bytes().all(|byte| byte.is_ascii_uppercase()) {
            return Err(invalid());
        }
        bounded(Some(*amount_micros))?;
    }
    Ok(())
}

fn fact_occurred_at(
    fact: &NormalizedAnalyticsFact,
    key: &AnalyticsFactKey,
) -> Result<DateTime<Utc>> {
    Ok(match fact {
        NormalizedAnalyticsFact::Invocation(value) => {
            if value.invocation_id.as_str() != key.id.as_str() {
                return Err(invalid());
            }
            bounded(value.latency_micros)?;
            value.occurred_at
        },
        NormalizedAnalyticsFact::Request(value) => {
            if value.request_key != *key {
                return Err(invalid());
            }
            bounded(value.input_tokens)?;
            bounded(value.output_tokens)?;
            bounded(value.latency_micros)?;
            spend_is_well_formed(&value.spend)?;
            value.occurred_at
        },
        NormalizedAnalyticsFact::Assessment(value) => {
            if value.conversation_key.id.as_str().is_empty()
                || value.conversation_key.id.as_str().len() > 512
                || value.conversation_key.source.is_empty()
                || value.conversation_key.source.len() > 128
                || value.conversation_key.source.chars().any(char::is_control)
            {
                return Err(invalid());
            }
            reference(&value.invocation_key)?;
            if value.assessment_key != *key
                || value.invocation_key.kind != AnalyticsFactKind::Invocation
            {
                return Err(invalid());
            }
            value.occurred_at
        },
        NormalizedAnalyticsFact::ResourceAssociation(value) => {
            reference(&value.invocation_key)?;
            reference(&value.request_key)?;
            if value.association_key != *key
                || value.invocation_key.kind != AnalyticsFactKind::Invocation
                || value.request_key.kind != AnalyticsFactKind::Request
            {
                return Err(invalid());
            }
            value.occurred_at
        },
        NormalizedAnalyticsFact::Artifact(value) => {
            if value.artifact_key != *key
                || value.execution_id.is_empty()
                || value.execution_id.len() > 512
                || value.tool_name.is_empty()
                || value.artifact_type.is_empty()
            {
                return Err(invalid());
            }
            if let Some(invocation) = &value.invocation_key {
                reference(invocation)?;
                if invocation.kind != AnalyticsFactKind::Invocation {
                    return Err(invalid());
                }
            }
            if let Some(request) = &value.request_key {
                reference(request)?;
                if request.kind != AnalyticsFactKind::Request {
                    return Err(invalid());
                }
            }
            bounded(value.payload_bytes)?;
            bounded(Some(value.findings))?;
            value.occurred_at
        },
    })
}
