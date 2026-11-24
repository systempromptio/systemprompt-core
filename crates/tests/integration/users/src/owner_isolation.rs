use anyhow::Result;
use systemprompt_identifiers::SessionId;
use systemprompt_test_fixtures::{seed_fixture_owners, seed_user_session, test_db_pool};
use systemprompt_users::UserRepository;

async fn sessions_are_read_through_their_owner_only(label: &str) -> Result<()> {
    let db = test_db_pool().await;
    let owners = seed_fixture_owners(&db, label).await?;
    let repo = UserRepository::new(&db);

    let mut written = vec![owners.user.session_id.clone()];
    for _ in 0..3 {
        let session_id = SessionId::generate();
        seed_user_session(&db, &owners.user.user_id, &session_id).await?;
        written.push(session_id);
    }

    let mut read: Vec<SessionId> = repo
        .list_sessions(&owners.user.user_id)
        .await?
        .into_iter()
        .map(|s| {
            assert_eq!(s.user_id.as_ref(), Some(&owners.user.user_id));
            s.session_id
        })
        .collect();
    read.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    written.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    assert_eq!(read, written, "the owner reads exactly the sessions it wrote");

    let admin_sessions = repo.list_sessions(&owners.admin.user_id).await?;
    assert_eq!(admin_sessions.len(), 1, "the admin sees only its own session");
    assert_eq!(admin_sessions[0].session_id, owners.admin.session_id);
    Ok(())
}

#[tokio::test]
async fn first_owner_never_sees_another_tests_sessions() -> Result<()> {
    sessions_are_read_through_their_owner_only("isolation-a").await
}

#[tokio::test]
async fn second_owner_never_sees_another_tests_sessions() -> Result<()> {
    sessions_are_read_through_their_owner_only("isolation-b").await
}
