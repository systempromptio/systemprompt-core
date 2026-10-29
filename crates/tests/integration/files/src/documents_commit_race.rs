//! The commit path under contention: a commit that loses the head's swap is
//! rebuilt on the new head, the swap itself admits one of two racing
//! transactions, sessionless commits share one node id counter, and an op id
//! is bound to the whole request.

use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::model::Media;
use systemprompt_files::documents::nodes::writer_tag;
use systemprompt_files::documents::repository::{DocumentRepository, HeadMove};
use systemprompt_files::documents::revision::CommitBody;
use systemprompt_files::documents::service::{CommentPolicy, CommitOutcome, CommitRequest};
use systemprompt_identifiers::{DocumentId, DocumentOpId, DocumentRevisionId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::EditOp;
use systemprompt_test_fixtures::test_db_pool;

use crate::documents_proposals::{memo, outline, rewrite, texts};
use crate::documents_service_fixture::{
    GroupSource, Harness, allow, api_author, harness, person, trace, unique,
};

fn request(
    document: &DocumentId,
    base: &DocumentRevisionId,
    ops: Vec<EditOp>,
    message: Option<&str>,
    op_id: Option<&DocumentOpId>,
) -> CommitRequest {
    CommitRequest {
        document_id: document.clone(),
        base_revision: Some(base.clone()),
        body: CommitBody::Ops(ops),
        author: api_author(),
        message: message.map(str::to_owned),
        lock_holder_id: None,
        comments: CommentPolicy::default(),
        op_id: op_id.cloned(),
        media: Media::new(),
    }
}

async fn edit(
    h: &Harness,
    who: &AuthenticatedUser,
    document: &DocumentId,
    base: &DocumentRevisionId,
    from: &str,
    to: &str,
) -> CommitOutcome {
    let head = outline(h, who, document, Some(base)).await;
    h.service
        .commit(
            who,
            request(document, base, vec![rewrite(&head, from, to)], None, None),
            &trace(),
        )
        .await
        .expect("commit")
}

#[tokio::test]
async fn commits_that_lose_the_head_swap_are_rebuilt_on_the_new_head() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("race");
    let (document, base) = memo(&h, &owner, None).await;
    let (a, b, c) = tokio::join!(
        edit(&h, &owner, &document, &base, "alpha", "ALPHA"),
        edit(&h, &owner, &document, &base, "beta", "BETA"),
        edit(&h, &owner, &document, &base, "gamma", "GAMMA"),
    );
    let outcomes = [a, b, c];
    assert!(
        outcomes.iter().all(|outcome| outcome.landed().is_some()),
        "every disjoint edit lands"
    );
    let count = |committed: bool| {
        outcomes
            .iter()
            .filter(|outcome| match outcome {
                CommitOutcome::Committed(_) => committed,
                CommitOutcome::Merged(_) => !committed,
                _ => false,
            })
            .count()
    };
    assert_eq!(count(true), 1, "exactly one edit lands on the base");
    assert_eq!(
        count(false),
        2,
        "the edits that landed second and third merged onto the new head"
    );
    let head = outline(&h, &owner, &document, None).await;
    let text = texts(&head);
    for expected in ["ALPHA", "BETA", "GAMMA"] {
        assert!(text.iter().any(|t| t == expected), "{expected} in {text:?}");
    }
}

#[tokio::test]
async fn one_of_two_transactions_racing_the_head_swap_wins() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("cas");
    let (document, first) = memo(&h, &owner, None).await;
    let CommitOutcome::Committed(landed) =
        edit(&h, &owner, &document, &first, "alpha", "ALPHA").await
    else {
        panic!("a commit on the head lands as committed");
    };
    let second = landed.revision.id.clone();
    let swap = HeadMove {
        document: &document,
        expected: Some(&second),
        new: &first,
        lock_holder: None,
    };
    let mut winner = h.repos.documents.begin().await.expect("begin winner");
    assert!(
        DocumentRepository::move_head(&mut winner, swap)
            .await
            .expect("winner swap")
    );
    let mut loser = h.repos.documents.begin().await.expect("begin loser");
    let racing = tokio::spawn({
        let document = document.clone();
        let first = first.clone();
        let second = second.clone();
        async move {
            let moved = DocumentRepository::move_head(
                &mut loser,
                HeadMove {
                    document: &document,
                    expected: Some(&second),
                    new: &first,
                    lock_holder: None,
                },
            )
            .await
            .expect("loser swap");
            drop(loser);
            moved
        }
    });
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    winner.commit().await.expect("commit winner");
    assert!(
        !racing.await.expect("join"),
        "the second swap sees the moved head"
    );
}

#[tokio::test]
async fn sessionless_commits_share_one_node_id_counter() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("counter");
    let (document, mut base) = memo(&h, &owner, None).await;
    for (from, to) in [("alpha", "one"), ("beta", "two"), ("gamma", "three")] {
        let head = outline(&h, &owner, &document, Some(&base)).await;
        let mut ops = vec![rewrite(&head, from, to)];
        ops.push(EditOp::AppendMarkdown {
            markdown: unique("added"),
        });
        let outcome = h
            .service
            .commit(&owner, request(&document, &base, ops, None, None), &trace())
            .await
            .expect("commit");
        base = outcome.landed().expect("landed").revision.id.clone();
    }
    let pool = test_db_pool().await.pool();
    let writers: Vec<String> = sqlx::query_scalar(
        "SELECT writer FROM document_node_counters WHERE document_id = $1::uuid",
    )
    .bind(document.as_str())
    .fetch_all(&*pool)
    .await
    .expect("counters");
    assert_eq!(writers, vec![writer_tag(owner.id.as_str())]);
}

#[tokio::test]
async fn an_op_id_reused_with_another_message_is_refused() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("digest");
    let (document, base) = memo(&h, &owner, None).await;
    let head = outline(&h, &owner, &document, Some(&base)).await;
    let ops = vec![rewrite(&head, "beta", "BETA")];
    let op_id = DocumentOpId::new(unique("op"));
    h.service
        .commit(
            &owner,
            request(&document, &base, ops.clone(), Some("first"), Some(&op_id)),
            &trace(),
        )
        .await
        .expect("commit");
    let replay = h
        .service
        .commit(
            &owner,
            request(&document, &base, ops.clone(), Some("first"), Some(&op_id)),
            &trace(),
        )
        .await
        .expect("replay");
    assert!(replay.landed().is_some_and(|landed| landed.replayed));
    let refused = h
        .service
        .commit(
            &owner,
            request(&document, &base, ops, Some("second"), Some(&op_id)),
            &trace(),
        )
        .await;
    assert!(matches!(refused, Err(DocumentsError::OpIdMismatch { .. })));
}
