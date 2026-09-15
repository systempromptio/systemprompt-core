//! Session geolocation enrichment through the session owner.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::SessionRepository;
use crate::{AnalyticsError, GeoIpReader, Result};
use systemprompt_identifiers::SessionId;

impl SessionRepository {
    pub(super) async fn backfill_geo(
        &self,
        reader: Option<&GeoIpReader>,
        batch_size: i64,
    ) -> Result<u64> {
        if batch_size <= 0 {
            return Err(AnalyticsError::invalid_argument(
                "batch size must be positive",
            ));
        }
        let mut updated = 0;
        let mut after: Option<SessionId> = None;
        loop {
            let rows = self
                .owner
                .sessions_missing_geo(after.as_ref(), batch_size)
                .await
                .map_err(AnalyticsError::from)?;
            let Some(last) = rows.last() else {
                break;
            };
            after = Some(last.0.clone());
            for (session_id, ip) in rows {
                if let Some((country, region, city)) =
                    crate::services::extractor::geoip::lookup_geoip(&ip, reader)
                {
                    updated += self
                        .owner
                        .set_session_geo(
                            &session_id,
                            country.as_deref(),
                            region.as_deref(),
                            city.as_deref(),
                        )
                        .await
                        .map_err(AnalyticsError::from)?;
                }
            }
        }
        Ok(updated)
    }
}
