use anyhow::Result;
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;

#[derive(Debug, Clone)]
pub struct SecurityRepository {
    pool: Arc<PgPool>,
}

#[derive(Debug)]
pub struct IpSessionRecord {
    pub ip_address: Option<String>,
    pub country: Option<String>,
    pub session_count: i64,
}

impl SecurityRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        let pool = db.pool_arc()?;
        Ok(Self { pool })
    }

    pub async fn find_high_volume_ips(&self, threshold: i64) -> Result<Vec<IpSessionRecord>> {
        let rows = sqlx::query!(
            r#"
            SELECT ip_address, COUNT(*) as session_count
            FROM user_sessions
            WHERE started_at >= NOW() - INTERVAL '24 hours'
              AND ip_address IS NOT NULL
              AND is_bot = false
            GROUP BY ip_address
            HAVING COUNT(*) >= $1
            "#,
            threshold
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|r| {
                r.ip_address.map(|ip| IpSessionRecord {
                    ip_address: Some(ip),
                    country: None,
                    session_count: r.session_count.unwrap_or(0),
                })
            })
            .collect())
    }

    pub async fn find_scanner_ips(&self, threshold: i64) -> Result<Vec<IpSessionRecord>> {
        let rows = sqlx::query!(
            r#"
            SELECT ip_address, COUNT(*) as session_count
            FROM user_sessions
            WHERE started_at >= NOW() - INTERVAL '24 hours'
              AND ip_address IS NOT NULL
              AND is_scanner = true
            GROUP BY ip_address
            HAVING COUNT(*) >= $1
            "#,
            threshold
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|r| {
                r.ip_address.map(|ip| IpSessionRecord {
                    ip_address: Some(ip),
                    country: None,
                    session_count: r.session_count.unwrap_or(0),
                })
            })
            .collect())
    }

    pub async fn find_recent_ips(&self) -> Result<Vec<IpSessionRecord>> {
        let rows = sqlx::query!(
            r#"
            SELECT ip_address, COUNT(*) as session_count
            FROM user_sessions
            WHERE started_at >= NOW() - INTERVAL '24 hours'
              AND ip_address IS NOT NULL
            GROUP BY ip_address
            "#
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|r| {
                r.ip_address.map(|ip| IpSessionRecord {
                    ip_address: Some(ip),
                    country: None,
                    session_count: r.session_count.unwrap_or(0),
                })
            })
            .collect())
    }

    pub async fn find_high_risk_country_ips(&self, threshold: i64) -> Result<Vec<IpSessionRecord>> {
        let rows = sqlx::query!(
            r#"
            SELECT ip_address, country, COUNT(*) as session_count
            FROM user_sessions
            WHERE started_at >= NOW() - INTERVAL '24 hours'
              AND ip_address IS NOT NULL
              AND country IS NOT NULL
            GROUP BY ip_address, country
            HAVING COUNT(*) >= $1
            "#,
            threshold
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|r| {
                r.ip_address.map(|ip| IpSessionRecord {
                    ip_address: Some(ip),
                    country: r.country,
                    session_count: r.session_count.unwrap_or(0),
                })
            })
            .collect())
    }
}
