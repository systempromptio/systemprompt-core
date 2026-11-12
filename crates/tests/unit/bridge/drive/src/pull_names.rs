use systemprompt_bridge::drive::DriveError;
use systemprompt_bridge::drive::names::{PathRefusal, case_collision, portable_segment};
use systemprompt_bridge::drive::pull::pull;
use systemprompt_identifiers::{DocumentId, DocumentRevisionId};
use systemprompt_models::ApiPaths;
use systemprompt_models::documents::content_hash;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support::Fixture;

#[test]
fn windows_device_names_and_trailing_dots_are_not_portable() {
    for name in [
        "CON",
        "con.md",
        "Aux.txt",
        "nul",
        "COM1.md",
        "lpt9",
        "prn.tar.gz",
    ] {
        assert_eq!(
            portable_segment(name),
            Err(PathRefusal::ReservedName),
            "{name}"
        );
    }
    for name in ["notes.", "notes ", "dir."] {
        assert_eq!(
            portable_segment(name),
            Err(PathRefusal::TrailingDotOrSpace),
            "{name}"
        );
    }
    for name in ["a<b", "a|b", "what?", "star*", "quote\"", "tab\there"] {
        assert_eq!(
            portable_segment(name),
            Err(PathRefusal::RefusedCharacter),
            "{name}"
        );
    }
    for name in [
        "console.md",
        "COM0",
        "com10",
        "auxiliary",
        "notes.md",
        ".hidden",
        "a b.md",
    ] {
        assert_eq!(portable_segment(name), Ok(()), "{name}");
    }
}

#[test]
fn names_that_fold_together_collide() {
    assert_eq!(
        case_collision("docs/plan.md", ["docs/Plan.md", "other.md"]),
        Err(PathRefusal::CaseCollision {
            existing: "docs/Plan.md".to_owned()
        })
    );
    assert_eq!(
        case_collision("docs/plan.md", ["docs/plan.md", "docs/plans.md"]),
        Ok(())
    );
}

#[tokio::test]
async fn an_unportable_name_is_kept_as_a_conflict_and_the_feed_moves_on() {
    let server = MockServer::start().await;
    let fx = Fixture::new(&server.uri());
    fx.synced_empty();
    let body: &[u8] = b"placed";
    Mock::given(method("GET"))
        .and(path(ApiPaths::BRIDGE_DRIVE_CHANGES))
        .and(query_param("since", "c0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "changes": [
                {
                    "cursor": "c1", "document_id": DocumentId::generate().as_str(), "path": "aux.md",
                    "revision_id": DocumentRevisionId::generate().as_str(),
                    "content_sha256": content_hash(b"device").as_str(), "kind": {"type": "created"},
                },
                {
                    "cursor": "c2", "document_id": DocumentId::generate().as_str(), "path": "ok.md",
                    "revision_id": DocumentRevisionId::generate().as_str(),
                    "content_sha256": content_hash(body).as_str(), "kind": {"type": "created"},
                },
            ],
            "next_cursor": "c9",
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(ApiPaths::bridge_drive_blob(&content_hash(body))))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body.to_vec()))
        .mount(&server)
        .await;

    let mut state = fx.state();
    pull(fx.cycle(), &mut state).await.expect("pull");

    assert!(!fx.root.dir().join("aux.md").exists());
    assert_eq!(fx.read("ok.md"), body);
    let saved = fx.state();
    assert_eq!(saved.cursor.as_deref(), Some("c9"));
    assert_eq!(saved.conflicts.len(), 1);
    assert_eq!(saved.conflicts[0].path, "aux.md");
    assert_eq!(
        saved.conflicts[0].sibling, "aux.md",
        "no local file holds the name"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_path_under_a_link_out_of_the_folder_is_never_written() {
    let server = MockServer::start().await;
    let fx = Fixture::new(&server.uri());
    fx.synced_empty();
    let outside = tempfile::tempdir().expect("outside");
    std::os::unix::fs::symlink(outside.path(), fx.root.dir().join("linked")).expect("link");
    let err = fx
        .root
        .file("linked/escape.md")
        .expect_err("a link is refused");
    assert!(
        matches!(
            err,
            DriveError::Unplaceable {
                refusal: PathRefusal::Symlink,
                ..
            }
        ),
        "{err:?}"
    );
    let body: &[u8] = b"escape";
    Mock::given(method("GET"))
        .and(path(ApiPaths::BRIDGE_DRIVE_CHANGES))
        .and(query_param("since", "c0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "changes": [{
                "cursor": "c1", "document_id": DocumentId::generate().as_str(),
                "path": "linked/escape.md",
                "revision_id": DocumentRevisionId::generate().as_str(),
                "content_sha256": content_hash(body).as_str(), "kind": {"type": "created"},
            }],
            "next_cursor": "c9",
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(ApiPaths::bridge_drive_blob(&content_hash(body))))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body.to_vec()))
        .mount(&server)
        .await;

    let mut state = fx.state();
    pull(fx.cycle(), &mut state).await.expect("pull");

    assert!(!outside.path().join("escape.md").exists());
    assert_eq!(fx.state().conflicts.len(), 1);
}
