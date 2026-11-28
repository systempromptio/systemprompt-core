//! Telling a file's media type from its first bytes, observed through the read
//! tool: a file it recognises comes back as an attachment of that type.

use serde_json::json;
use systemprompt_engine::store::ToolState;
use systemprompt_scripted_instance::{Respond, StopReason, ToolCall, Turn};

use super::ask;
use crate::support::{Scripted, text_turn, tool_states};

/// Writes each file into the project, has the model read them all in one
/// step, and returns the media type each read attached (none for a file that
/// came back as text or was refused).
async fn read_types(files: &[(&str, Vec<u8>)]) -> Vec<Option<String>> {
    let calls = files
        .iter()
        .enumerate()
        .map(|(index, (name, _))| {
            Respond::ToolCall(ToolCall {
                id: format!("read_{index}"),
                name: "read".to_owned(),
                input: json!({"path": name}),
                argument_chunks: Vec::new(),
            })
        })
        .collect();
    let reads = Turn {
        respond: calls,
        stop: Some(StopReason::ToolUse),
        ..Turn::default()
    };
    let scripted = Scripted::start(vec![reads, text_turn("done")]).await;
    for (name, bytes) in files {
        std::fs::write(scripted.project.join(name), bytes).unwrap();
    }
    let id = scripted.session().await;
    ask(&scripted, &id, "Read every file").await;
    let types = tool_states(&scripted.messages(&id).await)
        .into_iter()
        .map(|state| match state {
            ToolState::Completed { attachments, .. } => {
                attachments.into_iter().next().map(|file| file.mime)
            },
            _ => None,
        })
        .collect();
    scripted.finish().await;
    types
}

fn typed(types: &[Option<String>]) -> Vec<Option<&str>> {
    types.iter().map(Option::as_deref).collect()
}

/// The opening box of an ISO base media file with the given major brand and
/// one further compatible brand.
fn container(major: &[u8; 4], compatible: &[u8; 4]) -> Vec<u8> {
    let mut bytes = vec![0, 0, 0, 0x18];
    bytes.extend_from_slice(b"ftyp");
    bytes.extend_from_slice(major);
    bytes.extend_from_slice(&[0; 4]);
    bytes.extend_from_slice(compatible);
    bytes.extend_from_slice(major);
    bytes
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: images and PDFs are told from their first bytes, audio is not"]
async fn images_documents_and_audio_are_told_from_their_first_bytes() {
    let files = [
        ("a.dat", b"\x89PNG\r\n\x1a\n\0\0\0\0".to_vec()),
        ("b.dat", vec![0xFF, 0xD8, 0xFF, 0xE0, 0, 0x10]),
        ("c.dat", b"GIF89a\x01\0\x01\0".to_vec()),
        ("d.dat", b"RIFF\0\0\0\0WEBPVP8 ".to_vec()),
        ("e.dat", b"%PDF-1.7\n".to_vec()),
        ("f.dat", vec![1, 2, 3]),
        ("g.dat", b"ID3\x04\0\0\0\0".to_vec()),
        ("h.dat", vec![0xFF, 0xFB, 0x90, 0x64]),
        ("i.dat", vec![0xFF, 0xF1, 0x50, 0x80]),
        ("j.dat", b"RIFF\0\0\0\0WAVEfmt ".to_vec()),
        ("k.dat", b"OggS\0\x02\0\0".to_vec()),
        ("l.dat", b"fLaC\0\0\0\x22".to_vec()),
    ];
    let types = read_types(&files).await;
    assert_eq!(
        typed(&types),
        [
            Some("image/png"),
            Some("image/jpeg"),
            Some("image/gif"),
            Some("image/webp"),
            Some("application/pdf"),
            None,
            Some("audio/mpeg"),
            Some("audio/mpeg"),
            Some("audio/aac"),
            Some("audio/wav"),
            Some("audio/ogg"),
            Some("audio/flac"),
        ]
    );
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: media containers are not told apart by their brand; none is recognised"]
async fn media_containers_are_told_apart_by_their_brand() {
    let files = [
        ("a.dat", container(b"avif", b"mif1")),
        ("b.dat", container(b"avis", b"msf1")),
        ("c.dat", container(b"heic", b"mif1")),
        ("d.dat", container(b"mif1", b"avif")),
        ("e.dat", container(b"mif1", b"heic")),
        ("f.dat", container(b"isom", b"mp41")),
        ("g.dat", container(b"qt  ", b"wide")),
        ("h.dat", container(b"M4A ", b"isom")),
    ];
    let types = read_types(&files).await;
    assert_eq!(
        typed(&types),
        [
            Some("image/avif"),
            Some("image/avif"),
            Some("image/heic"),
            Some("image/avif"),
            Some("image/heic"),
            Some("video/mp4"),
            Some("video/quicktime"),
            Some("audio/mp4"),
        ]
    );
}
