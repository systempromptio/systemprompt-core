use std::collections::BTreeMap;

use systemprompt_brain::plane::{BrainPlane, DeclaredBrain};
use systemprompt_extension::prelude::{
    ChangeSource, ConfigPlane, PlaneContext, PlaneSnapshot, PlaneTx, ResolvedEntities,
    ResolvedEntity,
};
use systemprompt_identifiers::{FactKindId, TopicId, WorkflowId};
use systemprompt_models::brain::{
    ExtractorDecl, FieldDecl, FieldType, FiltersDoc, Granularity, ImportanceConfig, KindDecl,
    Materiality, RetrievalDoc, TopicDecl, WorkflowProfileDecl,
};
use systemprompt_models::notifications::AudienceSubject;
use systemprompt_traits::Dependencies;

use crate::fakes::PARSER;

pub(crate) const PRICING: &str = "pricing";
pub(crate) const OWNERSHIP: &str = "ownership";
pub(crate) const FINANCE: &str = "finance";
pub(crate) const ENGINEERING: &str = "engineering";
pub(crate) const PRICING_REVIEW_DAYS: u32 = 30;
pub(crate) const FINANCE_LEAD: &str = "finance_lead";
const SOURCE: &str = "brain/kinds/test.yaml";

pub(crate) fn kind(id: &str) -> FactKindId {
    FactKindId::try_new(id).expect("kind id")
}

pub(crate) fn topic(id: &str) -> TopicId {
    TopicId::try_new(id).expect("topic id")
}

fn text_field(required: bool) -> FieldDecl {
    FieldDecl {
        field_type: FieldType::Text,
        required,
        max_len: Some(200),
        entity_ref: None,
        values: Vec::new(),
    }
}

fn pricing_kind() -> KindDecl {
    KindDecl {
        kind: kind(PRICING),
        subject: "product".to_owned(),
        description: Some("A product's list price per region".to_owned()),
        fields: BTreeMap::from([
            ("price".to_owned(), text_field(true)),
            ("region".to_owned(), text_field(true)),
        ]),
        topics: vec![topic(FINANCE)],
        statement: "{subject} enterprise annual list price for {region} customers is {price} \
                    including support onboarding and premium maintenance"
            .to_owned(),
        identity: vec!["subject".to_owned(), "region".to_owned()],
        materiality: Materiality {
            require: vec!["price".to_owned()],
        },
        review_after_days: Some(PRICING_REVIEW_DAYS),
        extractor: Some(ExtractorDecl::Model {
            prompt: "Extract every list price the document states.".to_owned(),
            prompt_version: Some("v1".to_owned()),
        }),
    }
}

fn ownership_kind() -> KindDecl {
    KindDecl {
        kind: kind(OWNERSHIP),
        subject: "product".to_owned(),
        description: None,
        fields: BTreeMap::from([("team".to_owned(), text_field(true))]),
        topics: vec![topic(ENGINEERING)],
        statement: "{subject} is owned by the {team} team".to_owned(),
        identity: vec!["subject".to_owned()],
        materiality: Materiality::default(),
        review_after_days: None,
        extractor: Some(ExtractorDecl::Parser {
            name: PARSER.to_owned(),
        }),
    }
}

fn topic_decl(id: &str) -> TopicDecl {
    TopicDecl {
        id: topic(id),
        name: id.to_owned(),
        description: None,
        audience: vec![AudienceSubject::Everyone],
        granularity: Granularity::Individual,
    }
}

pub(crate) fn declared() -> DeclaredBrain {
    DeclaredBrain {
        kinds: vec![pricing_kind(), ownership_kind()],
        topics: vec![topic_decl(FINANCE), topic_decl(ENGINEERING)],
        interests: Vec::new(),
        workflows: vec![WorkflowProfileDecl {
            id: WorkflowId::try_new("pricing-review").expect("workflow id"),
            name: "pricing-review".to_owned(),
            kinds: vec![kind(PRICING)],
            confirm_role: Some(FINANCE_LEAD.to_owned()),
            quotas: BTreeMap::new(),
            budget_tokens: None,
        }],
        filters: Some(FiltersDoc::default()),
        importance: Some(ImportanceConfig::default()),
        retrieval: Some(RetrievalDoc::default()),
    }
}

fn resolved(brain: &DeclaredBrain) -> ResolvedEntities {
    brain
        .entity_list()
        .expect("entities encode")
        .into_iter()
        .map(|(key, value)| {
            (
                key,
                ResolvedEntity {
                    value: Some(value),
                    source: ChangeSource::Code(SOURCE.to_owned()),
                },
            )
        })
        .collect()
}

pub(crate) async fn apply(pool: &sqlx::PgPool, brain: &DeclaredBrain) {
    let deps = Dependencies::new();
    let root = std::env::temp_dir();
    let ctx = PlaneContext::new(&root, &deps);
    let mut tx = pool.begin().await.expect("begin");
    BrainPlane
        .write(&ctx, &mut PlaneTx::new(&mut tx), &resolved(brain))
        .await
        .expect("plane write");
    tx.commit().await.expect("commit");
}

pub(crate) async fn read_current(pool: &sqlx::PgPool) -> PlaneSnapshot {
    let deps = Dependencies::new();
    let root = std::env::temp_dir();
    let ctx = PlaneContext::new(&root, &deps);
    let mut tx = pool.begin().await.expect("begin");
    BrainPlane
        .read_current(&ctx, &mut PlaneTx::new(&mut tx))
        .await
        .expect("read current")
}

pub(crate) async fn export(pool: &sqlx::PgPool) -> Option<String> {
    let deps = Dependencies::new();
    let root = std::env::temp_dir();
    let ctx = PlaneContext::new(&root, &deps);
    let mut tx = pool.begin().await.expect("begin");
    BrainPlane
        .export(&ctx, &mut PlaneTx::new(&mut tx))
        .await
        .expect("export")
}
