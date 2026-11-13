//! Per-IP session counts over the last 24 hours, read by the analytics
//! crate's malicious-IP blacklist through
//! `systemprompt_traits::SessionStore::ip_session_counts`. The reads go to
//! the read replica; ban mutations live in `BannedIpRepository`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_traits::session_store::{IpSessionCount, IpSessionQuery};

use super::SessionRepository;
use crate::Result;

impl SessionRepository {
    pub async fn ip_session_counts(&self, query: IpSessionQuery) -> Result<Vec<IpSessionCount>> {
        match query {
            IpSessionQuery::HumanAtLeast(threshold) => self.high_volume_ips(threshold).await,
            IpSessionQuery::ScannerAtLeast(threshold) => self.scanner_ips(threshold).await,
            IpSessionQuery::All => self.recent_ips().await,
            IpSessionQuery::CountryAtLeast(threshold) => self.country_ips(threshold).await,
        }
    }

    async fn high_volume_ips(&self, threshold: i64) -> Result<Vec<IpSessionCount>> {
        let rows = sqlx::query!(
            r#"
            SELECT ip_address, COUNT(*) as session_count
            FROM v_clean_traffic
            WHERE started_at >= NOW() - INTERVAL '24 hours'
              AND ip_address IS NOT NULL
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
                r.ip_address.map(|ip_address| IpSessionCount {
                    ip_address,
                    country: None,
                    session_count: r.session_count.unwrap_or(0),
                })
            })
            .collect())
    }

    async fn scanner_ips(&self, threshold: i64) -> Result<Vec<IpSessionCount>> {
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
                r.ip_address.map(|ip_address| IpSessionCount {
                    ip_address,
                    country: None,
                    session_count: r.session_count.unwrap_or(0),
                })
            })
            .collect())
    }

    async fn recent_ips(&self) -> Result<Vec<IpSessionCount>> {
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
                r.ip_address.map(|ip_address| IpSessionCount {
                    ip_address,
                    country: None,
                    session_count: r.session_count.unwrap_or(0),
                })
            })
            .collect())
    }

    async fn country_ips(&self, threshold: i64) -> Result<Vec<IpSessionCount>> {
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
                r.ip_address.map(|ip_address| IpSessionCount {
                    ip_address,
                    country: r.country,
                    session_count: r.session_count.unwrap_or(0),
                })
            })
            .collect())
    }
}
