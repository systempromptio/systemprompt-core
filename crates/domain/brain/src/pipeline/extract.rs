//! The extract stage.
//!
//! A person-entered fact is its own candidate. Any other event is read once
//! through the composite [`SourceContent`] under the profile's timeout and
//! handed to one extractor per gathered kind; a source nobody can read
//! fails closed. Each extraction is recorded once per event and pipeline
//! version, and the daily token budget is checked before every call.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use sqlx::PgConnection;
use systemprompt_extension::brain::{FactExtractor, SourceContent};
use systemprompt_models::brain::{
    ExtractionInput, ExtractorDecl, FactCandidate, FactSource, KindDecl, SourceText,
};

use super::context::BatchContext;
use super::normalise::{EventPayload, Normalised};
use super::{BrainPipeline, metrics};
use crate::error::{BrainError, BrainResult};
use crate::models::NewExtraction;
use crate::repository::ExtractionRepository;

pub const ENTERED_EXTRACTOR: &str = "entered";

/// What extraction produced: candidates and the source text they cite, or
/// why the source could not be read.
#[derive(Debug, Clone, Default)]
pub struct Extracted {
    pub candidates: Vec<FactCandidate>,
    pub text: Option<SourceText>,
    pub unreadable: Option<Unreadable>,
}

/// Why a source could not be read: no reader handles it, or the reader
/// found it gone.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Unreadable {
    #[error("no source content reads {0}")]
    NoReader(&'static str),
    #[error("source {0} is gone or unreadable")]
    Gone(String),
}

#[must_use]
pub fn entered_candidate(event: &Normalised) -> Option<FactCandidate> {
    let EventPayload::Entered(entered) = &event.payload else {
        return None;
    };
    let mut builder = FactCandidate::builder(
        entered.kind.clone(),
        entered.subject.clone(),
        event.source(),
    )
    .fields(entered.fields.clone())
    .scope(entered.scope.clone())
    .extractor(ENTERED_EXTRACTOR);
    for topic in &entered.topics {
        builder = builder.topic(topic.clone());
    }
    Some(builder.build())
}

#[must_use]
pub fn select_extractor<'a>(
    extensions: &'a [Arc<dyn FactExtractor>],
    core: &'a dyn FactExtractor,
    kind: &KindDecl,
) -> Option<&'a dyn FactExtractor> {
    if let Some(ExtractorDecl::Parser { name }) = &kind.extractor {
        return extensions
            .iter()
            .find(|e| e.name() == name && e.handles(kind))
            .map(AsRef::as_ref);
    }
    extensions
        .iter()
        .find(|e| e.handles(kind))
        .map(AsRef::as_ref)
        .or_else(|| core.handles(kind).then_some(core))
}

impl BrainPipeline {
    pub(super) async fn extract(
        &self,
        conn: &mut PgConnection,
        ctx: &BatchContext,
        event: &Normalised,
        kinds: &[&KindDecl],
    ) -> BrainResult<Extracted> {
        if let Some(candidate) = entered_candidate(event) {
            return Ok(Extracted {
                candidates: vec![candidate],
                ..Extracted::default()
            });
        }
        let source = event.source();
        let mut out = Extracted::default();
        for kind in kinds {
            let Some(extractor) = select_extractor(&self.seams.extractors, &self.model, kind)
            else {
                tracing::debug!(kind = %kind.kind, "No brain extractor handles the kind");
                continue;
            };
            if out.text.is_none() {
                match self.fetch_text(&source).await? {
                    Ok(text) => out.text = Some(text),
                    Err(reason) => {
                        out.unreadable = Some(reason);
                        return Ok(out);
                    },
                }
            }
            self.check_budget(conn, ctx.loaded_at()).await?;
            let input = ExtractionInput {
                envelope: event.envelope.clone(),
                kind: (*kind).clone(),
                source: source.clone(),
                text: out.text.clone(),
            };
            let output = extractor.extract(&input).await?;
            let candidates = output.candidates.len();
            let extraction = NewExtraction {
                outbox_id: event.envelope.outbox_id.clone(),
                pipeline_version: ctx.version().clone(),
                kind: kind.kind.clone(),
                record: output.record,
                candidates: u32::try_from(candidates).unwrap_or(u32::MAX),
            };
            ExtractionRepository::record(&mut *conn, &extraction).await?;
            metrics::extraction(extractor.name(), candidates);
            out.candidates.extend(
                output
                    .candidates
                    .into_iter()
                    .filter(|c| c.kind == kind.kind),
            );
        }
        Ok(out)
    }

    async fn fetch_text(&self, source: &FactSource) -> BrainResult<Result<SourceText, Unreadable>> {
        let Some(content) = self.seams.sources.iter().find(|s| s.handles(source)) else {
            return Ok(Err(Unreadable::NoReader(source.class().as_str())));
        };
        let timeout = Duration::from_millis(self.profile.consumer.source_timeout_ms);
        match tokio::time::timeout(timeout, fetch(content.as_ref(), source)).await {
            Err(_elapsed) => Err(BrainError::SourceTimeout),
            Ok(Err(error)) => Err(error),
            Ok(Ok(Some(text))) => Ok(Ok(text)),
            Ok(Ok(None)) => Ok(Err(Unreadable::Gone(source.label()))),
        }
    }

    async fn check_budget(&self, conn: &mut PgConnection, now: DateTime<Utc>) -> BrainResult<()> {
        let day_start = now
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .map_or(now, |midnight| midnight.and_utc());
        let used = ExtractionRepository::tokens_since(conn, day_start).await?;
        let budget = i64::try_from(self.profile.extraction.daily_token_budget).unwrap_or(i64::MAX);
        if used >= budget {
            metrics::budget_exhausted();
            return Err(BrainError::BudgetExhausted);
        }
        Ok(())
    }
}

async fn fetch(
    content: &dyn SourceContent,
    source: &FactSource,
) -> BrainResult<Option<SourceText>> {
    Ok(content.fetch(source).await?)
}
