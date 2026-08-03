// Tests for `StreamProcessor::extract_message_content`: which A2A parts reach
// the AI as content, how each supported media class is mapped, and which
// malformed file parts are dropped rather than propagated.

use systemprompt_agent::models::a2a::{
    DataPart, FileContent, FilePart, Message, MessageRole, Part, TextPart,
};
use systemprompt_agent::services::a2a_server::processing::message::StreamProcessor;
use systemprompt_identifiers::{ContextId, MessageId};
use systemprompt_models::AiContentPart;

use base64::Engine as _;

fn message(parts: Vec<Part>) -> Message {
    Message {
        role: MessageRole::User,
        parts,
        message_id: MessageId::generate(),
        task_id: None,
        context_id: ContextId::generate(),
        metadata: None,
        extensions: None,
        reference_task_ids: None,
    }
}

fn text(t: &str) -> Part {
    Part::Text(TextPart { text: t.to_owned() })
}

fn file(name: Option<&str>, mime: Option<&str>, bytes: Option<&str>) -> Part {
    Part::File(FilePart {
        file: FileContent {
            name: name.map(ToOwned::to_owned),
            mime_type: mime.map(ToOwned::to_owned),
            bytes: bytes.map(ToOwned::to_owned),
            url: None,
        },
    })
}

fn b64(s: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(s)
}

#[test]
fn first_text_part_becomes_the_prompt_and_every_text_part_becomes_content() {
    let (prompt, parts) =
        StreamProcessor::extract_message_content(&message(vec![text("first"), text("second")]));

    assert_eq!(prompt, "first", "the prompt is the first text part only");
    assert_eq!(
        parts,
        vec![AiContentPart::text("first"), AiContentPart::text("second")]
    );
}

#[test]
fn an_empty_leading_text_part_lets_a_later_one_supply_the_prompt() {
    let (prompt, parts) =
        StreamProcessor::extract_message_content(&message(vec![text(""), text("real prompt")]));

    assert_eq!(prompt, "real prompt");
    assert_eq!(parts.len(), 2);
}

#[test]
fn a_message_with_no_parts_yields_no_prompt_and_no_content() {
    let (prompt, parts) = StreamProcessor::extract_message_content(&message(Vec::new()));

    assert!(prompt.is_empty());
    assert!(parts.is_empty());
}

#[test]
fn data_parts_are_dropped_entirely() {
    let (prompt, parts) = StreamProcessor::extract_message_content(&message(vec![
        Part::Data(DataPart {
            data: serde_json::Map::new(),
        }),
        text("only text survives"),
    ]));

    assert_eq!(prompt, "only text survives");
    assert_eq!(parts, vec![AiContentPart::text("only text survives")]);
}

#[test]
fn image_audio_and_video_files_map_to_their_own_content_kinds() {
    let (_prompt, parts) = StreamProcessor::extract_message_content(&message(vec![
        file(Some("a.png"), Some("image/png"), Some("IMGDATA")),
        file(Some("a.wav"), Some("audio/wav"), Some("AUDDATA")),
        file(Some("a.mp4"), Some("video/mp4"), Some("VIDDATA")),
    ]));

    assert_eq!(
        parts,
        vec![
            AiContentPart::image("image/png", "IMGDATA"),
            AiContentPart::audio("audio/wav", "AUDDATA"),
            AiContentPart::video("video/mp4", "VIDDATA"),
        ]
    );
}

#[test]
fn a_mime_type_with_a_charset_suffix_still_matches_its_media_class() {
    let (_prompt, parts) = StreamProcessor::extract_message_content(&message(vec![file(
        Some("a.png"),
        Some("image/png; charset=binary"),
        Some("IMGDATA"),
    )]));

    assert_eq!(
        parts,
        vec![AiContentPart::image("image/png; charset=binary", "IMGDATA")]
    );
}

#[test]
fn a_text_file_is_base64_decoded_and_labelled_with_its_name_and_mime() {
    let (_prompt, parts) = StreamProcessor::extract_message_content(&message(vec![file(
        Some("notes.md"),
        Some("text/markdown"),
        Some(&b64("# Heading")),
    )]));

    assert_eq!(
        parts,
        vec![AiContentPart::text(
            "[File: notes.md (text/markdown)]\n# Heading"
        )]
    );
}

#[test]
fn an_unnamed_text_file_is_labelled_unnamed() {
    let (_prompt, parts) = StreamProcessor::extract_message_content(&message(vec![file(
        None,
        Some("text/plain"),
        Some(&b64("body")),
    )]));

    assert_eq!(
        parts,
        vec![AiContentPart::text("[File: unnamed (text/plain)]\nbody")]
    );
}

#[test]
fn a_text_file_with_invalid_base64_is_dropped() {
    let (_prompt, parts) = StreamProcessor::extract_message_content(&message(vec![
        file(
            Some("bad.txt"),
            Some("text/plain"),
            Some("!!!not base64!!!"),
        ),
        text("survivor"),
    ]));

    assert_eq!(parts, vec![AiContentPart::text("survivor")]);
}

#[test]
fn a_text_file_whose_bytes_are_not_utf8_is_dropped() {
    let invalid_utf8 = base64::engine::general_purpose::STANDARD.encode([0xffu8, 0xfe, 0xfd]);
    let (_prompt, parts) = StreamProcessor::extract_message_content(&message(vec![
        file(Some("bin.txt"), Some("text/plain"), Some(&invalid_utf8)),
        text("survivor"),
    ]));

    assert_eq!(parts, vec![AiContentPart::text("survivor")]);
}

#[test]
fn a_file_with_an_unsupported_mime_type_is_dropped() {
    let (_prompt, parts) = StreamProcessor::extract_message_content(&message(vec![file(
        Some("doc.pdf"),
        Some("application/pdf"),
        Some(&b64("%PDF")),
    )]));

    assert!(parts.is_empty(), "unsupported types never reach the AI");
}

#[test]
fn a_file_without_a_mime_type_or_without_bytes_is_dropped() {
    let (_prompt, parts) = StreamProcessor::extract_message_content(&message(vec![
        file(Some("a.png"), None, Some("IMGDATA")),
        file(Some("a.png"), Some("image/png"), None),
    ]));

    assert!(parts.is_empty());
}

#[test]
fn text_and_file_parts_are_emitted_in_source_order() {
    let (prompt, parts) = StreamProcessor::extract_message_content(&message(vec![
        text("describe this"),
        file(Some("a.png"), Some("image/png"), Some("IMGDATA")),
        text("please"),
    ]));

    assert_eq!(prompt, "describe this");
    assert_eq!(
        parts,
        vec![
            AiContentPart::text("describe this"),
            AiContentPart::image("image/png", "IMGDATA"),
            AiContentPart::text("please"),
        ]
    );
}
