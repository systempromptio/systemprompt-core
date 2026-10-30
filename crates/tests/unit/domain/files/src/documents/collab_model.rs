use loro::{ExportMode, LoroDoc, VersionVector};
use systemprompt_files::documents::collab::model::{body_of, is_collab_format, splice_body};
use systemprompt_files::documents::engine::loro::{BODY_CONTAINER, body_text, seed};
use systemprompt_provider_contracts::documents::DocumentFormat;

const PAGE: &str = "<!doctype html>\n<html><head><title>Plan</title></head>\n<BODY class=\"x\"><p>One</p></BODY>\n</html>\n";

fn replica_of(server: &LoroDoc) -> LoroDoc {
    let client = LoroDoc::new();
    client
        .import(&server.export(ExportMode::Snapshot).expect("snapshot"))
        .expect("import snapshot");
    client
}

#[test]
fn two_seeds_of_one_body_accept_each_others_updates() {
    let a = seed("hello").expect("seed a");
    let b = seed("hello").expect("seed b");
    let before = a.oplog_vv();
    a.get_text(BODY_CONTAINER)
        .insert(5, " world")
        .expect("insert");
    a.commit();
    let update = a.export(ExportMode::updates(&before)).expect("update");
    let status = b.import(&update).expect("import");
    assert!(status.pending.is_none());
    assert_eq!(body_text(&b), "hello world");
    assert_eq!(a.oplog_vv(), b.oplog_vv());
}

#[test]
fn a_joined_replica_and_the_server_converge_on_concurrent_edits() {
    let server = seed("abc").expect("seed");
    let alice = replica_of(&server);
    let bob = replica_of(&server);
    let joined_vv = server.oplog_vv();

    alice
        .get_text(BODY_CONTAINER)
        .insert(0, "A")
        .expect("alice");
    alice.commit();
    bob.get_text(BODY_CONTAINER).insert(3, "B").expect("bob");
    bob.commit();
    let from_alice = alice.export(ExportMode::updates(&joined_vv)).expect("a");
    let from_bob = bob.export(ExportMode::updates(&joined_vv)).expect("b");

    for doc in [&server, &alice, &bob] {
        doc.import(&from_alice).expect("alice update");
        doc.import(&from_bob).expect("bob update");
    }
    assert_eq!(body_text(&server), "AabcB");
    assert_eq!(body_text(&alice), body_text(&server));
    assert_eq!(body_text(&bob), body_text(&server));
    assert_eq!(alice.oplog_vv(), server.oplog_vv());
}

#[test]
fn version_vectors_round_trip_and_report_what_a_client_lacks() {
    let server = seed("x").expect("seed");
    let client = replica_of(&server);
    let client_vv = VersionVector::decode(&client.oplog_vv().encode()).expect("decode");
    assert!(client_vv.includes_vv(&server.oplog_vv()));
    server
        .get_text(BODY_CONTAINER)
        .insert(1, "y")
        .expect("insert");
    server.commit();
    assert!(!client_vv.includes_vv(&server.oplog_vv()));
    let missing = server
        .export(ExportMode::updates(&client_vv))
        .expect("missing");
    client.import(&missing).expect("import missing");
    assert_eq!(body_text(&client), "xy");
}

#[test]
fn a_garbage_update_is_refused() {
    let server = seed("x").expect("seed");
    assert!(server.import(b"not a loro update").is_err());
}

#[test]
fn html_body_is_the_inner_markup_of_body() {
    assert_eq!(body_of(DocumentFormat::Html, PAGE), "<p>One</p>");
}

#[test]
fn html_materialises_inside_the_base_document() {
    let out = splice_body(DocumentFormat::Html, PAGE, "<p>Two</p>");
    assert_eq!(
        out,
        "<!doctype html>\n<html><head><title>Plan</title></head>\n<BODY class=\"x\"><p>Two</p></BODY>\n</html>\n"
    );
}

#[test]
fn html_without_body_is_edited_whole() {
    let fragment = "<p>bare</p>";
    assert_eq!(body_of(DocumentFormat::Html, fragment), fragment);
    assert_eq!(
        splice_body(DocumentFormat::Html, fragment, "<p>new</p>"),
        "<p>new</p>"
    );
}

#[test]
fn bodyguard_tag_is_not_mistaken_for_body() {
    let html = "<html><bodyguard>x</bodyguard><body>in</body></html>";
    assert_eq!(body_of(DocumentFormat::Html, html), "in");
}

#[test]
fn markdown_is_edited_whole() {
    let md = "# Plan\n\n- one\n";
    assert_eq!(body_of(DocumentFormat::Markdown, md), md);
    assert_eq!(
        splice_body(DocumentFormat::Markdown, md, "# New\n"),
        "# New\n"
    );
}

#[test]
fn markup_and_office_text_formats_are_collab_formats() {
    for format in [
        DocumentFormat::Html,
        DocumentFormat::Markdown,
        DocumentFormat::Docx,
        DocumentFormat::Odt,
    ] {
        assert!(is_collab_format(format), "{format:?}");
    }
    for format in [
        DocumentFormat::Xlsx,
        DocumentFormat::Pptx,
        DocumentFormat::Text,
    ] {
        assert!(!is_collab_format(format));
    }
}
