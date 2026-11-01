use systemprompt_extension::brain::RecallPrincipal;
use systemprompt_identifiers::{BrainFactId, UserId};
use systemprompt_models::auth::RequestActor;
use systemprompt_models::brain::{
    BrainFact, FactScope, FactSource, FactStatus, RetrievalTier, ScoredFact, TierRank,
};
use systemprompt_models::origin::ClientKind;

use crate::fixtures::{at, document_source, kind, subject, topic, user};

pub(crate) fn id(n: u32) -> BrainFactId {
    BrainFactId::try_new(format!("m_{n:08}-0000-4000-8000-000000000000")).expect("fact id")
}

pub(crate) fn brain_fact(n: u32, kind_id: &str) -> BrainFact {
    BrainFact {
        id: id(n),
        kind: kind(kind_id),
        subject: subject("client:acme"),
        scope: FactScope::Org,
        statement: format!("fact number {n}"),
        fields: serde_json::Map::new(),
        source: document_source("doc-1", "rev-1"),
        topics: Vec::new(),
        status: FactStatus::Confirmed,
        confirmed_by: None,
        confirmed_at: None,
        importance_base: 0.5,
        valid_from: None,
        valid_to: None,
        review_after: None,
        retention_until: None,
        legal_hold: false,
        identity_hash: format!("identity-{n}"),
        statement_hash: format!("statement-{n}"),
        revision: 1,
        pinned: false,
        owner_user_id: None,
        created_at: at(1_700_000_000),
        updated_at: at(1_700_000_000),
    }
}

pub(crate) fn with_scope(mut fact: BrainFact, scope: FactScope) -> BrainFact {
    fact.scope = scope;
    fact
}

pub(crate) fn with_topic(mut fact: BrainFact, topic_id: &str) -> BrainFact {
    fact.topics.push(topic(topic_id));
    fact
}

pub(crate) fn with_source(mut fact: BrainFact, source: FactSource) -> BrainFact {
    fact.source = source;
    fact
}

pub(crate) fn scored(fact: BrainFact, score: f64) -> ScoredFact {
    ScoredFact {
        fact,
        score,
        ranks: vec![TierRank {
            tier: RetrievalTier::FullText,
            rank: 1,
        }],
    }
}

pub(crate) fn principal(user_id: &str, project: Option<&str>) -> RecallPrincipal {
    RecallPrincipal {
        user_id: user(user_id),
        client: ClientKind::ClaudeCode,
        actor: RequestActor::Human,
        project: project.map(str::to_owned),
        client_id: None,
    }
}

pub(crate) fn uid(value: &str) -> UserId {
    user(value)
}
