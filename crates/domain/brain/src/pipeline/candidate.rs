//! One candidate through the per-candidate stages: rules, validate,
//! render, dedupe, route, status, persist and notify, all on the event's
//! savepoint.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgConnection;
use systemprompt_models::brain::{
    FactCandidate, FactStatus, KindDecl, SourceText, identity_hash, render_statement,
};

use super::context::BatchContext;
use super::dedupe::{self, DedupeDecision};
use super::normalise::Normalised;
use super::notify::{Notice, notify};
use super::persist::{Effect, PersistPlan, persist};
use super::rules::{self, InterestProbe};
use super::status::{self, Lifecycle};
use super::validate::{Validated, validate};
use super::{BrainPipeline, route};
use crate::error::BrainResult;

/// What became of one candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateResult {
    Rejected { by: String, reason: String },
    Stored(Effect),
}

struct Prepared<'a> {
    candidate: FactCandidate,
    kind: &'a KindDecl,
    statement: String,
    identity_hash: String,
    dispute_reason: Option<String>,
}

impl BrainPipeline {
    #[expect(
        clippy::too_many_arguments,
        reason = "the stage threads the event's savepoint connection and batch context with the candidate"
    )]
    pub(super) async fn candidate(
        &self,
        conn: &mut PgConnection,
        ctx: &BatchContext,
        event: &Normalised,
        candidate: FactCandidate,
        text: Option<&SourceText>,
    ) -> BrainResult<CandidateResult> {
        let prepared = match self.prepare(conn, ctx, candidate, text).await? {
            Ok(prepared) => prepared,
            Err(rejected) => return Ok(rejected),
        };
        let snapshot = dedupe::snapshot(
            conn,
            &prepared.kind.kind,
            &prepared.candidate.subject,
            &prepared.identity_hash,
        )
        .await?;
        let decision = match prepared.dispute_reason {
            Some(_) => dedupe::forced_dispute(&snapshot).unwrap_or(DedupeDecision::Insert),
            None => dedupe::decide(&prepared.statement, &statement_hash(&prepared), &snapshot),
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
        Ok(CandidateResult::Stored(persisted.effect))
    }

    async fn prepare<'a>(
        &self,
        conn: &mut PgConnection,
        ctx: &'a BatchContext,
        mut candidate: FactCandidate,
        text: Option<&SourceText>,
    ) -> BrainResult<Result<Prepared<'a>, CandidateResult>> {
        let Some(kind) = ctx.kind(&candidate.kind) else {
            return Ok(Err(rejected(
                "core.kinds",
                format!("kind {} is not declared", candidate.kind),
            )));
        };
        rules::add_kind_topics(&mut candidate, kind);
        candidate.subject = rules::resolve_alias(conn, &candidate.subject).await?;
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
        let identity_hash = identity_hash(
            &kind.kind,
            &candidate.subject,
            &kind.identity,
            &candidate.fields,
        );
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
        ctx: &BatchContext,
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

fn statement_hash(prepared: &Prepared<'_>) -> String {
    systemprompt_models::brain::statement_hash(&prepared.statement)
}

fn rejected(by: &str, reason: String) -> CandidateResult {
    CandidateResult::Rejected {
        by: by.to_owned(),
        reason,
    }
}
