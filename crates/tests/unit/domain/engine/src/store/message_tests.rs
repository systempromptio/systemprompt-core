use systemprompt_engine::store::{
    AssistantMessage, Change, FinishReason, Message, MessageError, MessageErrorKind, MessagePage,
    PartBody, Store, Usage, now_ms,
};

use crate::store::support::{TestResult, assistant, session, text_of, user};

#[test]
fn messages_and_parts_come_back_in_order_and_page_backwards() -> TestResult {
    let store = Store::open_in_memory()?;
    let created = session(&store)?;
    let mut expected = Vec::new();
    for turn in 0..6 {
        let prompt = format!("prompt {turn}");
        let reply = format!("reply {turn}");
        let user_id = user(&store, &created.id, &prompt)?;
        assistant(&store, &created.id, &user_id, &reply)?;
        expected.push(prompt);
        expected.push(reply);
    }

    let all = store.messages(&created.id, &MessagePage::all())?;
    assert_eq!(all.len(), 12);
    let texts: Vec<&str> = all.iter().flat_map(|entry| text_of(&entry.parts)).collect();
    assert_eq!(
        texts,
        expected.iter().map(String::as_str).collect::<Vec<_>>()
    );
    assert!(matches!(all[1].parts[0].body, PartBody::StepStart { .. }));

    let newest = store.messages(&created.id, &MessagePage::latest(4))?;
    assert_eq!(newest.len(), 4);
    assert_eq!(newest[0].message.id(), all[8].message.id());
    let older = store.messages(
        &created.id,
        &MessagePage::latest(4).before(newest[0].message.id().clone()),
    )?;
    assert_eq!(older.len(), 4);
    assert_eq!(older[3].message.id(), all[7].message.id());
    assert_eq!(older[0].message.id(), all[4].message.id());
    Ok(())
}

#[test]
fn messages_round_trip_with_usage_and_errors() -> TestResult {
    let store = Store::open_in_memory()?;
    let created = session(&store)?;
    let user_id = user(&store, &created.id, "go")?;
    let mut message = AssistantMessage::new(store.ids(), created.id, user_id, "build", "model-a");
    message.usage = Usage {
        input: 10,
        output: 20,
        reasoning: 3,
        cache_read: 4,
        cache_write: 5,
    };
    message.finish = Some(FinishReason::ToolCalls);
    message.error = Some(MessageError {
        kind: MessageErrorKind::Api,
        message: "overloaded".into(),
        status: Some(529),
        retryable: true,
    });
    message.completed_at = Some(now_ms());
    let message = Message::from(message);

    let mut feed = store.subscribe();
    store.upsert_message(&message)?;
    assert_eq!(store.get_message(message.id())?, Some(message.clone()));
    let Change::MessageUpdated(published) = feed.try_recv()? else {
        return Err("expected a message change".into());
    };
    assert_eq!(published, message);
    Ok(())
}
