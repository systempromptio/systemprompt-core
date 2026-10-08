//! Resolution of the subject a quota window is keyed by.
//!
//! `user` is the authenticated user and `api_key` the authenticating key.
//! Any other subject names a scope dimension: the request's attributed value
//! for it wins, and only a dimension the request was not attributed in falls
//! through to that dimension's subject-attribute provider (its first value).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_security::authz::SubjectProviderSet;

use super::QuotaWindow;
use super::reserve::QuotaSubjects;
use crate::policies::{API_KEY_QUOTA_SUBJECT, USER_QUOTA_SUBJECT};

pub(super) struct WindowSubject<'a> {
    pub(super) kind: &'a str,
    pub(super) id: String,
}

pub(super) enum SubjectResolution<'a> {
    Resolved(WindowSubject<'a>),
    Fault(&'static str),
}

pub(super) const FAULT_PROVIDER_ERROR: &str = "subject attribute provider failed";
pub(super) const FAULT_PROVIDER_EMPTY: &str = "subject attribute provider returned no value";
pub(super) const FAULT_PROVIDER_MISSING: &str = "no subject attribute provider for this dimension";
pub(super) const FAULT_NO_API_KEY: &str = "request was not authenticated by an API key";

pub(super) async fn resolve_subject<'a>(
    window: &'a QuotaWindow,
    subjects: &QuotaSubjects<'_>,
    providers: &SubjectProviderSet,
) -> SubjectResolution<'a> {
    if window.subject == USER_QUOTA_SUBJECT {
        return SubjectResolution::Resolved(WindowSubject {
            kind: USER_QUOTA_SUBJECT,
            id: subjects.user_id.as_str().to_owned(),
        });
    }
    if window.subject == API_KEY_QUOTA_SUBJECT {
        return subjects
            .api_key_id
            .map_or(SubjectResolution::Fault(FAULT_NO_API_KEY), |key| {
                SubjectResolution::Resolved(WindowSubject {
                    kind: API_KEY_QUOTA_SUBJECT,
                    id: key.as_str().to_owned(),
                })
            });
    }
    if let Some(value) = subjects.attribution.value_for(&window.subject) {
        return SubjectResolution::Resolved(WindowSubject {
            kind: &window.subject,
            id: value.to_owned(),
        });
    }
    let Some(provider) = providers.find(&window.subject) else {
        return SubjectResolution::Fault(FAULT_PROVIDER_MISSING);
    };
    let values = match provider.values_for(subjects.user_id).await {
        Ok(values) => values,
        Err(error) => {
            tracing::warn!(
                subject = %window.subject,
                %error,
                "Quota subject attribute provider failed"
            );
            return SubjectResolution::Fault(FAULT_PROVIDER_ERROR);
        },
    };
    let Some(id) = values.into_iter().next() else {
        tracing::warn!(
            subject = %window.subject,
            "Quota subject attribute provider returned no value"
        );
        return SubjectResolution::Fault(FAULT_PROVIDER_EMPTY);
    };
    SubjectResolution::Resolved(WindowSubject {
        kind: &window.subject,
        id,
    })
}
