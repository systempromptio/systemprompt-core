//! [`RoundCache::load`]: the preload one consumer round runs before its
//! per-event loop. One set claim of the result ledger for every decoded
//! event, then, for the entered facts among them, one alias resolution of
//! every distinct subject, one read of the dedupe rows and one of the
//! tombstones: four statements for the round instead of seven per
//! candidate. A round without entered facts claims and reads nothing else.
//!
//! The identity hash is computed here the way the candidate stage will
//! compute it, from the resolved subject and the entered fields; a stage
//! that changes either (a validator rewriting fields) misses the cache and
//! reads the store, which is correct and merely slower.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{HashMap, HashSet};

use sqlx::PgConnection;
use systemprompt_identifiers::{EventOutboxId, FactKindId};
use systemprompt_models::brain::{BrainEvent, EnteredFact, Pathway, SubjectKey};

use super::context::BatchContext;
use super::dedupe::LIVE_STATUSES;
use super::round::{RoundCache, RoundRows};
use super::rules::{ALIAS_THRESHOLD, identity_of};
use crate::error::BrainResult;
use crate::repository::{
    AliasRepository, DedupeSetRepository, EventResultRepository, TombstoneRepository,
};

impl RoundCache {
    pub async fn load(
        conn: &mut PgConnection,
        ctx: &BatchContext,
        events: &[(EventOutboxId, BrainEvent)],
    ) -> BrainResult<Self> {
        let claims: Vec<(EventOutboxId, String)> = events
            .iter()
            .map(|(id, event)| (id.clone(), event.kind.to_string()))
            .collect();
        let claimed: HashSet<EventOutboxId> =
            EventResultRepository::claim_many(&mut *conn, ctx.version(), &claims)
                .await?
                .into_iter()
                .collect();
        let entered = entered_facts(ctx, events);
        let mut rows = RoundRows {
            claimed,
            ..RoundRows::default()
        };
        if entered.is_empty() {
            return Ok(Self::new(rows));
        }
        rows.resolved = resolve_subjects(&mut *conn, &entered).await?;
        for fact in &entered {
            let Some(kind) = ctx.kind(&fact.kind) else {
                continue;
            };
            let subject = rows
                .resolved
                .get(&fact.subject)
                .cloned()
                .unwrap_or_else(|| fact.subject.clone());
            let hash = identity_of(kind, &subject, &fact.fields);
            rows.pairs.insert((fact.kind.clone(), subject));
            rows.identities.insert((fact.kind.clone(), hash));
        }
        let identities: Vec<(FactKindId, String)> = rows.identities.iter().cloned().collect();
        let pairs: Vec<(FactKindId, SubjectKey)> = rows.pairs.iter().cloned().collect();
        rows.facts =
            DedupeSetRepository::list_rows(&mut *conn, &identities, &pairs, &LIVE_STATUSES).await?;
        rows.tombstones = TombstoneRepository::list_existing(conn, &identities)
            .await?
            .into_iter()
            .collect();
        Ok(Self::new(rows))
    }
}

// JSON: an entered event's payload is the `EnteredFact` the sink wrote;
// one that does not decode is left to the normalise stage to refuse.
fn entered_facts(ctx: &BatchContext, events: &[(EventOutboxId, BrainEvent)]) -> Vec<EnteredFact> {
    events
        .iter()
        .filter(|(_, event)| Pathway::from_kind(&event.kind) == Some(Pathway::Entered))
        .filter_map(|(_, event)| serde_json::from_value::<EnteredFact>(event.payload.clone()).ok())
        .filter(|fact| ctx.kind(&fact.kind).is_some())
        .collect()
}

async fn resolve_subjects(
    conn: &mut PgConnection,
    entered: &[EnteredFact],
) -> BrainResult<HashMap<SubjectKey, SubjectKey>> {
    let mut subjects: Vec<SubjectKey> = Vec::new();
    for fact in entered {
        if !subjects.contains(&fact.subject) {
            subjects.push(fact.subject.clone());
        }
    }
    let resolved = AliasRepository::resolve_many(conn, &subjects, ALIAS_THRESHOLD).await?;
    Ok(resolved
        .into_iter()
        .map(|(subject, found)| {
            let target = found.map_or_else(
                || subject.clone(),
                |id| SubjectKey::new(subject.kind.clone(), id),
            );
            (subject, target)
        })
        .collect())
}
