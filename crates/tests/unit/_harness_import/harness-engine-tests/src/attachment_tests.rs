use std::path::PathBuf;
use std::sync::Arc;

use harness_engine::{AttachmentInput, Delivery, ErrorKind};
use harness_store::PartBody;

use crate::support::{
    Scripted, TestGovernor, harness, prompt, text_reply, titled_session, wait_idle,
};

fn files(parts: &[harness_store::Part]) -> Vec<(String, Option<String>, String)> {
    parts
        .iter()
        .filter_map(|part| match &part.body {
            PartBody::File { mime, name, url } => Some((mime.clone(), name.clone(), url.clone())),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn an_image_file_is_stored_as_a_base64_data_url() {
    let transport = Arc::new(Scripted::with(vec![(200, text_reply("A picture."))]));
    let harness = harness(Arc::new(TestGovernor::default()), transport);
    std::fs::write(harness.project.join("a.png"), b"png bytes").unwrap();
    let id = titled_session(&harness).await;
    let mut events = harness.engine.subscribe();

    let mut input = prompt(&harness, &id, "Look at this", Delivery::Steer);
    input.attachments = vec![AttachmentInput::Path(PathBuf::from("a.png"))];
    harness.engine.prompt(input).await.unwrap();
    wait_idle(&mut events, &id).await;

    let messages = harness.engine.messages(&id).await.unwrap();
    let stored = files(&messages[0].parts);
    let expected = (
        "image/png".to_owned(),
        Some("a.png".to_owned()),
        "data:image/png;base64,cG5nIGJ5dGVz".to_owned(),
    );
    assert_eq!(stored, vec![expected]);
    harness.engine.shutdown().await;
}

#[tokio::test]
async fn a_text_file_is_attached_with_its_contents() {
    let transport = Arc::new(Scripted::with(vec![(200, text_reply("Read it."))]));
    let harness = harness(Arc::new(TestGovernor::default()), Arc::clone(&transport));
    let id = titled_session(&harness).await;
    let mut events = harness.engine.subscribe();

    let mut input = prompt(&harness, &id, "", Delivery::Steer);
    input.attachments = vec![AttachmentInput::Path(PathBuf::from("notes.txt"))];
    harness.engine.prompt(input).await.unwrap();
    wait_idle(&mut events, &id).await;

    let messages = harness.engine.messages(&id).await.unwrap();
    let stored = files(&messages[0].parts);
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].0, "text/plain");
    assert!(stored[0].2.starts_with("file:///"), "{}", stored[0].2);
    assert!(messages[0].parts.iter().any(|part| matches!(
        &part.body,
        PartBody::Text { text, synthetic: true } if text.contains("hello from the notes file")
    )));
    assert!(
        transport.requests()[0]
            .to_string()
            .contains("hello from the notes file")
    );
    harness.engine.shutdown().await;
}

#[tokio::test]
async fn an_attachment_that_is_not_a_base64_data_url_is_refused() {
    let transport = Arc::new(Scripted::default());
    let harness = harness(Arc::new(TestGovernor::default()), Arc::clone(&transport));
    let id = titled_session(&harness).await;

    for url in ["https://example.com/a.png", "data:text/plain,hello"] {
        let mut input = prompt(&harness, &id, "Look", Delivery::Steer);
        input.attachments = vec![AttachmentInput::DataUrl {
            url: url.to_owned(),
            name: None,
        }];
        let error = harness.engine.prompt(input).await.unwrap_err();
        assert_eq!(error.kind(), ErrorKind::InvalidInput, "{url}");
    }
    assert_eq!(
        harness.engine.messages(&id).await.unwrap(),
        Vec::<harness_store::MessageWithParts>::new()
    );
    assert_eq!(transport.requests(), Vec::<serde_json::Value>::new());
    harness.engine.shutdown().await;
}
