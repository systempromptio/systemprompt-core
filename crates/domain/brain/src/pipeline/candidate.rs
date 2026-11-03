//! One candidate through the per-candidate stages: rules, validate,
//! render, dedupe, route, status, persist and notify, all on the event's
//! savepoint. With a round cache the alias resolution and the dedupe
//! snapshot come from it when it covers the candidate, and the write the
//! candidate lands is reported back as a [`DedupeWrite`] for the round.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgConnection;
use systemprompt_models::brain::{
    FactCandidate, FactStatus, KindDecl, SourceText, render_statement,
};

use super::dedupe::{self, DedupeDecision, DedupeSnapshot};
use super::normalise::Normalised;
use super::notify::{Notice, notify};
use super::persist::{Effect, PersistPlan, Persisted, persist};
use super::round::{CachedFact, DedupeWrite, is_live};
use super::rules::{self, InterestProbe};
use super::status::{self, Lifecycle};
use super::validate::{Validated, validate};
use super::{BrainPipeline, ProcessParams, route};
use crate::error::BrainResult;

/// What became of one candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateResult {
    Rejected { by: String, reason: String },
    Stored(Persisted),
}

#[derive(Debug, Clone, Copy)]
pub(super) struct EventScope<'a> {
    pub(super) event: &'a Normalised,
    pub(super) text: Option<&'a SourceText>,
}

struct Prepared<'a> {
    candidate: FactCandidate,
    kind: &'a KindDecl,
    statement: String,
    identity_hash: String,
    dispute_reason: Option<String>,
}

impl BrainPipeline {
    pub(super) async fn candidate(
        &self,
        conn: &mut PgConnection,
        params: &ProcessParams<'_>,
        scope: &EventScope<'_>,
        candidate: FactCandidate,
    ) -> BrainResult<(CandidateResult, Option<DedupeWrite>)> {
        let ctx = params.ctx;
        let event = scope.event;
        let prepared = match self.prepare(conn, params, candidate, scope.text).await? {
            Ok(prepared) => prepared,
            Err(rejected) => return Ok((rejected, None)),
        };
        let snapshot = snapshot(conn, params, &prepared).await?;
        let hash = statement_hash(&prepared);
        let decision = match prepared.dispute_reason {
            Some(_) => dedupe::forced_dispute(&snapshot).unwrap_or(DedupeDecision::Insert),
            None => dedupe::decide(&prepared.statement, &hash, &snapshot),
        };
        let routed = route::route(event, &prepared.candidate);
        let lifecycle = self.lifecycle(ctx, event, &prepared);
        let plan = PersistPlan {
            candidate: &prepared.candidate,
            statement: &prepared.statement,
            identity_hash: &prepared.identity_hash,
            routed: &routed,
            lifecycle,
            actor: event.actor(),
            version: ctx.version(),
            dispute_reason: prepared.dispute_reason.as_deref(),
            observed_at: event.observed_at(),
            reindex: params.reindex,
        };
        let persisted = persist(conn, &plan, decision).await?;
        let probe = InterestProbe {
            candidate: &prepared.candidate,
            statement: &prepared.statement,
            pathway: event.pathway(),
            now: ctx.loaded_at(),
        };
        let interests = rules::matching_interests(ctx.interests(), &probe);
        let notice = Notice {
            actor: event.actor(),
            candidate: &prepared.candidate,
            statement: &prepared.statement,
            status: lifecycle.status,
            persisted: &persisted,
            interests: &interests,
        };
        notify(self.notifications.as_ref(), conn, &notice).await?;
        let write = written(&prepared, &persisted, hash, lifecycle.status, params);
        Ok((CandidateResult::Stored(persisted), write))
    }

    async fn prepare<'a>(
        &self,
        conn: &mut PgConnection,
        params: &ProcessParams<'a>,
        mut candidate: FactCandidate,
        text: Option<&SourceText>,
    ) -> BrainResult<Result<Prepared<'a>, CandidateResult>> {
        let ctx = params.ctx;
        let Some(kind) = ctx.kind(&candidate.kind) else {
            return Ok(Err(rejected(
                "core.kinds",
                format!("kind {} is not declared", candidate.kind),
            )));
        };
        rules::add_kind_topics(&mut candidate, kind);
        let cached = params
            .round
            .and_then(|round| round.resolved(&candidate.subject));
        candidate.subject = match cached {
            Some(subject) => subject,
            None => rules::resolve_alias(conn, &candidate.subject).await?,
        };
        let (candidate, dispute_reason) =
            match validate(&self.seams.validators, candidate, kind, text, ctx.topics()).await? {
                Validated::Accept(candidate) => (candidate, None),
                Validated::Dispute { candidate, reason } => (candidate, Some(reason)),
                Validated::Reject { validator, reason } => {
                    return Ok(Err(rejected(&validator, reason)));
                },
            };
        let statement =
            match render_statement(&kind.statement, &candidate.subject, &candidate.fields) {
                Ok(statement) => statement,
                Err(error) => return Ok(Err(rejected("core.statement", error.to_string()))),
            };
        let identity_hash = rules::identity_of(kind, &candidate.subject, &candidate.fields);
        Ok(Ok(Prepared {
            candidate,
            kind,
            statement,
            identity_hash,
            dispute_reason,
        }))
    }

    fn lifecycle(
        &self,
        ctx: &super::BatchContext,
        event: &Normalised,
        prepared: &Prepared<'_>,
    ) -> Lifecycle {
        let status = if prepared.dispute_reason.is_some() {
            FactStatus::Disputed
        } else {
            status::status_for(
                &self.profile.capture.auto_confirm,
                event,
                &prepared.candidate,
            )
        };
        let confirmed = status == FactStatus::Confirmed;
        Lifecycle {
            status,
            importance_base: status::importance(
                &self.seams.importance,
                &prepared.candidate,
                ctx.importance(),
                confirmed,
            ),
            review_after: status::review_after(
                prepared.kind,
                self.profile.lifecycle.review_after_days,
                ctx.loaded_at(),
            ),
        }
    }
}

async fn snapshot(
    conn: &mut PgConnection,
    params: &ProcessParams<'_>,
    prepared: &Prepared<'_>,
) -> BrainResult<DedupeSnapshot> {
    let kind = &prepared.kind.kind;
    let subject = &prepared.candidate.subject;
    if let Some(cached) = params
        .round
        .and_then(|round| round.snapshot(kind, subject, &prepared.identity_hash))
    {
        return Ok(cached);
    }
    dedupe::snapshot(conn, kind, subject, &prepared.identity_hash).await
}

fn written(
    prepared: &Prepared<'_>,
    persisted: &Persisted,
    statement_hash: String,
    status: FactStatus,
    params: &ProcessParams<'_>,
) -> Option<DedupeWrite> {
    let id = persisted.fact.clone()?;
    match persisted.effect {
        Effect::Created => Some(DedupeWrite::Created(CachedFact {
            id,
            kind: prepared.kind.kind.clone(),
            subject: prepared.candidate.subject.clone(),
            identity_hash: prepared.identity_hash.clone(),
            statement: prepared.statement.clone(),
            statement_hash,
            live: is_live(status),
            updated_at: params.ctx.loaded_at(),
        })),
        Effect::Revised => Some(DedupeWrite::Revised {
            id,
            statement: prepared.statement.clone(),
            statement_hash,
            status,
        }),
        Effect::Unchanged | Effect::Merged | Effect::Disputed | Effect::Tombstoned => None,
    }
}

fn statement_hash(prepared: &Prepared<'_>) -> String {
    systemprompt_models::brain::statement_hash(&prepared.statement)
}

fn rejected(by: &str, reason: String) -> CandidateResult {
    CandidateResult::Rejected {
        by: by.to_owned(),
        reason,
    }
}
