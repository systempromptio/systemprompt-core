//! Resolution of the subject a quota window is keyed by.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::{Arc, OnceLock};

use sqlx::PgPool;
use systemprompt_ai::USER_QUOTA_SUBJECT;
use systemprompt_identifiers::UserId;
use systemprompt_security::authz::{
    AuthzHookContext, NullAuditSink, SharedSubjectAttributeProvider, discover_subject_providers,
};

use super::QuotaWindow;

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

fn subject_providers(pool: &Arc<PgPool>) -> &'static [SharedSubjectAttributeProvider] {
    static PROVIDERS: OnceLock<Vec<SharedSubjectAttributeProvider>> = OnceLock::new();
    PROVIDERS.get_or_init(|| {
        discover_subject_providers(&AuthzHookContext {
            pool: Arc::clone(pool),
            sink: Arc::new(NullAuditSink),
        })
    })
}

pub(super) async fn resolve_subject<'a>(
    window: &'a QuotaWindow,
    user_id: &UserId,
    pool: &Arc<PgPool>,
) -> SubjectResolution<'a> {
    if window.subject == USER_QUOTA_SUBJECT {
        return SubjectResolution::Resolved(WindowSubject {
            kind: USER_QUOTA_SUBJECT,
            id: user_id.as_str().to_owned(),
        });
    }
    let Some(provider) = subject_providers(pool)
        .iter()
        .find(|p| p.dimension().rule_type.as_str() == window.subject)
    else {
        return SubjectResolution::Fault(FAULT_PROVIDER_MISSING);
    };
    let values = match provider.values_for(user_id).await {
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
