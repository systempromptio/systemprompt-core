//! Scope attribution rows (`ai_request_attributions`).
//!
//! Written in the same transaction as the request row they describe, read
//! back in batches by the OTLP exporter.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use sqlx::{Postgres, Transaction};
use systemprompt_identifiers::{AiRequestId, ScopeDimension};
use systemprompt_models::attribution::{AttributionEntry, AttributionSource, RequestAttribution};
use systemprompt_traits::RepositoryError;

use super::AiRequestRepository;

pub(super) async fn insert_attributions(
    tx: &mut Transaction<'_, Postgres>,
    id: &AiRequestId,
    attribution: &RequestAttribution,
) -> Result<(), RepositoryError> {
    if attribution.entries.is_empty() {
        return Ok(());
    }
    let mut dimensions = Vec::with_capacity(attribution.entries.len());
    let mut values = Vec::with_capacity(attribution.entries.len());
    let mut sources = Vec::with_capacity(attribution.entries.len());
    for entry in &attribution.entries {
        dimensions.push(entry.dimension.as_str().to_owned());
        values.push(entry.value.clone());
        sources.push(entry.source.as_str().to_owned());
    }
    sqlx::query!(
        r#"
        INSERT INTO ai_request_attributions (request_id, dimension, value, source)
        SELECT $1, t.dimension, t.value, t.source
        FROM UNNEST($2::text[], $3::text[], $4::text[]) AS t(dimension, value, source)
        "#,
        id.as_str(),
        &dimensions,
        &values,
        &sources
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

impl AiRequestRepository {
    pub async fn attributions_for(
        &self,
        ids: &[AiRequestId],
    ) -> Result<HashMap<AiRequestId, Vec<AttributionEntry>>, RepositoryError> {
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
        .fetch_all(self.pool())
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
