use systemprompt_database::Database;

#[tokio::test]
async fn transaction_with_parameters() {
    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL environment variable required");

    match Database::new_postgres(&database_url).await {
        Ok(db) => {
            let mut tx = db.begin().await.expect("Failed to begin transaction");

            let result: (i64,) = sqlx::query_as("SELECT $1::int8")
                .bind(42i64)
                .fetch_one(&mut *tx)
                .await
                .expect("Failed to execute query with parameters");

            assert_eq!(result.0, 42, "Expected parameter to be bound correctly");

            tx.commit().await.expect("Failed to commit transaction");
        },
        Err(e) => {
            eprintln!("Skipping test (database not available): {}", e);
        },
    }
}
