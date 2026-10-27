use systemprompt_files::documents::revision::{
    CommitBody, CommitPlan, MergeOutcome, three_way, three_way_bytes,
};
use systemprompt_identifiers::DocumentRevisionId;
use systemprompt_provider_contracts::documents::{DocumentFormat, EditOp};

fn rev() -> DocumentRevisionId {
    DocumentRevisionId::generate()
}

fn bytes() -> CommitBody {
    CommitBody::Bytes(b"x".to_vec())
}

fn ops() -> CommitBody {
    CommitBody::Ops(vec![EditOp::SetText {
        text: "x".to_owned(),
    }])
}

#[test]
fn ops_apply_on_the_head_whatever_their_base() {
    let (head, base) = (rev(), rev());
    for format in [
        DocumentFormat::Docx,
        DocumentFormat::Xlsx,
        DocumentFormat::Pptx,
        DocumentFormat::Markdown,
    ] {
        for read_from in [Some(&head), Some(&base), None] {
            let plan = CommitPlan::decide(Some(&head), read_from, format, &ops());
            assert_eq!(plan, CommitPlan::ApplyOnHead);
        }
    }
}

#[test]
fn ops_merge_only_when_the_head_moved_past_their_base() {
    let (head, base) = (rev(), rev());
    let plan = CommitPlan::ApplyOnHead;
    assert!(plan.merges(Some(&head), Some(&base)));
    assert!(!plan.merges(Some(&head), Some(&head)));
    assert!(!plan.merges(Some(&head), None));
}

#[test]
fn bytes_at_the_head_are_stored() {
    let head = rev();
    for format in [DocumentFormat::Docx, DocumentFormat::Markdown] {
        let plan = CommitPlan::decide(Some(&head), Some(&head), format, &bytes());
        assert_eq!(plan, CommitPlan::Store);
        assert!(!plan.merges(Some(&head), Some(&head)));
    }
}

#[test]
fn stale_office_bytes_diff_onto_the_head() {
    let (head, base) = (rev(), rev());
    for format in [
        DocumentFormat::Docx,
        DocumentFormat::Xlsx,
        DocumentFormat::Pptx,
    ] {
        let plan = CommitPlan::decide(Some(&head), Some(&base), format, &bytes());
        assert_eq!(plan, CommitPlan::DiffOnHead);
        assert!(plan.merges(Some(&head), Some(&base)));
    }
}

#[test]
fn stale_text_bytes_merge_three_ways() {
    let (head, base) = (rev(), rev());
    for format in [
        DocumentFormat::Html,
        DocumentFormat::Markdown,
        DocumentFormat::Text,
    ] {
        let plan = CommitPlan::decide(Some(&head), Some(&base), format, &bytes());
        assert_eq!(plan, CommitPlan::MergeBytes);
        assert!(plan.merges(Some(&head), Some(&base)));
    }
}

#[test]
fn bytes_without_a_base_against_a_head_conflict() {
    let head = rev();
    let plan = CommitPlan::decide(Some(&head), None, DocumentFormat::Markdown, &bytes());
    assert_eq!(plan, CommitPlan::Conflict);
    assert!(!plan.merges(Some(&head), None));
}

#[test]
fn a_document_without_a_head_takes_bytes_only() {
    assert_eq!(
        CommitPlan::decide(None, None, DocumentFormat::Text, &bytes()),
        CommitPlan::Store
    );
    assert_eq!(
        CommitPlan::decide(None, None, DocumentFormat::Text, &ops()),
        CommitPlan::NoHead
    );
}

#[test]
fn three_way_merges_disjoint_edits() {
    let merged = three_way("a\nb\nc\n", "A\nb\nc\n", "a\nb\nC\n");
    assert_eq!(merged, MergeOutcome::Clean("A\nb\nC\n".to_owned()));
}

#[test]
fn three_way_reports_overlapping_edits() {
    assert_eq!(three_way("a\n", "b\n", "c\n"), MergeOutcome::Conflicted);
}

#[test]
fn three_way_bytes_refuses_non_utf8() {
    assert_eq!(
        three_way_bytes(b"a\n", &[0xff, 0xfe], b"a\n"),
        MergeOutcome::Conflicted
    );
}
