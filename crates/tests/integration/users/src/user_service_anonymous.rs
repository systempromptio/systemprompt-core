use std::sync::Arc;

use systemprompt_test_fixtures::test_db_pool;
use systemprompt_users::{UserRepository, UserService};

#[tokio::test]
async fn anonymous_users_appear_only_in_the_including_anonymous_reads() {
    let db = test_db_pool().await;
    let service = UserService::new(Arc::new(UserRepository::new(&db)));
    let rendered = format!("{service:?}");
    assert!(rendered.contains("owner_reassignments: []"), "{rendered}");
    let fingerprint = format!("anonread{}", uuid::Uuid::new_v4().simple());
    let anon = service
        .create_anonymous(&fingerprint)
        .await
        .expect("anonymous");

    let plain = service.search(&fingerprint, 10).await.expect("search");
    assert!(plain.iter().all(|user| user.id != anon.id));
    let including = service
        .search_including_anonymous(&fingerprint, 10)
        .await
        .expect("search including");
    assert_eq!(
        including.iter().map(|u| u.id.clone()).collect::<Vec<_>>(),
        vec![anon.id.clone()]
    );

    let listed = service
        .list_including_anonymous(10_000, 0)
        .await
        .expect("list including");
    assert!(listed.iter().any(|user| user.id == anon.id));
    let listed_plain = service.list(10_000, 0).await.expect("list");
    assert!(listed_plain.iter().all(|user| user.id != anon.id));

    let total = service
        .count_including_anonymous()
        .await
        .expect("count including");
    assert!(total >= 1);
    assert!(total >= i64::try_from(listed_plain.len()).expect("len"));

    sqlx::query!("DELETE FROM users WHERE id = $1", anon.id.as_str())
        .execute(db.pool().as_ref())
        .await
        .expect("cleanup");
}
