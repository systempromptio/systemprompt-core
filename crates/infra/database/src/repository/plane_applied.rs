//! Whether a config plane has ever been applied on this database.
//!
//! An explicit apply records each plane it ran in `config_plane_applied`,
//! declared or not, on the apply's own transaction. A runtime reader whose
//! table is empty asks [`find_config_plane_applied`] before it chooses
//! between "the operator declared nothing" and "nobody ran the apply", and
//! fails closed on the second.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::{PgConnection, PgPool};
use systemprompt_traits::RepositoryError;

pub const PLANE_NOT_APPLIED_HINT: &str = "run `systemprompt admin config apply`";

pub async fn find_config_plane_applied(
    pool: &PgPool,
    plane: &str,
) -> Result<bool, RepositoryError> {
    let applied = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM config_plane_applied WHERE plane = $1) AS "applied!""#,
        plane,
    )
    .fetch_one(pool)
    .await?;
    Ok(applied)
}

pub async fn record_config_plane_applied(
    conn: &mut PgConnection,
    plane: &str,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        "INSERT INTO config_plane_applied (plane) VALUES ($1)
         ON CONFLICT (plane) DO UPDATE SET applied_at = NOW()",
        plane,
    )
    .execute(conn)
    .await?;
    Ok(())
}
