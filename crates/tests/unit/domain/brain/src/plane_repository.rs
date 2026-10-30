use chrono::{Duration, Utc};
use systemprompt_brain::models::{PlaneRow, PlaneSource};
use systemprompt_brain::repository::{
    BrainRepositories, ConfigDocRepository, InterestRepository, KindRepository, PlaneWrite,
    TopicRepository, WorkflowRepository,
};
use systemprompt_identifiers::UserId;
use systemprompt_test_fixtures::DisposableDb;

fn write<'a>(id: &'a str, body: &'a serde_json::Value, source: PlaneSource) -> PlaneWrite<'a> {
    PlaneWrite { id, body, source }
}

fn shape(rows: &[PlaneRow]) -> Vec<(String, serde_json::Value, String)> {
    rows.iter()
        .map(|r| (r.id.clone(), r.body.clone(), r.source.clone()))
        .collect()
}

macro_rules! round_trip {
    ($name:ident, $repo:ident, $field:ident, $one:literal, $two:literal) => {
        #[tokio::test]
        async fn $name() {
            let db = DisposableDb::with_schema(stringify!($name)).await;
            let pool = db.test_pool().await;
            let repos = BrainRepositories::new(&pool);
            let first = serde_json::json!({ "v": 1 });
            let second = serde_json::json!({ "v": 2 });
            let other = serde_json::json!({ "v": 3 });
            let mut conn = repos.write_pool().acquire().await.expect("conn");

            $repo::upsert(&mut conn, &write($one, &first, PlaneSource::File))
                .await
                .expect("insert");
            $repo::upsert(&mut conn, &write($two, &other, PlaneSource::File))
                .await
                .expect("insert other");
            let before = $repo::list(&mut *conn).await.expect("list");
            $repo::upsert(&mut conn, &write($one, &second, PlaneSource::Console))
                .await
                .expect("update");

            let rows = repos.$field.list_all().await.expect("list all");
            assert_eq!(
                shape(&rows),
                vec![
                    ($two.to_owned(), other.clone(), "file".to_owned()),
                    ($one.to_owned(), second.clone(), "console".to_owned()),
                ]
            );
            let stamp = |rows: &[PlaneRow]| {
                rows.iter()
                    .find(|r| r.id == $one)
                    .map(|r| r.updated_at)
                    .expect("row")
            };
            assert!(stamp(&rows) >= stamp(&before));

            assert!($repo::delete(&mut conn, $one).await.expect("delete"));
            assert!(!$repo::delete(&mut conn, $one).await.expect("delete again"));
            assert_eq!(
                shape(&$repo::list(&mut *conn).await.expect("list")),
                vec![($two.to_owned(), other, "file".to_owned())]
            );
            drop(conn);
            pool.write_pool().close().await;
            db.drop_now().await;
        }
    };
}

round_trip!(
    kinds_upsert_update_and_delete,
    KindRepository,
    kinds,
    "b.one",
    "a.two"
);
round_trip!(
    topics_upsert_update_and_delete,
    TopicRepository,
    topics,
    "b.one",
    "a.two"
);
round_trip!(
    workflows_upsert_update_and_delete,
    WorkflowRepository,
    workflows,
    "b.one",
    "a.two"
);
round_trip!(
    config_docs_upsert_update_and_delete,
    ConfigDocRepository,
    config_docs,
    "retrieval",
    "filters"
);

#[tokio::test]
async fn interests_keep_their_owner_and_expiry_and_clear_them_on_update() {
    let db = DisposableDb::with_schema("brain_plane_interests").await;
    let pool = db.test_pool().await;
    let repos = BrainRepositories::new(&pool);
    let owner = UserId::generate();
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $2, $3)")
        .bind(owner.as_str())
        .bind(format!("owner-{}", owner.as_str()))
        .bind(format!("owner-{}@test.invalid", owner.as_str()))
        .execute(repos.write_pool())
        .await
        .expect("user");
    let expires = Utc::now() + Duration::days(7);
    let body = serde_json::json!({ "topics": ["pricing"] });
    let mut conn = repos.write_pool().acquire().await.expect("conn");

    InterestRepository::upsert(
        &mut conn,
        &write("i.mine", &body, PlaneSource::Console),
        Some(&owner),
        Some(expires),
    )
    .await
    .expect("insert");

    let read = || async {
        sqlx::query_as::<_, (Option<String>, Option<chrono::DateTime<Utc>>)>(
            "SELECT owner_user_id, expires_at FROM brain_interests WHERE id = 'i.mine'",
        )
        .fetch_one(repos.write_pool())
        .await
        .expect("interest")
    };
    let (stored_owner, stored_expiry) = read().await;
    assert_eq!(stored_owner.as_deref(), Some(owner.as_str()));
    let stored_expiry = stored_expiry.expect("expiry");
    assert!((stored_expiry - expires).num_milliseconds().abs() < 1);

    InterestRepository::upsert(
        &mut conn,
        &write("i.mine", &body, PlaneSource::File),
        None,
        None,
    )
    .await
    .expect("update");
    assert_eq!(read().await, (None, None));
    assert_eq!(
        shape(&repos.interests.list_all().await.expect("list all")),
        vec![("i.mine".to_owned(), body, "file".to_owned())]
    );

    assert!(
        InterestRepository::delete(&mut conn, "i.mine")
            .await
            .expect("delete")
    );
    assert!(
        InterestRepository::list(&mut *conn)
            .await
            .expect("list")
            .is_empty()
    );
    drop(conn);
    pool.write_pool().close().await;
    db.drop_now().await;
}
