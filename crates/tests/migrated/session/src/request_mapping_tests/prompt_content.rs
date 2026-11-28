//! What a prompt and its attachments become in the request: the prompt text,
//! images and documents as media blocks, and nothing the user did not send.

use std::path::PathBuf;

use serde_json::{Value, json};
use systemprompt_engine::run::AttachmentInput;

use super::vision::Vision;
use super::{PDF, PIXELS, ask, block_types, data_url, image_block, text_block, user_turn};
use crate::support::{Scripted, text_turn};

// JSON: the recorded turns, counted for their image blocks.
fn images(messages: &Value) -> usize {
    block_types(&messages[0])
        .into_iter()
        .filter(|kind| *kind == "image")
        .count()
}

#[tokio::test(flavor = "multi_thread")]
async fn naming_a_skill_in_a_prompt_does_not_put_the_skills_content_in_the_conversation() {
    let scripted = Scripted::start(vec![text_turn("ok")]).await;
    let skill = scripted.project.join(".harness/skills/deploy");
    std::fs::create_dir_all(&skill).unwrap();
    std::fs::write(
        skill.join("SKILL.md"),
        "---\nname: deploy\ndescription: Ships a release\n---\nWalk the release checklist in order.\n",
    )
    .unwrap();
    let id = scripted.session().await;

    let messages = ask(&scripted, &id, "Use @deploy for this").await;
    assert_eq!(messages, json!([user_turn(&["Use @deploy for this"])]));
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn inline_images_and_pdfs_are_sent_as_media_with_no_location_text() {
    let vision = Vision::start(vec![text_turn("ok")]).await;
    let id = vision.session().await;
    let attachments = vec![
        data_url("image/png", PIXELS, Some("image.png")),
        data_url("application/pdf", PDF, Some("document.pdf")),
    ];

    let messages = vision.ask(&id, "Inspect these", attachments).await;
    assert_eq!(
        messages,
        json!([{"role": "user", "content": [
            text_block("Inspect these"),
            image_block("image/png", PIXELS),
            {
                "type": "document",
                "source": {"type": "base64", "media_type": "application/pdf", "data": PDF},
                "title": "document.pdf"
            }
        ]}])
    );
    vision.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: an attached image file is sent as media alone; its path on disk is not stated"]
async fn an_attached_image_file_is_introduced_by_its_path_before_the_media() {
    let vision = Vision::start(vec![text_turn("ok")]).await;
    std::fs::write(vision.project.join("shot.png"), [0u8, 1, 2, 3]).unwrap();
    let id = vision.session().await;
    let attachments = vec![AttachmentInput::Path(PathBuf::from("shot.png"))];

    let messages = vision.ask(&id, "Inspect this image", attachments).await;
    assert_eq!(block_types(&messages[0]), ["text", "text", "image"]);
    let location = messages[0]["content"][1]["text"]
        .as_str()
        .unwrap_or_default();
    assert!(location.contains("shot.png"), "{location}");
    assert_eq!(messages[0]["content"][2], image_block("image/png", PIXELS));
    vision.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: the same image attached twice under one name is sent twice"]
async fn the_same_image_attached_twice_under_one_name_is_sent_once() {
    let vision = Vision::start(vec![text_turn("ok")]).await;
    let id = vision.session().await;
    let attachments = vec![
        data_url("image/png", PIXELS, Some("image.png")),
        data_url("image/png", PIXELS, Some("image.png")),
    ];

    let messages = vision.ask(&id, "Compare", attachments).await;
    assert_eq!(images(&messages), 1);
    vision.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_same_image_under_different_names_or_from_a_file_is_sent_each_time() {
    let vision = Vision::start(vec![text_turn("ok")]).await;
    std::fs::write(vision.project.join("disk.png"), [0u8, 1, 2, 3]).unwrap();
    let id = vision.session().await;
    let attachments = vec![
        data_url("image/png", PIXELS, Some("one.png")),
        data_url("image/png", PIXELS, Some("two.png")),
        AttachmentInput::Path(PathBuf::from("disk.png")),
    ];

    let messages = vision.ask(&id, "Compare", attachments).await;
    assert_eq!(
        messages[0]["content"],
        json!([
            text_block("Compare"),
            image_block("image/png", PIXELS),
            image_block("image/png", PIXELS),
            image_block("image/png", PIXELS)
        ])
    );
    vision.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: an attachment must be a base64 data URL; a web address is refused as invalid input"]
async fn an_image_given_by_web_address_is_sent_as_a_url_source() {
    let vision = Vision::start(vec![text_turn("ok")]).await;
    let id = vision.session().await;
    let address = "https://example.test/a.png";
    let attachment = AttachmentInput::DataUrl {
        url: address.to_owned(),
        name: Some("a.png".to_owned()),
    };

    let messages = vision.ask(&id, "Describe it", vec![attachment]).await;
    assert_eq!(
        messages[0]["content"][1],
        json!({"type": "image", "source": {"type": "url", "url": address}})
    );
    assert_eq!(images(&messages), 1);
    vision.finish().await;
}
