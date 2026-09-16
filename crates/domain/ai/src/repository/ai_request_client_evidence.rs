//! Repository for the client evidence recorded beside each AI request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::RepositoryError;
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::AiRequestId;
use systemprompt_models::wire::origin::{
    ClientAttestation, ClientEvidence, ClientKind, NativeMarker,
};

#[must_use]
#[derive(Debug, Clone)]
pub struct AiRequestClientEvidenceRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
}

struct EvidenceRow {
    kind_source: String,
    attested_host: Option<String>,
    declared_client: Option<String>,
    native_marker: Option<String>,
    ua_product: Option<String>,
    ua_version: Option<String>,
    sdk_lang: Option<String>,
    sdk_package_version: Option<String>,
    sdk_runtime: Option<String>,
    sdk_runtime_version: Option<String>,
    sdk_os: Option<String>,
    sdk_arch: Option<String>,
}

impl AiRequestClientEvidenceRepository {
    pub fn new(db: &DbPool) -> Result<Self, RepositoryError> {
        let pool = db
            .pool_arc()
            .map_err(|e| RepositoryError::PoolInitialization(e.to_string()))?;
        let write_pool = db
            .write_pool_arc()
            .map_err(|e| RepositoryError::PoolInitialization(e.to_string()))?;
        Ok(Self { pool, write_pool })
    }

    pub async fn upsert(
        &self,
        ai_request_id: &AiRequestId,
        evidence: &ClientEvidence,
    ) -> Result<(), RepositoryError> {
        sqlx::query!(
            r#"
            INSERT INTO ai_request_client_evidence (
                ai_request_id, kind_source, attested_host, declared_client, native_marker,
                ua_product, ua_version, sdk_lang, sdk_package_version, sdk_runtime,
                sdk_runtime_version, sdk_os, sdk_arch
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            ON CONFLICT (ai_request_id) DO UPDATE
            SET kind_source = EXCLUDED.kind_source,
                attested_host = EXCLUDED.attested_host,
                declared_client = EXCLUDED.declared_client,
                native_marker = EXCLUDED.native_marker,
                ua_product = EXCLUDED.ua_product,
                ua_version = EXCLUDED.ua_version,
                sdk_lang = EXCLUDED.sdk_lang,
                sdk_package_version = EXCLUDED.sdk_package_version,
                sdk_runtime = EXCLUDED.sdk_runtime,
                sdk_runtime_version = EXCLUDED.sdk_runtime_version,
                sdk_os = EXCLUDED.sdk_os,
                sdk_arch = EXCLUDED.sdk_arch
            "#,
            ai_request_id.as_str(),
            evidence.kind_source.as_str(),
            evidence.attested_host.map(ClientKind::as_str),
            evidence.declared_client.as_deref(),
            evidence.native_marker.map(NativeMarker::as_str),
            evidence.ua_product.as_deref(),
            evidence.ua_version.as_deref(),
            evidence.sdk_lang.as_deref(),
            evidence.sdk_package_version.as_deref(),
            evidence.sdk_runtime.as_deref(),
            evidence.sdk_runtime_version.as_deref(),
            evidence.sdk_os.as_deref(),
            evidence.sdk_arch.as_deref()
        )
        .execute(self.write_pool.as_ref())
        .await?;
        Ok(())
    }

    pub async fn find(
        &self,
        ai_request_id: &AiRequestId,
    ) -> Result<Option<ClientEvidence>, RepositoryError> {
        let row = sqlx::query_as!(
            EvidenceRow,
            r#"
            SELECT kind_source, attested_host, declared_client, native_marker,
                   ua_product, ua_version, sdk_lang, sdk_package_version, sdk_runtime,
                   sdk_runtime_version, sdk_os, sdk_arch
            FROM ai_request_client_evidence
            WHERE ai_request_id = $1
            "#,
            ai_request_id.as_str()
        )
        .fetch_optional(self.pool.as_ref())
        .await?;
        row.map(TryInto::try_into).transpose()
    }
}

impl TryFrom<EvidenceRow> for ClientEvidence {
    type Error = RepositoryError;

    fn try_from(row: EvidenceRow) -> Result<Self, Self::Error> {
        let invalid =
            |e: systemprompt_models::wire::origin::OriginParseError| RepositoryError::InvalidData {
                field: "ai_request_client_evidence".to_owned(),
                reason: e.to_string(),
            };
        Ok(Self {
            kind_source: ClientAttestation::parse(&row.kind_source).map_err(invalid)?,
            attested_host: row
                .attested_host
                .as_deref()
                .map(ClientKind::parse)
                .transpose()
                .map_err(invalid)?,
            declared_client: row.declared_client,
            native_marker: row
                .native_marker
                .as_deref()
                .map(NativeMarker::parse)
                .transpose()
                .map_err(invalid)?,
            ua_product: row.ua_product,
            ua_version: row.ua_version,
            sdk_lang: row.sdk_lang,
            sdk_package_version: row.sdk_package_version,
            sdk_runtime: row.sdk_runtime,
            sdk_runtime_version: row.sdk_runtime_version,
            sdk_os: row.sdk_os,
            sdk_arch: row.sdk_arch,
        })
    }
}
