//! Admitting a prompt's attachments: what is stored for a file, a directory
//! and a data URL, what is refused before anything is stored, and prompts for a
//! session that does not exist.

use std::path::PathBuf;

use serde_json::json;
use systemprompt_engine::run::{Admitted, AttachmentInput, Delivery, EngineError, ErrorKind};
use systemprompt_engine::store::{MessageWithParts, PartBody, SessionId};

use super::vision::Vision;
use super::{HEIC, PNG, data_url, image_block, text_block};
use crate::support::{Scripted, settle, text_turn};

/// The file parts of the first stored message, as (type, name, address).
fn files(messages: &[MessageWithParts]) -> Vec<(String, Option<String>, String)> {
    messages
        .first()
        .map(|message| {
            message
                .parts
                .iter()
                .filter_map(|part| match &part.body {
                    PartBody::File { mime, name, url } => {
                        Some((mime.clone(), name.clone(), url.clone()))
                    },
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

async fn attach(
    scripted: &Scripted,
    id: &SessionId,
    attachment: AttachmentInput,
) -> Result<Admitted, EngineError> {
    let mut input = scripted.prompt(id, "Inspect this", Delivery::Steer);
    input.attachments = vec![attachment];
    scripted.engine.prompt(input).await
}

/// Attaches one thing, waits for the run and returns what was stored for it.
async fn stored(
    scripted: &Scripted,
    id: &SessionId,
    attachment: AttachmentInput,
    expectation: &str,
) -> Vec<(String, Option<String>, String)> {
    let mut events = scripted.engine.subscribe();
    attach(scripted, id, attachment).await.expect(expectation);
    settle(&mut events, id).await;
    files(&scripted.messages(id).await)
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: attaching a directory is refused as an attachment that cannot be read"]
async fn an_attached_directory_reaches_the_model_as_a_listing_of_its_entries() {
    let scripted = Scripted::start(vec![text_turn("ok")]).await;
    let source = scripted.project.join("source");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::write(source.join("index.rs"), "fn main() {}\n").unwrap();
    let id = scripted.session().await;

    let directory = AttachmentInput::Path(PathBuf::from("source"));
    let parts = stored(&scripted, &id, directory, "a directory can be attached").await;
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].1.as_deref(), Some("source"));
    let sent = scripted.model_bodies()[0]["messages"].to_string();
    assert!(sent.contains("index.rs"), "{sent}");
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn images_given_as_data_urls_are_stored_and_sent_exactly_as_given() {
    let cases = [
        ("image/png", PNG, Some("screenshot.png")),
        ("image/heic", HEIC, None),
    ];
    let vision = Vision::start(vec![text_turn("ok")]).await;
    let id = vision.session().await;
    let attachments = cases
        .iter()
        .map(|(mime, data, name)| data_url(mime, data, *name))
        .collect();

    let messages = vision.ask(&id, "Inspect these", attachments).await;
    assert!(!vision.project.join("screenshot.png").exists());
    let expected: Vec<(String, Option<String>, String)> = cases
        .iter()
        .map(|(mime, data, name)| {
            (
                (*mime).to_owned(),
                name.map(str::to_owned),
                format!("data:{mime};base64,{data}"),
            )
        })
        .collect();
    assert_eq!(files(&vision.engine.messages(&id).await.unwrap()), expected);
    assert_eq!(
        messages[0]["content"],
        json!([
            text_block("Inspect these"),
            image_block("image/png", PNG),
            image_block("image/heic", HEIC)
        ])
    );
    vision.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: an image attachment is stored and sent at its original size; nothing scales it down"]
async fn a_large_image_is_scaled_down_before_it_is_stored() {
    let scripted = Scripted::start(vec![text_turn("ok")]).await;
    let id = scripted.session().await;
    let padding = "AAAA".repeat(1_440_000);
    let given = format!("data:image/png;base64,{PNG}{padding}");
    let length = given.len();
    let image = AttachmentInput::DataUrl {
        url: given,
        name: None,
    };

    let parts = stored(&scripted, &id, image, "a large image can be attached").await;
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].0, "image/png");
    assert!(parts[0].2.len() < length, "{} bytes kept", parts[0].2.len());
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: an inline attachment of any size is admitted; there is no size limit"]
async fn an_inline_attachment_over_the_size_limit_is_refused_without_repeating_its_bytes() {
    let scripted = Scripted::start(vec![text_turn("ok")]).await;
    let id = scripted.session().await;
    let over_twenty_mebibytes = "AAAA".repeat(6_990_507);
    let oversized = AttachmentInput::DataUrl {
        url: format!("data:application/octet-stream;base64,{over_twenty_mebibytes}"),
        name: None,
    };

    let error = attach(&scripted, &id, oversized)
        .await
        .expect_err("an oversized attachment is refused");
    assert_eq!(error.kind(), ErrorKind::InvalidInput);
    let shown = format!("{error} {error:?}");
    assert!(shown.len() < 1_024, "{} bytes of error text", shown.len());
    assert!(scripted.messages(&id).await.is_empty());
    assert!(scripted.model_bodies().is_empty());
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_prompt_or_a_compaction_for_a_session_that_does_not_exist_is_not_found() {
    let scripted = Scripted::start(Vec::new()).await;
    let id = scripted.session().await;
    assert!(scripted.engine.delete(&id).await.unwrap());

    let prompt = scripted
        .engine
        .prompt(scripted.prompt(&id, "Anyone there?", Delivery::Steer))
        .await
        .expect_err("the session is gone");
    assert_eq!(prompt.kind(), ErrorKind::NotFound);
    let compaction = scripted
        .engine
        .compact(&id)
        .await
        .expect_err("the session is gone");
    assert_eq!(compaction.kind(), ErrorKind::NotFound);
    assert!(scripted.model_bodies().is_empty());
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: an attached file is typed by its extension alone, so image bytes under another extension are stored as application/octet-stream"]
async fn an_attached_file_is_typed_by_its_content_when_its_name_does_not_say() {
    let scripted = Scripted::start(vec![text_turn("ok")]).await;
    let signature = b"\x89PNG\r\n\x1a\n\0\0\0\0";
    std::fs::write(scripted.project.join("capture.bin"), signature).unwrap();
    let id = scripted.session().await;

    let capture = AttachmentInput::Path(PathBuf::from("capture.bin"));
    let parts = stored(&scripted, &id, capture, "a binary file can be attached").await;
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].0, "image/png");
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: data with no stated type and no recognisable content is admitted as application/octet-stream"]
async fn untyped_data_whose_type_cannot_be_told_is_refused() {
    let scripted = Scripted::start(vec![text_turn("ok")]).await;
    let id = scripted.session().await;
    let untyped = AttachmentInput::DataUrl {
        url: "data:;base64,AQID".to_owned(),
        name: None,
    };

    let error = attach(&scripted, &id, untyped)
        .await
        .expect_err("data of no known type is refused");
    assert_eq!(error.kind(), ErrorKind::InvalidInput);
    assert!(scripted.messages(&id).await.is_empty());
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: the data URL scheme is matched in lower case only, so an upper-case scheme is refused"]
async fn a_data_url_is_recognised_whatever_the_case_of_its_scheme() {
    let scripted = Scripted::start(vec![text_turn("ok")]).await;
    let id = scripted.session().await;
    let shouted = AttachmentInput::DataUrl {
        url: format!("DATA:image/png;base64,{PNG}"),
        name: None,
    };

    let parts = stored(&scripted, &id, shouted, "the scheme is matched in any case").await;
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].0, "image/png");
    scripted.finish().await;
}
