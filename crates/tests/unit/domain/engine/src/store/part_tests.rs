use serde_json::json;
use systemprompt_engine::store::{
    CallId, Change, JsonMap, Part, PartBody, PartId, Store, StoreError, ToolState,
};

use crate::store::support::{TestResult, empty_reply, session};

#[test]
fn updating_a_part_keeps_its_place() -> TestResult {
    let store = Store::open_in_memory()?;
    let created = session(&store)?;
    let message_id = empty_reply(&store, &created.id)?;

    let mut tool = Part::new(
        store.ids(),
        created.id.clone(),
        message_id.clone(),
        PartBody::Tool {
            call_id: CallId::try_new("call-1")?,
            tool: "read".into(),
            state: Box::new(ToolState::Pending { input: json!({}) }),
        },
    );
    store.upsert_part(&tool)?;
    let text = Part::new(
        store.ids(),
        created.id,
        message_id.clone(),
        PartBody::Text {
            text: "after".into(),
            synthetic: false,
        },
    );
    store.upsert_part(&text)?;

    tool.body = PartBody::Tool {
        call_id: CallId::try_new("call-1")?,
        tool: "read".into(),
        state: Box::new(ToolState::Completed {
            input: json!({"path": "a.rs"}),
            output: "contents".into(),
            title: "a.rs".into(),
            metadata: JsonMap::new(),
            attachments: Vec::new(),
            started: 1,
            ended: 2,
        }),
    };
    let stored = store.upsert_part(&tool)?;
    assert_eq!(stored.created_at, tool.created_at);

    let parts = store.parts(&message_id)?;
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].id, tool.id);
    let PartBody::Tool { state, .. } = &parts[0].body else {
        return Err("first part is not the tool".into());
    };
    assert!(state.is_finished());
    assert_eq!(parts[1].id, text.id);
    Ok(())
}

#[test]
fn text_deltas_append_and_are_published() -> TestResult {
    let store = Store::open_in_memory()?;
    let created = session(&store)?;
    let message_id = empty_reply(&store, &created.id)?;
    let part = store.upsert_part(&Part::new(
        store.ids(),
        created.id.clone(),
        message_id.clone(),
        PartBody::Text {
            text: String::new(),
            synthetic: false,
        },
    ))?;
    let reasoning = store.upsert_part(&Part::new(
        store.ids(),
        created.id.clone(),
        message_id.clone(),
        PartBody::Reasoning {
            text: "think".into(),
            provider_data: Some(json!({"signature": "sig"})),
        },
    ))?;

    let mut feed = store.subscribe();
    store.append_text(&part.id, "Hel")?;
    store.append_text(&part.id, "lo \"quoted\" ")?;
    store.append_text(&part.id, "wörld")?;
    store.append_text(&reasoning.id, "ing")?;

    let stored = store.get_part(&part.id)?.ok_or("part missing")?;
    assert_eq!(stored.body.text(), Some("Hello \"quoted\" wörld"));
    let stored = store.get_part(&reasoning.id)?.ok_or("reasoning missing")?;
    let PartBody::Reasoning {
        text,
        provider_data,
    } = stored.body
    else {
        return Err("not reasoning".into());
    };
    assert_eq!(text, "thinking");
    assert_eq!(provider_data, Some(json!({"signature": "sig"})));

    let Change::PartDelta {
        part_id,
        delta,
        message_id: delta_message,
        ..
    } = feed.try_recv()?
    else {
        return Err("expected a delta".into());
    };
    assert_eq!(part_id, part.id);
    assert_eq!(delta, "Hel");
    assert_eq!(delta_message, message_id);

    let step = store.upsert_part(&Part::new(
        store.ids(),
        created.id,
        message_id,
        PartBody::Compaction { auto: true },
    ))?;
    assert!(matches!(
        store.append_text(&step.id, "x"),
        Err(StoreError::NotText { .. })
    ));
    assert!(matches!(
        store.append_text(&PartId::mint(store.ids()), "x"),
        Err(StoreError::NotFound { .. })
    ));
    Ok(())
}

#[test]
fn a_failed_tool_keeps_its_metadata_and_older_rows_decode_without_it() -> TestResult {
    let store = Store::open_in_memory()?;
    let created = session(&store)?;
    let message_id = empty_reply(&store, &created.id)?;
    let mut metadata = JsonMap::new();
    metadata.insert("refused_by".into(), json!("policy"));
    let failed = store.upsert_part(&Part::new(
        store.ids(),
        created.id,
        message_id,
        PartBody::Tool {
            call_id: CallId::try_new("call-9")?,
            tool: "read".into(),
            state: Box::new(ToolState::Error {
                input: json!({"path": "a.rs"}),
                error: "refused".into(),
                metadata: metadata.clone(),
                started: 1,
                ended: 2,
            }),
        },
    ))?;
    let stored = store.get_part(&failed.id)?.ok_or("part missing")?;
    let PartBody::Tool { state, .. } = stored.body else {
        return Err("not a tool part".into());
    };
    let ToolState::Error { metadata: kept, .. } = *state else {
        return Err("not a failed tool".into());
    };
    assert_eq!(kept, metadata);

    let older: ToolState = serde_json::from_value(
        json!({"status": "error", "input": {}, "error": "boom", "started": 1, "ended": 2}),
    )?;
    let ToolState::Error {
        metadata: absent,
        error,
        ..
    } = older
    else {
        return Err("not a failed tool".into());
    };
    assert_eq!(error, "boom");
    assert!(absent.is_empty());
    Ok(())
}
