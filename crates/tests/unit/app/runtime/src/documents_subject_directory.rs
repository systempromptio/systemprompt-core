use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_database::DbPool;
use systemprompt_extension::notifications::{
    Membership, NotificationAudienceError, NotificationAudienceResolver,
};
use systemprompt_files::documents::SubjectDirectory;
use systemprompt_identifiers::UserId;
use systemprompt_models::notifications::AudienceSubject;
use systemprompt_runtime::documents::{ExtensionAudiences, UserSubjectDirectory};
use systemprompt_users::UserRepository;
use uuid::Uuid;

struct Team;

#[async_trait]
impl NotificationAudienceResolver for Team {
    fn name(&self) -> &'static str {
        "test-team"
    }

    fn handles(&self, subject: &AudienceSubject) -> bool {
        matches!(subject, AudienceSubject::Dimension { kind, .. } if kind == "team")
    }

    async fn members(
        &self,
        subject: &AudienceSubject,
        limit: usize,
    ) -> Result<Vec<UserId>, NotificationAudienceError> {
        match subject {
            AudienceSubject::Dimension { value, .. } => Ok((0..limit.min(2))
                .map(|i| UserId::new(format!("{value}-member-{i}")))
                .collect()),
            _ => Ok(Vec::new()),
        }
    }

    async fn is_member(
        &self,
        _subject: &AudienceSubject,
        _user: &UserId,
    ) -> Result<Membership, NotificationAudienceError> {
        Ok(Membership::Unknown)
    }
}

async fn insert(
    pool: &DbPool,
    tag: &str,
    label: &str,
    status: &str,
    kind: &str,
    roles: &[&str],
) -> UserId {
    let id = UserId::new(format!("dir-{tag}-{label}"));
    let roles: Vec<String> = roles.iter().map(|role| (*role).to_owned()).collect();
    sqlx::query(
        "INSERT INTO users (id, name, email, status, account_kind, roles, display_name) \
         VALUES ($1, $1, $1 || '@example.test', $2, $3, $4, $5)",
    )
    .bind(id.as_str())
    .bind(status)
    .bind(kind)
    .bind(&roles)
    .bind(format!("Display {label}"))
    .execute(&*pool.write_pool())
    .await
    .expect("insert user");
    id
}

async fn directory(pool: &DbPool, audiences: ExtensionAudiences) -> UserSubjectDirectory {
    UserSubjectDirectory::new(
        Arc::new(UserRepository::new(pool)),
        vec!["catalog-role".to_owned()],
        audiences,
    )
}

#[tokio::test]
async fn users_and_roles_resolve_only_active_people() {
    let pool = systemprompt_test_fixtures::test_db_pool().await;
    let tag = Uuid::new_v4().simple().to_string();
    let role = format!("role-{tag}");
    let active = insert(&pool, &tag, "active", "active", "person", &["user", &role]).await;
    let suspended = insert(&pool, &tag, "suspended", "suspended", "person", &[&role]).await;
    let service = insert(&pool, &tag, "service", "active", "service", &[&role]).await;
    let dir = directory(&pool, ExtensionAudiences::default()).await;

    assert_eq!(
        dir.roles_of(&active).await.unwrap(),
        Some(vec!["user".to_owned(), role.clone()])
    );
    assert_eq!(dir.roles_of(&suspended).await.unwrap(), None);
    assert_eq!(
        dir.roles_of_many(&[suspended.clone(), active.clone(), UserId::new("nobody")])
            .await
            .unwrap(),
        vec![None, Some(vec!["user".to_owned(), role.clone()]), None]
    );

    assert_eq!(
        dir.members("user", active.as_str(), 10).await.unwrap(),
        vec![active.clone()]
    );
    assert!(
        dir.members("user", suspended.as_str(), 10)
            .await
            .unwrap()
            .is_empty()
    );
    let mut holders = dir.members("role", &role, 10).await.unwrap();
    holders.sort();
    let mut expected = vec![active.clone(), service.clone()];
    expected.sort();
    assert_eq!(holders, expected);

    assert!(dir.is_person(&active).await.unwrap());
    assert!(!dir.is_person(&service).await.unwrap());
    assert!(!dir.is_person(&suspended).await.unwrap());

    let found = dir.search_users(&format!("dir-{tag}"), 10).await.unwrap();
    let values: Vec<&str> = found.iter().map(|m| m.value.as_str()).collect();
    assert_eq!(values, vec![active.as_str()]);
    assert_eq!(found[0].display, "Display active");
    assert_eq!(
        found[0].detail.as_deref(),
        Some(format!("{active}@example.test").as_str())
    );

    let roles = dir.roles().await.unwrap();
    for expected in ["admin", "user", "catalog-role", role.as_str()] {
        assert!(
            roles.iter().any(|r| r == expected),
            "{expected} missing from {roles:?}"
        );
    }
    let mut sorted = roles.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(roles, sorted);
}

#[tokio::test]
async fn dimensions_resolve_through_the_first_handling_resolver() {
    let pool = systemprompt_test_fixtures::test_db_pool().await;
    let unbound = directory(&pool, ExtensionAudiences::default()).await;
    assert!(
        unbound
            .members("team", "core", 10)
            .await
            .unwrap()
            .is_empty()
    );

    let audiences = ExtensionAudiences::default();
    audiences.bind(vec![Arc::new(Team)]);
    let bound = directory(&pool, audiences).await;

    assert_eq!(
        bound.members("team", "core", 1).await.unwrap(),
        vec![UserId::new("core-member-0")]
    );
    assert!(bound.members("region", "eu", 10).await.unwrap().is_empty());
}
