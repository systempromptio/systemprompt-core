//! Indexed dimensions derived from validated typed facts, never independently
//! asserted.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::validation;
use crate::Result;
use systemprompt_identifiers::{
    AnalyticsFactId, DeviceId, ManagedResourceId, NativeSessionId, ResourceRevisionId, UserId,
};
use systemprompt_models::feedback::analytics::{
    AssessmentOutcome, InvocationConsumerIdentity, InvocationResourceAttribution,
    NormalizedAnalyticsFact, RecordedSpend,
};

#[derive(Default)]
pub(super) struct Columns {
    pub consumer: Option<UserId>,
    pub device: Option<DeviceId>,
    pub host: Option<String>,
    pub session: Option<NativeSessionId>,
    pub resource: Option<ManagedResourceId>,
    pub resource_revision: Option<ResourceRevisionId>,
    pub invocation_source: Option<String>,
    pub invocation: Option<AnalyticsFactId>,
    pub request_source: Option<String>,
    pub request: Option<AnalyticsFactId>,
    pub succeeded: Option<bool>,
    pub currency: Option<String>,
    pub amount: Option<i64>,
    pub input: Option<i64>,
    pub output: Option<i64>,
    pub latency: Option<i64>,
    pub conversation_source: Option<String>,
    pub conversation: Option<AnalyticsFactId>,
    pub assessment: Option<String>,
    pub score: Option<i64>,
}

impl Columns {
    pub(super) fn of(source: &str, fact: Option<&NormalizedAnalyticsFact>) -> Result<Self> {
        let mut columns = Self::default();
        match fact {
            Some(NormalizedAnalyticsFact::Invocation(value)) => {
                columns.identity(&value.consumer)?;
                columns.attribution(&value.attribution);
                columns.invocation_source = Some(source.to_owned());
                columns.invocation = Some(AnalyticsFactId::new(value.invocation_id.as_str()));
                columns.succeeded = Some(value.succeeded);
                columns.latency = validation::bounded(value.latency_micros)?;
            },
            Some(NormalizedAnalyticsFact::Request(value)) => {
                columns.identity(&value.consumer)?;
                columns.succeeded = Some(value.succeeded);
                columns.input = validation::bounded(value.input_tokens)?;
                columns.output = validation::bounded(value.output_tokens)?;
                columns.latency = validation::bounded(value.latency_micros)?;
                if let RecordedSpend::Known {
                    currency,
                    amount_micros,
                } = &value.spend
                {
                    columns.currency = Some(currency.clone());
                    columns.amount = validation::bounded(Some(*amount_micros))?;
                }
            },
            Some(NormalizedAnalyticsFact::Assessment(value)) => {
                columns.conversation_source = Some(value.conversation_key.source.clone());
                columns.conversation = Some(value.conversation_key.id.clone());
                columns.invocation_source = Some(value.invocation_key.source.clone());
                columns.invocation = Some(value.invocation_key.id.clone());
                columns.assessment = Some(
                    match value.outcome {
                        AssessmentOutcome::Scored { score_millionths } => {
                            columns.score = Some(score_millionths);
                            "scored"
                        },
                        AssessmentOutcome::Failed => "failed",
                        AssessmentOutcome::Unavailable => "unavailable",
                    }
                    .to_owned(),
                );
            },
            Some(NormalizedAnalyticsFact::ResourceAssociation(value)) => {
                columns.attribution(&value.attribution);
                columns.invocation_source = Some(value.invocation_key.source.clone());
                columns.invocation = Some(value.invocation_key.id.clone());
                columns.request_source = Some(value.request_key.source.clone());
                columns.request = Some(value.request_key.id.clone());
            },
            None => {},
        }
        Ok(columns)
    }

    fn identity(&mut self, identity: &InvocationConsumerIdentity) -> Result<()> {
        if let InvocationConsumerIdentity::Authenticated {
            consumer_id,
            device_id,
            host,
            session_id,
        } = identity
        {
            self.consumer = Some(consumer_id.clone());
            self.device = Some(device_id.clone());
            self.host = serde_json::to_value(host)?.as_str().map(str::to_owned);
            self.session = Some(session_id.clone());
        }
        Ok(())
    }

    fn attribution(&mut self, attribution: &InvocationResourceAttribution) {
        if let InvocationResourceAttribution::Verified {
            resource_id,
            revision_id,
        } = attribution
        {
            self.resource = Some(resource_id.clone());
            self.resource_revision = Some(revision_id.clone());
        }
    }
}
