use anyhow::Result;
use sqlx::PgConnection;
use systemprompt_test_fixtures::test_db_pool;
use systemprompt_users::{DeclaredUserRow, DeclaredUserWrite, UserError, UserRepository};

fn write<'a>(email: &'a str, kind: &'a str, roles: &'a [String]) -> DeclaredUserWrite<'a> {
    DeclaredUserWrite {
        email,
        name: "Declared Person",
        account_kind: kind,
        roles,
        status: "active",
        source: "identity/users.yaml",
    }
}

async fn row(conn: &mut PgConnection, email: &str) -> Result<Option<DeclaredUserRow>> {
    Ok(UserRepository::list_declared_on(conn)
        .await?
        .into_iter()
        .find(|row| row.email == email))
}

#[tokio::test]
async fn a_declared_user_is_upserted_by_email_and_released() -> Result<()> {
    let db = test_db_pool().await;
    let pool = db.write_pool();
    let mut conn = pool.acquire().await?;
    let email = format!("declared_{}@example.com", uuid::Uuid::new_v4().simple());
    let roles = vec!["user".to_owned(), "sales".to_owned()];

    let id = UserRepository::upsert_declared_on(
        &mut conn,
        &write(&email.to_uppercase(), "person", &roles),
    )
    .await?;
    let stored = row(&mut conn, &email).await?.expect("declared row");
    assert_eq!(stored.account_kind, "person");
    assert_eq!(stored.roles, roles);
    assert_eq!(stored.status, "active");
    assert_eq!(
        UserRepository::find_id_by_email_on(&mut conn, &email).await?,
        Some(id.clone())
    );

    let narrowed = vec!["user".to_owned()];
    let again =
        UserRepository::upsert_declared_on(&mut conn, &write(&email, "person", &narrowed)).await?;
    assert_eq!(again, id, "the same email keeps the same row");
    assert_eq!(row(&mut conn, &email).await?.expect("row").roles, narrowed);

    let emails = UserRepository::emails_by_ids_on(&mut conn, std::slice::from_ref(&id)).await?;
    assert_eq!(emails, vec![(id.clone(), email.clone())]);

    UserRepository::release_declared_on(&mut conn, &email).await?;
    assert!(row(&mut conn, &email).await?.is_none());
    let status: String = sqlx::query_scalar("SELECT status FROM users WHERE id = $1")
        .bind(id.as_str())
        .fetch_one(&mut *conn)
        .await?;
    assert_eq!(status, "inactive");

    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(id.as_str())
        .execute(&mut *conn)
        .await?;
    Ok(())
}

#[tokio::test]
async fn a_declaration_of_the_other_account_kind_is_refused() -> Result<()> {
    let db = test_db_pool().await;
    let pool = db.write_pool();
    let mut conn = pool.acquire().await?;
    let email = format!("svc_{}@example.com", uuid::Uuid::new_v4().simple());
    let roles = vec!["user".to_owned()];
    let id =
        UserRepository::upsert_declared_on(&mut conn, &write(&email, "service", &roles)).await?;

    let refused = UserRepository::upsert_declared_on(&mut conn, &write(&email, "person", &roles))
        .await
        .unwrap_err();
    assert!(
        matches!(&refused, UserError::AccountKindMismatch(e) if *e == email),
        "{refused:?}"
    );
    assert_eq!(
        row(&mut conn, &email).await?.expect("row").account_kind,
        "service"
    );
    assert_eq!(
        UserRepository::find_id_by_email_on(&mut conn, "nobody@nowhere.invalid").await?,
        None
    );

    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(id.as_str())
        .execute(&mut *conn)
        .await?;
    Ok(())
}
