//! Resolution of the subject a quota window is keyed by.
//!
//! `user` is the authenticated user and `api_key` the authenticating key.
//! Any other subject names a scope dimension: the request's attributed value
//! for it wins, and only a dimension the request was not attributed in falls
//! through to that dimension's subject-attribute provider (its first value).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_governance::authz::SubjectProviderSet;
use systemprompt_identifiers::QuotaSubjectKind;

use super::QuotaWindow;
use super::reserve::QuotaSubjects;

pub(super) struct WindowSubject {
    pub(super) kind: QuotaSubjectKind,
    pub(super) id: String,
}

pub(super) enum SubjectResolution {
    Resolved(WindowSubject),
    Fault(&'static str),
}

pub(super) const FAULT_PROVIDER_ERROR: &str = "subject attribute provider failed";
pub(super) const FAULT_PROVIDER_EMPTY: &str = "subject attribute provider returned no value";
pub(super) const FAULT_PROVIDER_MISSING: &str = "no subject attribute provider for this dimension";
pub(super) const FAULT_NO_API_KEY: &str = "request was not authenticated by an API key";

pub(super) async fn resolve_subject(
    window: &QuotaWindow,
    subjects: &QuotaSubjects<'_>,
    providers: &SubjectProviderSet,
) -> SubjectResolution {
    let Ok(kind) = window.subject.parse::<QuotaSubjectKind>() else {
        return SubjectResolution::Fault(FAULT_PROVIDER_MISSING);
    };
    match kind {
        QuotaSubjectKind::User => {
            return SubjectResolution::Resolved(WindowSubject {
                kind,
                id: subjects.user_id.as_str().to_owned(),
            });
        },
        QuotaSubjectKind::ApiKey => {
            return subjects
                .api_key_id
                .map_or(SubjectResolution::Fault(FAULT_NO_API_KEY), |key| {
                    SubjectResolution::Resolved(WindowSubject {
                        kind,
                        id: key.as_str().to_owned(),
                    })
                });
        },
        QuotaSubjectKind::Dimension(_) => {},
    }
    if let Some(value) = subjects.attribution.value_for(&window.subject) {
        return SubjectResolution::Resolved(WindowSubject {
            kind,
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
    SubjectResolution::Resolved(WindowSubject { kind, id })
}
