//! The four brain tools: what each takes, what it tells the model, and the
//! service call behind it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Deserialize;
use serde_json::{Map, Value, json};
use systemprompt_identifiers::{BrainFactId, FactKindId, TopicId};
use systemprompt_models::brain::{EnteredFact, EnteredStatus, FactScope, FactStatus, SubjectKey};

use super::render::{fact_detail, page_text, why_text};
use super::tools::BrainOperation;
use crate::members::brain::client::FactQuery;
use crate::members::brain::error::BrainError;
use crate::members::brain::service::BrainService;

const MAX_LOOKUP: u32 = 50;

pub(super) struct Lookup;
pub(super) struct Why;
pub(super) struct Propose;
pub(super) struct Dispute;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LookupInput {
    fact_id: Option<BrainFactId>,
    kind: Option<FactKindId>,
    subject: Option<String>,
    #[serde(default)]
    status: Vec<FactStatus>,
    limit: Option<u32>,
    project: Option<String>,
    after: Option<String>,
}

impl BrainOperation for Lookup {
    type Input = LookupInput;
    const NAME: &'static str = "brain_lookup";
    const TITLE: &'static str = "Look up team memory";
    const DESCRIPTION: &'static str = "Read facts from the organisation's shared memory. Pass fact_id to read one fact with its fields, or filter a listing by kind, subject (written kind:id, such as client:acme), status and project. Each fact comes back as one line: id, revision, kind, status, scope, the statement and its subject. Only facts you may see are returned; a listing is paged, and the answer names the cursor to pass as after for the next page.";

    // JSON: a tool's input schema is JSON Schema, handed to the model as it is.
    fn schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "fact_id": { "type": "string", "description": "One fact's id, as m_<uuid>." },
                "kind": { "type": "string", "description": "Only facts of this declared kind." },
                "subject": { "type": "string", "description": "Only facts about this subject, written kind:id." },
                "status": {
                    "type": "array",
                    "items": { "type": "string", "enum": ["candidate", "proposed", "confirmed", "disputed", "stale", "superseded", "retired"] },
                    "description": "Only facts in one of these statuses."
                },
                "limit": { "type": "integer", "minimum": 1, "maximum": MAX_LOOKUP, "description": "At most this many facts (default 20)." },
                "project": { "type": "string", "description": "The project whose project-scoped facts may be shown." },
                "after": { "type": "string", "description": "The cursor of the page to continue from." }
            },
            "additionalProperties": false
        })
    }

    fn target(input: &LookupInput) -> String {
        match (&input.fact_id, &input.subject) {
            (Some(id), _) => id.to_string(),
            (None, Some(subject)) => subject.clone(),
            (None, None) => "*".to_owned(),
        }
    }

    async fn call(service: BrainService, input: LookupInput) -> Result<String, BrainError> {
        if let Some(id) = &input.fact_id {
            return Ok(fact_detail(&service.fact(id).await?));
        }
        let query = FactQuery {
            kind: input.kind,
            subject: input.subject.as_deref().map(subject).transpose()?,
            statuses: input.status,
            limit: Some(input.limit.unwrap_or(20).clamp(1, MAX_LOOKUP)),
            project: input.project,
            as_of: None,
            after: input.after,
        };
        Ok(page_text(&service.facts(&query).await?))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WhyInput {
    fact_id: BrainFactId,
    project: Option<String>,
}

impl BrainOperation for Why {
    type Input = WhyInput;
    const NAME: &'static str = "brain_why";
    const TITLE: &'static str = "Explain a memory fact";
    const DESCRIPTION: &'static str = "Explain why a fact from the organisation's shared memory ranks where it does: its importance and the terms behind it (base, subject match, workflow, recency, usage), and whether recall would offer it to a request or withhold it, with the reason. Pass the project to read the ranking as that project's requests see it.";

    // JSON: a tool's input schema is JSON Schema, handed to the model as it is.
    fn schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "fact_id": { "type": "string", "description": "The fact's id, as m_<uuid>." },
                "project": { "type": "string", "description": "The project to rank the fact for." }
            },
            "required": ["fact_id"],
            "additionalProperties": false
        })
    }

    fn target(input: &WhyInput) -> String {
        input.fact_id.to_string()
    }

    async fn call(service: BrainService, input: WhyInput) -> Result<String, BrainError> {
        let why = service
            .why(&input.fact_id, input.project.as_deref())
            .await?;
        Ok(why_text(&why))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ProposeInput {
    kind: FactKindId,
    subject: String,
    // JSON: fact fields are typed by the kind's declaration, which only the instance holds.
    fields: Map<String, Value>,
    #[serde(default)]
    topics: Vec<TopicId>,
    scope: ScopeInput,
}

#[derive(Deserialize)]
#[serde(
    tag = "level",
    content = "key",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum ScopeInput {
    Project(String),
    Group(String),
    Org,
}

impl BrainOperation for Propose {
    type Input = ProposeInput;
    const NAME: &'static str = "brain_propose";
    const TITLE: &'static str = "Propose a memory fact";
    const DESCRIPTION: &'static str = "Propose a new fact for the organisation's shared memory. Give a declared kind, the subject written kind:id, the kind's fields and the scope that may see it: a project or a group (with its key) or the whole org. The fact is queued as proposed and is not recalled until a person confirms it; a fact whose identity was erased is refused. Propose only what the conversation established, not guesses.";

    // JSON: a tool's input schema is JSON Schema, handed to the model as it is.
    fn schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "kind": { "type": "string", "description": "The declared kind of fact." },
                "subject": { "type": "string", "description": "What the fact is about, written kind:id." },
                "fields": { "type": "object", "description": "The kind's fields, by name." },
                "topics": { "type": "array", "items": { "type": "string" }, "description": "Declared topics to tag the fact with." },
                "scope": {
                    "type": "object",
                    "properties": {
                        "level": { "type": "string", "enum": ["project", "group", "org"] },
                        "key": { "type": "string", "description": "The project or group name; left out for org." }
                    },
                    "required": ["level"],
                    "additionalProperties": false
                }
            },
            "required": ["kind", "subject", "fields", "scope"],
            "additionalProperties": false
        })
    }

    fn target(input: &ProposeInput) -> String {
        input.subject.clone()
    }

    async fn call(service: BrainService, input: ProposeInput) -> Result<String, BrainError> {
        let scope = match input.scope {
            ScopeInput::Project(key) => FactScope::Project(named(key)?),
            ScopeInput::Group(key) => FactScope::Group(named(key)?),
            ScopeInput::Org => FactScope::Org,
        };
        let fact = EnteredFact {
            kind: input.kind,
            subject: subject(&input.subject)?,
            fields: input.fields,
            scope,
            topics: input.topics,
            status: EnteredStatus::Proposed,
            observed_at: None,
        };
        let outbox = service.enter(&fact).await?;
        Ok(format!(
            "Proposed a {} fact about {}; the instance queued it as event {outbox}. It is recalled only once a person confirms it.",
            fact.kind, fact.subject
        ))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DisputeInput {
    fact_id: BrainFactId,
    reason: String,
}

impl BrainOperation for Dispute {
    type Input = DisputeInput;
    const NAME: &'static str = "brain_dispute";
    const TITLE: &'static str = "Dispute a memory fact";
    const DESCRIPTION: &'static str = "Dispute a fact in the organisation's shared memory that is wrong or out of date, with the reason in a sentence or two. A disputed fact is no longer recalled until a person resolves the dispute, so dispute only what the conversation shows to be wrong.";

    // JSON: a tool's input schema is JSON Schema, handed to the model as it is.
    fn schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "fact_id": { "type": "string", "description": "The fact's id, as m_<uuid>." },
                "reason": { "type": "string", "minLength": 1, "description": "Why the fact is wrong." }
            },
            "required": ["fact_id", "reason"],
            "additionalProperties": false
        })
    }

    fn target(input: &DisputeInput) -> String {
        input.fact_id.to_string()
    }

    async fn call(service: BrainService, input: DisputeInput) -> Result<String, BrainError> {
        let reason = input.reason.trim();
        if reason.is_empty() {
            return Err(BrainError::InvalidInput {
                what: "a dispute needs a reason",
            });
        }
        let dispute = service.dispute(&input.fact_id, reason).await?;
        Ok(format!(
            "Disputed {}; dispute {dispute} stays open until a person resolves it, and the fact is not recalled meanwhile.",
            input.fact_id
        ))
    }
}

fn subject(raw: &str) -> Result<SubjectKey, BrainError> {
    raw.trim()
        .parse::<SubjectKey>()
        .map_err(|_malformed| BrainError::InvalidInput {
            what: "the subject is not written kind:id",
        })
}

fn named(key: String) -> Result<String, BrainError> {
    if key.trim().is_empty() {
        return Err(BrainError::InvalidInput {
            what: "a project or group scope needs its name as the key",
        });
    }
    Ok(key)
}
