use serde_json::json;
use systemprompt_identifiers::{ArtifactId, ContextId, MessageId, TaskId};
use systemprompt_models::a2a::{
    AgentCard, Artifact, ArtifactMetadata, Message, MessageRole, PROTOCOL_VERSION, Part,
    PartContent, SecurityRequirement, SendMessageResult, StreamResponse, Task,
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
    let value = serde_json::to_value(SendMessageResult::Task(task.clone())).unwrap();
    assert!(value.get("task").is_some());
    assert_eq!(
        serde_json::from_value::<SendMessageResult>(value).unwrap(),
        SendMessageResult::Task(task)
    );

    let value = serde_json::to_value(SendMessageResult::Message(message())).unwrap();
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
