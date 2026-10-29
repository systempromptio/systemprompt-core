use systemprompt_files::documents::codecs::docx::{NativeComment, with_comments};
use systemprompt_identifiers::DocumentNodeId;

use super::docx_fixture::{entries, fixture, part};

fn comment(node: &str, text: &str) -> NativeComment {
    NativeComment {
        node: DocumentNodeId::new(node),
        author: "ana@example.com".to_owned(),
        date: "2026-10-28T09:00:00Z".to_owned(),
        text: text.to_owned(),
    }
}

#[test]
fn a_comment_becomes_a_native_word_comment_with_its_part_type_and_relationship() {
    let out = with_comments(
        &fixture(),
        &[comment("pid:2B3C4D5E", "Check <this> & that")],
    )
    .expect("export");
    let document = part(&out, "word/document.xml");
    assert!(document.contains("<w:commentRangeStart w:id=\"0\"/>"));
    assert!(document.contains(
        "<w:commentRangeEnd w:id=\"0\"/><w:r><w:commentReference w:id=\"0\"/></w:r></w:p>"
    ));
    let comments = part(&out, "word/comments.xml");
    assert!(comments.contains("<w:comment w:id=\"0\" w:author=\"ana@example.com\""));
    assert!(comments.contains("Check &lt;this&gt; &amp; that"));
    assert!(part(&out, "[Content_Types].xml").contains("PartName=\"/word/comments.xml\""));
    assert!(part(&out, "word/_rels/document.xml.rels").contains("Target=\"comments.xml\""));
}

#[test]
fn a_comment_on_a_node_that_is_not_a_paragraph_leaves_the_package_unchanged() {
    let before = fixture();
    let out = with_comments(&before, &[comment("body/tbl[1]", "table note")]).expect("export");
    assert_eq!(out, before);
    assert!(
        entries(&out)
            .iter()
            .all(|(name, ..)| name != "word/comments.xml")
    );
}
