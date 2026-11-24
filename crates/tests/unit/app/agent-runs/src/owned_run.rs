//! `AgentRunRepository::find_owned_run` answers a run only to its owner: a
//! foreign id and an unknown id return the same `None`.

use systemprompt_identifiers::{AgentRunId, UserId};

use crate::repository::{fixture, queue};

#[tokio::test]
async fn the_owner_reads_their_run() {
    let f = fixture().await;
    let id = queue(&f).await;
    let row = f
        .repo
        .find_owned_run(&id, &f.user_id)
        .await
        .expect("read")
        .expect("own run");
    assert_eq!(row.id, id);
    assert_eq!(row.user_id, f.user_id);
}

#[tokio::test]
async fn a_foreign_run_answers_exactly_as_an_unknown_one() {
    let owner = fixture().await;
    let stranger = fixture().await;
    let id = queue(&owner).await;

    let foreign = stranger
        .repo
        .find_owned_run(&id, &stranger.user_id)
        .await
        .expect("foreign read");
    let unknown = stranger
        .repo
        .find_owned_run(&AgentRunId::generate(), &stranger.user_id)
        .await
        .expect("unknown read");
    assert!(
        foreign.is_none(),
        "a run is never read through another user"
    );
    assert_eq!(foreign.is_none(), unknown.is_none());

    let absent_user = UserId::new("no-such-user");
    assert!(
        owner
            .repo
            .find_owned_run(&id, &absent_user)
            .await
            .expect("absent owner read")
            .is_none()
    );
    assert!(
        owner.repo.find_run(&id).await.expect("unscoped").is_some(),
        "the worker read still sees the row"
    );
}
