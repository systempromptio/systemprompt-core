use systemprompt_loader::vertex_discovery::classify::{
    Classification, PublisherModel, classify, is_serverless,
};
use systemprompt_models::services::VertexRateCard;

fn model(json: serde_json::Value) -> PublisherModel {
    serde_json::from_value(json).expect("a listing entry deserializes")
}

fn card() -> VertexRateCard {
    VertexRateCard::embedded().expect("the embedded rate card parses")
}

// Rows below are lifted verbatim from a live listing (probed 2026-09-11), so
// the predicates are pinned to what Vertex actually returns rather than to a
// paraphrase of it.
fn qwen_next_instruct() -> PublisherModel {
    model(serde_json::json!({
        "name": "publishers/qwen/models/qwen3-next-80b-a3b-instruct-maas",
        "versionId": "001",
        "openSourceCategory": "THIRD_PARTY_OWNED_OSS",
        "supportedActions": {},
        "launchStage": "GA",
        "publisherModelTemplate": "projects/{project}/locations/{location}/publishers/google/models/qwen3-next-80b-a3b-instruct-maas@001"
    }))
}

fn deployable_checkpoint() -> PublisherModel {
    model(serde_json::json!({
        "name": "publishers/qwen/models/qwen3-5",
        "versionId": "qwen3.5-2b",
        "openSourceCategory": "THIRD_PARTY_OWNED_OSS",
        "supportedActions": {
            "deploy": {
                "modelDisplayName": "Qwen3.5-2B",
                "containerSpec": {"imageUri": "us-docker.pkg.dev/vertex-ai/serve:latest"}
            }
        },
        "launchStage": "GA"
    }))
}

fn gemini_flash() -> PublisherModel {
    model(serde_json::json!({
        "name": "publishers/google/models/gemini-2.5-flash",
        "versionId": "default",
        "supportedActions": {"openGenerationAiStudio": {"references": {}}},
        "launchStage": "GA"
    }))
}

fn gemini_embedding() -> PublisherModel {
    model(serde_json::json!({
        "name": "publishers/google/models/gemini-embedding-001",
        "versionId": "default",
        "openSourceCategory": "PROPRIETARY",
        "launchStage": "GA"
    }))
}

fn glm_5() -> PublisherModel {
    model(serde_json::json!({
        "name": "publishers/zai-org/models/glm-5-maas",
        "versionId": "001",
        "openSourceCategory": "THIRD_PARTY_OWNED_OSS",
        "launchStage": "EXPERIMENTAL"
    }))
}

#[test]
fn the_name_splits_into_publisher_and_model() {
    let qwen = qwen_next_instruct();
    assert_eq!(qwen.publisher(), "qwen");
    assert_eq!(qwen.model_name(), "qwen3-next-80b-a3b-instruct-maas");
    assert_eq!(qwen.upstream(), "qwen/qwen3-next-80b-a3b-instruct-maas");
}

#[test]
fn a_maas_partner_model_is_serverless_and_a_checkpoint_is_not() {
    assert!(is_serverless(&qwen_next_instruct()));
    assert!(
        !is_serverless(&deployable_checkpoint()),
        "a deploy action means weights plus a container, not an endpoint"
    );
    assert!(
        is_serverless(&glm_5()),
        "an absent supportedActions is as empty as an empty one"
    );
}

// Why: Google's own publisher is serverless across the board, so shape cannot
// separate chat from embeddings there — the rate card does it.
#[test]
fn a_google_model_is_serverless_but_only_published_if_it_is_priced() {
    let card = card();
    assert!(is_serverless(&gemini_flash()));
    assert!(is_serverless(&gemini_embedding()));
    assert_eq!(
        classify(&gemini_flash(), &card, "vertex").0,
        Classification::Publish
    );
    assert_eq!(
        classify(&gemini_embedding(), &card, "vertex").0,
        Classification::Unpriced
    );
}

#[test]
fn a_checkpoint_is_rejected_before_the_card_is_consulted() {
    let card = card();
    let (classification, entry) = classify(&deployable_checkpoint(), &card, "vertex-maas");
    assert_eq!(classification, Classification::NotServerless);
    assert!(entry.is_none());
}

// Why: a preview model can change or vanish under a running deployment, so
// publishing one is a per-model decision recorded on the card.
#[test]
fn a_preview_model_is_published_only_when_its_card_entry_allows_it() {
    let card = card();
    let (classification, entry) = classify(&glm_5(), &card, "vertex-maas");
    assert_eq!(classification, Classification::Publish);
    assert!(entry.expect("glm-5 is priced").allow_preview);

    let mut withheld = card;
    for entry in &mut withheld.entries {
        entry.allow_preview = false;
    }
    assert_eq!(
        classify(&glm_5(), &withheld, "vertex-maas").0,
        Classification::PreviewWithheld
    );
}

// Why: the two Vertex providers speak different wires, so a model priced for
// the MaaS surface must never be appended to the gemini one.
#[test]
fn an_entry_priced_for_one_provider_is_not_offered_to_the_other() {
    let card = card();
    assert_eq!(
        classify(&qwen_next_instruct(), &card, "vertex-maas").0,
        Classification::Publish
    );
    assert_eq!(
        classify(&qwen_next_instruct(), &card, "vertex").0,
        Classification::Unpriced
    );
}
