use super::schema_check::assert_conforms;
use serde_json::json;
use std::collections::HashMap;
use systemprompt_identifiers::{ArtifactId, ContextId, MessageId, TaskId};
use systemprompt_models::a2a::{
    AgentCapabilities, AgentCard, AgentExtension, AgentSkill, ApiKeyLocation, ApiKeySecurityScheme,
    Artifact, ArtifactMetadata, AuthorizationCodeOAuthFlow, ClientCredentialsOAuthFlow,
    DeviceCodeOAuthFlow, HttpAuthSecurityScheme, Message, MessageRole, MutualTlsSecurityScheme,
    OAuth2SecurityScheme, OAuthFlows, OpenIdConnectSecurityScheme, PROTOCOL_VERSION, Part,
    PartContent, SecurityRequirement, SecurityScheme, SendMessageResult, StreamResponse, Task,
    TaskArtifactUpdateEvent, TaskState, TaskStatus, TaskStatusUpdateEvent, VERSION_HEADER,
};

fn message() -> Message {
    Message {
        role: MessageRole::User,
        parts: vec![Part::text("hello")],
        message_id: MessageId::new("msg-1"),
        task_id: None,
        context_id: ContextId::generate(),
        metadata: None,
        extensions: None,
        reference_task_ids: None,
    }
}

fn artifact() -> Artifact {
    Artifact {
        id: ArtifactId::new("art-1"),
        title: Some("Report".to_owned()),
        description: None,
        parts: vec![Part::data(json!({"rows": 3}))],
        extensions: Vec::new(),
        metadata: ArtifactMetadata::new(
            "table".to_owned(),
            ContextId::generate(),
            TaskId::new("task-1"),
        ),
    }
}

#[test]
fn text_part_with_media_type_round_trips() {
    let wire = json!({"text": "hi", "mediaType": "text/plain"});
    let part: Part = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(part.as_text(), Some("hi"));
    assert_eq!(part.media_type.as_deref(), Some("text/plain"));
    assert_eq!(serde_json::to_value(&part).unwrap(), wire);
}

#[test]
fn part_with_two_content_keys_is_rejected() {
    let err = serde_json::from_value::<Part>(json!({"text": "hi", "url": "https://x"}))
        .unwrap_err()
        .to_string();
    assert!(err.contains("2 content fields"), "{err}");
}

#[test]
fn part_without_content_is_rejected() {
    let err = serde_json::from_value::<Part>(json!({"mediaType": "text/plain"}))
        .unwrap_err()
        .to_string();
    assert!(err.contains("no content"), "{err}");
}

#[test]
fn legacy_file_part_is_rejected() {
    let legacy = json!({"file": {"name": "a.txt", "mimeType": "text/plain", "bytes": "aGk="}});
    assert!(serde_json::from_value::<Part>(legacy).is_err());
}

#[test]
fn raw_and_url_parts_carry_media_type_and_filename() {
    let raw = Part::raw("aGk=", "text/plain").with_filename("a.txt");
    assert_eq!(
        serde_json::to_value(&raw).unwrap(),
        json!({"raw": "aGk=", "filename": "a.txt", "mediaType": "text/plain"})
    );
    assert!(raw.is_file());
    assert_eq!(raw.as_raw(), Some("aGk="));

    let url = Part::url("https://example.com/a.png", "image/png");
    assert_eq!(url.as_url(), Some("https://example.com/a.png"));
    assert!(url.as_text().is_none());
}

#[test]
fn data_part_accepts_any_json_value_including_null() {
    for value in [json!({"k": 1}), json!([1, 2]), json!("s"), json!(null)] {
        let part: Part = serde_json::from_value(json!({"data": value.clone()})).unwrap();
        assert_eq!(part.content, PartContent::Data(value.clone()));
        assert_eq!(part.as_data(), Some(&value));
        assert_eq!(serde_json::to_value(&part).unwrap(), json!({"data": value}));
    }
}

#[test]
fn stream_response_is_externally_tagged_without_kind_or_final() {
    let event = TaskStatusUpdateEvent::new(
        TaskId::new("task-1"),
        ContextId::generate(),
        TaskStatus {
            state: TaskState::Working,
            message: None,
            timestamp: None,
        },
    );
    let value = serde_json::to_value(StreamResponse::StatusUpdate(event.clone())).unwrap();
    let inner = value.get("statusUpdate").unwrap();
    assert_eq!(inner["taskId"], "task-1");
    assert_eq!(inner["status"]["state"], "TASK_STATE_WORKING");
    assert!(inner.get("kind").is_none());
    assert!(inner.get("final").is_none());
    let back: StreamResponse = serde_json::from_value(value).unwrap();
    assert_eq!(back, StreamResponse::StatusUpdate(event));
}

#[test]
fn artifact_update_event_uses_spec_names() {
    let event =
        TaskArtifactUpdateEvent::new(TaskId::new("task-1"), ContextId::generate(), artifact())
            .chunk(true, false);
    let value = serde_json::to_value(StreamResponse::ArtifactUpdate(event)).unwrap();
    let inner = &value["artifactUpdate"];
    assert_eq!(inner["append"], true);
    assert_eq!(inner["lastChunk"], false);
    assert_eq!(inner["artifact"]["artifactId"], "art-1");
    assert_eq!(inner["artifact"]["name"], "Report");
    assert!(inner.get("kind").is_none());
}

#[test]
fn send_message_result_tags_task_and_message() {
    let task = Task::default();
    let value = serde_json::to_value(SendMessageResult::Task(Box::new(task.clone()))).unwrap();
    assert!(value["task"].get("createdAt").is_none());
    assert!(value["task"].get("lastModified").is_none());
    let expected = Task {
        created_at: None,
        last_modified: None,
        ..task
    };
    assert_eq!(
        serde_json::from_value::<SendMessageResult>(value).unwrap(),
        SendMessageResult::Task(Box::new(expected))
    );

    let value = serde_json::to_value(SendMessageResult::Message(Box::new(message()))).unwrap();
    assert_eq!(value["message"]["role"], "ROLE_USER");
}

#[test]
fn agent_card_uses_1_0_names() {
    let mut card = AgentCard::builder(
        "a".to_owned(),
        "d".to_owned(),
        "https://agent".to_owned(),
        "2.0.0".to_owned(),
    )
    .with_extended_agent_card()
    .build();
    card.security_requirements = Some(vec![SecurityRequirement::scheme(
        "oauth2",
        vec!["admin".to_owned()],
    )]);
    let value = serde_json::to_value(&card).unwrap();
    assert_eq!(
        value["supportedInterfaces"][0]["protocolVersion"],
        PROTOCOL_VERSION
    );
    assert_eq!(value["capabilities"]["extendedAgentCard"], true);
    assert!(
        value["capabilities"]
            .get("stateTransitionHistory")
            .is_none()
    );
    assert!(value.get("supportsAuthenticatedExtendedCard").is_none());
    assert_eq!(
        value["securityRequirements"],
        json!([{"schemes": {"oauth2": {"list": ["admin"]}}}])
    );
    assert_eq!(serde_json::from_value::<AgentCard>(value).unwrap(), card);
}

#[test]
fn protocol_constants_match_spec() {
    assert_eq!(PROTOCOL_VERSION, "1.0");
    assert_eq!(VERSION_HEADER, "A2A-Version");
}

fn every_scheme() -> Vec<(&'static str, SecurityScheme)> {
    let scopes = HashMap::from([("admin".to_owned(), "Administer".to_owned())]);
    vec![
        (
            "apiKeySecurityScheme",
            SecurityScheme::ApiKey(ApiKeySecurityScheme {
                description: Some("key".to_owned()),
                location: ApiKeyLocation::Header,
                name: "X-Api-Key".to_owned(),
            }),
        ),
        (
            "httpAuthSecurityScheme",
            SecurityScheme::HttpAuth(HttpAuthSecurityScheme {
                description: None,
                scheme: "Bearer".to_owned(),
                bearer_format: Some("JWT".to_owned()),
            }),
        ),
        (
            "oauth2SecurityScheme",
            SecurityScheme::OAuth2(OAuth2SecurityScheme {
                description: None,
                flows: OAuthFlows::AuthorizationCode(AuthorizationCodeOAuthFlow {
                    authorization_url: "https://idp/authorize".to_owned(),
                    token_url: "https://idp/token".to_owned(),
                    refresh_url: None,
                    scopes: scopes.clone(),
                    pkce_required: true,
                }),
                oauth2_metadata_url: Some(
                    "https://idp/.well-known/oauth-authorization-server".to_owned(),
                ),
            }),
        ),
        (
            "oauth2SecurityScheme",
            SecurityScheme::OAuth2(OAuth2SecurityScheme {
                description: None,
                flows: OAuthFlows::ClientCredentials(ClientCredentialsOAuthFlow {
                    token_url: "https://idp/token".to_owned(),
                    refresh_url: Some("https://idp/refresh".to_owned()),
                    scopes: scopes.clone(),
                }),
                oauth2_metadata_url: None,
            }),
        ),
        (
            "oauth2SecurityScheme",
            SecurityScheme::OAuth2(OAuth2SecurityScheme {
                description: None,
                flows: OAuthFlows::DeviceCode(DeviceCodeOAuthFlow {
                    device_authorization_url: "https://idp/device".to_owned(),
                    token_url: "https://idp/token".to_owned(),
                    refresh_url: None,
                    scopes,
                }),
                oauth2_metadata_url: None,
            }),
        ),
        (
            "openIdConnectSecurityScheme",
            SecurityScheme::OpenIdConnect(OpenIdConnectSecurityScheme {
                description: None,
                open_id_connect_url: "https://idp/.well-known/openid-configuration".to_owned(),
            }),
        ),
        (
            "mtlsSecurityScheme",
            SecurityScheme::MutualTls(MutualTlsSecurityScheme {
                description: Some("client cert".to_owned()),
            }),
        ),
    ]
}

#[test]
fn every_security_scheme_is_a_schema_conformant_oneof() {
    for (key, scheme) in every_scheme() {
        let value = serde_json::to_value(&scheme).unwrap();
        let object = value.as_object().unwrap();
        assert_eq!(object.keys().collect::<Vec<_>>(), vec![key]);
        assert_conforms("Security Scheme", &value);
        assert_eq!(
            serde_json::from_value::<SecurityScheme>(value).unwrap(),
            scheme
        );
    }
}

#[test]
fn oauth_flows_wire_names() {
    let value = serde_json::to_value(&every_scheme()[2].1).unwrap();
    let flow = &value["oauth2SecurityScheme"]["flows"]["authorizationCode"];
    assert_eq!(flow["pkceRequired"], true);
    assert_eq!(flow["authorizationUrl"], "https://idp/authorize");
    assert_eq!(
        value["oauth2SecurityScheme"]["oauth2MetadataUrl"],
        "https://idp/.well-known/oauth-authorization-server"
    );
}

#[test]
fn legacy_security_scheme_shapes_are_rejected() {
    let legacy_tagged = json!({"type": "apiKey", "name": "k", "in": "header"});
    assert!(serde_json::from_value::<SecurityScheme>(legacy_tagged).is_err());
    let implicit = json!({"oauth2SecurityScheme": {"flows": {"implicit": {
        "authorizationUrl": "https://idp/authorize", "scopes": {}
    }}}});
    assert!(serde_json::from_value::<SecurityScheme>(implicit).is_err());
}

#[test]
fn full_agent_card_conforms_to_schema() {
    let mut card = AgentCard::builder(
        "agent".to_owned(),
        "desc".to_owned(),
        "https://agent/a2a".to_owned(),
        "2.0.0".to_owned(),
    )
    .with_streaming()
    .with_extended_agent_card()
    .with_provider("Org".to_owned(), "https://org".to_owned())
    .build();
    card.capabilities = AgentCapabilities {
        extensions: Some(vec![AgentExtension::mcp_tools_extension()]),
        ..card.capabilities
    };
    card.security_schemes = Some(
        every_scheme()
            .into_iter()
            .enumerate()
            .map(|(i, (_, scheme))| (format!("scheme{i}"), scheme))
            .collect(),
    );
    card.security_requirements = Some(vec![SecurityRequirement::scheme(
        "scheme2",
        vec!["admin".to_owned()],
    )]);
    card.skills = vec![AgentSkill {
        id: "s".to_owned(),
        name: "Skill".to_owned(),
        description: "does".to_owned(),
        tags: vec!["t".to_owned()],
        examples: Some(vec!["e".to_owned()]),
        input_modes: Some(vec!["text/plain".to_owned()]),
        output_modes: None,
        security_requirements: Some(vec![SecurityRequirement::scheme("scheme0", vec![])]),
    }];

    let value = serde_json::to_value(&card).unwrap();
    assert_conforms("Agent Card", &value);
    assert_eq!(serde_json::from_value::<AgentCard>(value).unwrap(), card);
}

#[test]
fn artifact_and_message_conform_to_schema() {
    let mut artifact = artifact();
    artifact.extensions = vec!["https://ext/v1".to_owned()];
    let value = serde_json::to_value(&artifact).unwrap();
    assert_eq!(value["extensions"], json!(["https://ext/v1"]));
    assert_conforms("Artifact", &value);

    let mut message = message();
    message.metadata = Some(serde_json::Map::from_iter([("k".to_owned(), json!(1))]));
    message
        .parts
        .push(Part::raw("aGk=", "text/plain").with_filename("a.txt"));
    let value = serde_json::to_value(&message).unwrap();
    assert_eq!(value["metadata"], json!({"k": 1}));
    assert_conforms("Message", &value);
}

#[test]
fn stream_events_conform_to_schema() {
    let status = TaskStatusUpdateEvent::new(
        TaskId::new("task-1"),
        ContextId::generate(),
        TaskStatus {
            state: TaskState::InputRequired,
            message: Some(message()),
            timestamp: None,
        },
    );
    let value = serde_json::to_value(StreamResponse::StatusUpdate(status)).unwrap();
    assert_conforms("Stream Response", &value);

    let update =
        TaskArtifactUpdateEvent::new(TaskId::new("task-1"), ContextId::generate(), artifact());
    let value = serde_json::to_value(StreamResponse::ArtifactUpdate(update)).unwrap();
    assert_conforms("Stream Response", &value);
}

#[test]
fn message_rejects_non_object_metadata() {
    let mut value = serde_json::to_value(message()).unwrap();
    value["metadata"] = json!("not-an-object");
    assert!(serde_json::from_value::<Message>(value).is_err());
}

#[test]
fn schema_checker_rejects_unknown_properties() {
    let result = std::panic::catch_unwind(|| {
        assert_conforms(
            "Artifact",
            &json!({"artifactId": "a", "parts": [], "title": "x"}),
        );
    });
    assert!(result.is_err());
}

#[test]
fn task_with_history_and_artifacts_conforms_to_schema() {
    let task = Task {
        history: Some(vec![message()]),
        artifacts: Some(vec![artifact()]),
        ..Task::default()
    };
    assert!(task.created_at.is_some());
    let value = serde_json::to_value(SendMessageResult::Task(Box::new(task))).unwrap();
    assert_conforms("Send Message Response", &value);
}
