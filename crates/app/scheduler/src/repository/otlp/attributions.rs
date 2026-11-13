//! The attribution rows the trace export stamps on each request span, read
//! with the rest of the audit tail.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use systemprompt_identifiers::{AiRequestId, ScopeDimension};
use systemprompt_models::attribution::{AttributionEntry, AttributionSource};
use systemprompt_traits::RepositoryError;

use super::tail::OtlpAuditTailRepository;
use crate::error::SchedulerResult;

impl OtlpAuditTailRepository {
    pub async fn attributions_for(
        &self,
        ids: &[AiRequestId],
    ) -> SchedulerResult<HashMap<AiRequestId, Vec<AttributionEntry>>> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        let keys: Vec<String> = ids.iter().map(|id| id.as_str().to_owned()).collect();
        let rows = sqlx::query!(
            r#"
            SELECT request_id, dimension, value, source
            FROM ai_request_attributions
            WHERE request_id = ANY($1)
            ORDER BY request_id, dimension
            "#,
            &keys
        )
        .fetch_all(&self.pool)
        .await?;
        let mut out: HashMap<AiRequestId, Vec<AttributionEntry>> = HashMap::new();
        for row in rows {
            let dimension = ScopeDimension::try_new(row.dimension.as_str()).map_err(|e| {
                RepositoryError::decode(
                    format!("ai_request_attributions.dimension '{}'", row.dimension),
                    e,
                )
            })?;
            let source = AttributionSource::parse(&row.source).ok_or_else(|| {
                RepositoryError::invalid_data(
                    "ai_request_attributions.source",
                    format!("'{}' is not a known source", row.source),
                )
            })?;
            out.entry(AiRequestId::new(row.request_id))
                .or_default()
                .push(AttributionEntry {
                    dimension,
                    value: row.value,
                    source,
                });
        }
        Ok(out)
    }
}
