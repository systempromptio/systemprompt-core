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
fn base_at_head_stores_bytes_and_applies_ops() {
    let head = rev();
    for format in [DocumentFormat::Docx, DocumentFormat::Markdown] {
        assert_eq!(
            CommitPlan::decide(Some(&head), Some(&head), format, &bytes()),
            CommitPlan::Store
        );
        assert_eq!(
            CommitPlan::decide(Some(&head), Some(&head), format, &ops()),
            CommitPlan::Apply
        );
    }
}

#[test]
fn stale_office_commits_conflict() {
    let (head, base) = (rev(), rev());
    for format in [
        DocumentFormat::Docx,
        DocumentFormat::Xlsx,
        DocumentFormat::Pptx,
    ] {
        assert_eq!(
            CommitPlan::decide(Some(&head), Some(&base), format, &bytes()),
            CommitPlan::Conflict
        );
        assert_eq!(
            CommitPlan::decide(Some(&head), Some(&base), format, &ops()),
            CommitPlan::Conflict
        );
    }
}

#[test]
fn stale_text_commits_merge() {
    let (head, base) = (rev(), rev());
    for format in [
        DocumentFormat::Html,
        DocumentFormat::Markdown,
        DocumentFormat::Text,
    ] {
        let plan = CommitPlan::decide(Some(&head), Some(&base), format, &bytes());
        assert_eq!(plan, CommitPlan::MergeBytes);
        assert!(plan.merges());
        assert_eq!(
            CommitPlan::decide(Some(&head), Some(&base), format, &ops()),
            CommitPlan::MergeOps
        );
    }
}

#[test]
fn a_missing_base_against_a_head_conflicts() {
    let head = rev();
    assert_eq!(
        CommitPlan::decide(Some(&head), None, DocumentFormat::Markdown, &bytes()),
        CommitPlan::Conflict
    );
    assert!(!CommitPlan::Conflict.merges());
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
