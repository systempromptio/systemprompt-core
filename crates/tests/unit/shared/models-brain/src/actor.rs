use systemprompt_identifiers::{AgentName, ClientId};
use systemprompt_models::auth::{RequestActor, TokenInteraction, bearer_actor};
use systemprompt_models::origin::MemoryPreference;

fn agent(name: &str, attended: bool) -> RequestActor {
    RequestActor::Agent {
        name: AgentName::new(name),
        attended,
    }
}

#[test]
fn bearer_actor_matrix() {
    let client = ClientId::new("sp_bridge");
    let interactive = Some(TokenInteraction::Interactive);
    let headless = Some(TokenInteraction::NonInteractive);
    let cases = [
        (
            Some("farm:run-1"),
            Some(&client),
            interactive,
            agent("farm:run-1", false),
        ),
        (
            Some("farm:run-1"),
            None,
            headless,
            agent("farm:run-1", false),
        ),
        (None, Some(&client), interactive, agent("sp_bridge", true)),
        (None, Some(&client), headless, agent("sp_bridge", false)),
        (None, Some(&client), None, agent("sp_bridge", false)),
        (None, None, interactive, agent("agent", true)),
        (None, None, None, agent("agent", false)),
    ];
    for (purpose, client, interaction, expected) in cases {
        assert_eq!(bearer_actor(purpose, client, interaction), expected);
    }
}

#[test]
fn invalid_names_fall_back() {
    let long = "a".repeat(65);
    assert_eq!(bearer_actor(Some(&long), None, None), agent("agent", false));
    assert_eq!(
        bearer_actor(Some("bad name"), None, None),
        agent("agent", false)
    );
    let client = ClientId::new("ok-client");
    assert_eq!(
        bearer_actor(Some(""), Some(&client), None),
        agent("agent", false)
    );
    let exact = "b".repeat(64);
    assert_eq!(bearer_actor(Some(&exact), None, None), agent(&exact, false));
}

#[test]
fn request_actor_wire_shape() {
    let encoded = serde_json::to_value(agent("x", true)).expect("encode");
    assert_eq!(
        encoded,
        serde_json::json!({"kind": "agent", "name": "x", "attended": true})
    );
    let human = serde_json::to_value(RequestActor::Human).expect("encode");
    assert_eq!(human, serde_json::json!({"kind": "human"}));
}

#[test]
fn memory_preference_parse() {
    for value in ["off", "OFF", "Off"] {
        assert_eq!(
            MemoryPreference::from_header(Some(value)),
            MemoryPreference::Off
        );
    }
    for value in [
        None,
        Some("on"),
        Some(""),
        Some("false"),
        Some("offf"),
        Some(" off"),
    ] {
        assert_eq!(
            MemoryPreference::from_header(value),
            MemoryPreference::Default
        );
    }
}
