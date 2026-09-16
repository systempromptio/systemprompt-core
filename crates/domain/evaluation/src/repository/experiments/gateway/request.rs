//! Admission inputs and outcomes for gateway requests bound to an execution
//! session.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::Result;
use systemprompt_identifiers::{
    AiRequestId, EvalReservationId, ModelId, ProviderId, SessionId, UserId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestAdmission {
    Ordinary,
    Reserved(EvalReservationId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvaluationTrafficClass {
    Fixture,
    LiveEvaluation,
    Suggestion,
    Judge,
}

impl EvaluationTrafficClass {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Fixture => "fixture",
            Self::LiveEvaluation => "live_evaluation",
            Self::Suggestion => "suggestion",
            Self::Judge => "judge",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AdmissionRequest<'a> {
    pub owner: &'a UserId,
    pub session: &'a SessionId,
    pub request: &'a AiRequestId,
    pub model: &'a ModelId,
    pub provider: &'a ProviderId,
    pub bound_microdollars: i64,
}

#[derive(Debug, Clone, Copy)]
pub struct AdmissionRequestBuilder<'a> {
    owner: &'a UserId,
    session: &'a SessionId,
    request: Option<&'a AiRequestId>,
    model: Option<&'a ModelId>,
    provider: Option<&'a ProviderId>,
    bound_microdollars: Option<i64>,
}

impl<'a> AdmissionRequest<'a> {
    pub const fn builder(owner: &'a UserId, session: &'a SessionId) -> AdmissionRequestBuilder<'a> {
        AdmissionRequestBuilder {
            owner,
            session,
            request: None,
            model: None,
            provider: None,
            bound_microdollars: None,
        }
    }
}

impl<'a> AdmissionRequestBuilder<'a> {
    pub const fn request(mut self, request: &'a AiRequestId) -> Self {
        self.request = Some(request);
        self
    }
    pub const fn model(mut self, model: &'a ModelId) -> Self {
        self.model = Some(model);
        self
    }
    pub const fn provider(mut self, provider: &'a ProviderId) -> Self {
        self.provider = Some(provider);
        self
    }
    pub const fn bound_microdollars(mut self, amount: i64) -> Self {
        self.bound_microdollars = Some(amount);
        self
    }
    pub fn build(self) -> Result<AdmissionRequest<'a>> {
        let bound_microdollars = self
            .bound_microdollars
            .filter(|amount| *amount > 0)
            .ok_or_else(|| crate::experiments::invalid("Positive reservation bound required"))?;
        Ok(AdmissionRequest {
            owner: self.owner,
            session: self.session,
            request: self
                .request
                .ok_or_else(|| crate::experiments::invalid("Request ID required"))?,
            model: self
                .model
                .ok_or_else(|| crate::experiments::invalid("Model required"))?,
            provider: self
                .provider
                .ok_or_else(|| crate::experiments::invalid("Provider required"))?,
            bound_microdollars,
        })
    }
}
