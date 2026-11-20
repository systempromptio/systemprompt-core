//! People search through `UserRepository::search_active_by_prefix`: every
//! word must start a word of a name or email, in any order, accents ignored.

use anyhow::Result;
use systemprompt_identifiers::UserId;
use systemprompt_test_fixtures::test_db_pool;
use systemprompt_users::UserRepository;

async fn found(repo: &UserRepository, query: &str, id: &UserId) -> Result<bool> {
    let users = repo.search_active_by_prefix(query, 50).await?;
    Ok(users.iter().any(|user| &user.id == id))
}

#[tokio::test]
async fn people_search_matches_words_in_any_order_ignoring_accents() -> Result<()> {
    let db = test_db_pool().await;
    let repo = UserRepository::new(&db);
    let tag = uuid::Uuid::new_v4().simple().to_string();
    let email = format!("jesus.vargas.{tag}@example.com");
    let user = repo
        .create(
            &format!("jvargas_{tag}"),
            &email,
            Some("Jesús Vargas Sánchez"),
            Some("Jesús Vargas Sánchez"),
        )
        .await?;

    let email_prefix = &email[..email.find('@').unwrap_or(email.len()) + 4];
    let outcome = async {
        for query in [
            "jesus vargas",
            "vargas jesus",
            "Jesús",
            "jesus",
            "JESÚS",
            "vargas sánchez",
            "  sanchez   jes ",
            email_prefix,
            &format!("jvargas_{tag}"),
            &user.id.as_str()[..8],
        ] {
            anyhow::ensure!(
                found(&repo, query, &user.id).await?,
                "{query:?} should match"
            );
        }
        for query in [
            "jesus garcia",
            "argas",
            "jesus vargas sanchez nobody",
            "%vargas",
        ] {
            anyhow::ensure!(
                !found(&repo, query, &user.id).await?,
                "{query:?} should not match"
            );
        }
        Ok(())
    }
    .await;

    sqlx::query!("DELETE FROM users WHERE id = $1", user.id.as_str())
        .execute(db.pool().as_ref())
        .await?;
    outcome
}
