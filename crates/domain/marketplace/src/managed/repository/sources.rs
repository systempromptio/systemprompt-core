//! Import provenance never changes the runtime publication pointer.

use sqlx::types::Json;
use systemprompt_identifiers::{ManagedResourceId, ManagedSourceId, SourceSnapshotId, UserId};

use super::{ManagedRepository, NewResource};
use crate::managed::error::invalid;
use crate::managed::provenance::validate_key;
use crate::managed::{AssetDigest, ManagedError, Result, SnapshotProvenance, SourceSpec};

impl ManagedRepository {
    pub async fn register_source(
        &self,
        owner: &UserId,
        name: &str,
        spec: &SourceSpec,
    ) -> Result<ManagedSourceId> {
        spec.validate()?;
        validate_key(name)?;
        let id = ManagedSourceId::generate();
        let kind = spec.kind();
        sqlx::query!("INSERT INTO managed_sources(id,owner_id,name,kind,specification) VALUES($1,$2,$3,$4,$5) ON CONFLICT(owner_id,name) DO NOTHING",
            id.as_str(), owner.as_str(), name, kind, Json(spec) as _).execute(&self.pool).await?;
        let row = sqlx::query!(r#"SELECT id, specification AS "specification!: Json<SourceSpec>" FROM managed_sources WHERE owner_id=$1 AND name=$2"#,
            owner.as_str(), name).fetch_one(&self.pool).await?;
        if serde_jcs::to_vec(&row.specification.0)? != serde_jcs::to_vec(spec)? {
            return Err(ManagedError::Conflict(
                "Source name already has a different specification".to_owned(),
            ));
        }
        Ok(ManagedSourceId::new(row.id))
    }

    pub async fn get_source(&self, owner: &UserId, id: &ManagedSourceId) -> Result<SourceSpec> {
        let row = sqlx::query_scalar!(r#"SELECT specification AS "specification!: Json<SourceSpec>" FROM managed_sources WHERE owner_id=$1 AND id=$2"#,
            owner.as_str(), id.as_str()).fetch_optional(&self.pool).await?.ok_or(ManagedError::Unavailable)?;
        row.0.validate()?;
        Ok(row.0)
    }

    pub async fn capture_snapshot(
        &self,
        owner: &UserId,
        source: &ManagedSourceId,
        provenance: &SnapshotProvenance,
    ) -> Result<SourceSnapshotId> {
        provenance.validate(&self.get_source(owner, source).await?)?;
        let digest = AssetDigest::of(&serde_jcs::to_vec(provenance)?);
        let id = SourceSnapshotId::generate();
        sqlx::query!("INSERT INTO managed_source_snapshots(id,owner_id,source_id,digest,provenance) VALUES($1,$2,$3,$4,$5) ON CONFLICT(owner_id,source_id,digest) DO NOTHING",
            id.as_str(), owner.as_str(), source.as_str(), digest.as_str(), Json(provenance) as _).execute(&self.pool).await?;
        let existing = sqlx::query_scalar!("SELECT id FROM managed_source_snapshots WHERE owner_id=$1 AND source_id=$2 AND digest=$3",
            owner.as_str(), source.as_str(), digest.as_str()).fetch_one(&self.pool).await?;
        Ok(SourceSnapshotId::new(existing))
    }

    pub async fn bind_resource(
        &self,
        owner: &UserId,
        input: &NewResource,
    ) -> Result<ManagedResourceId> {
        self.get_source(owner, &input.source_id).await?;
        validate_key(&input.upstream_key)?;
        validate_key(&input.resource_key)?;
        if input.resource_key.contains('/') {
            return Err(invalid("Installation resource keys cannot contain slashes"));
        }
        let id = ManagedResourceId::generate();
        let kind = input.kind.as_str();
        sqlx::query!("INSERT INTO managed_resources(id,owner_id,source_id,upstream_key,kind,resource_key) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT DO NOTHING",
            id.as_str(), owner.as_str(), input.source_id.as_str(), input.upstream_key, kind, input.resource_key).execute(&self.pool).await?;
        let row = sqlx::query!("SELECT id,kind,resource_key FROM managed_resources WHERE owner_id=$1 AND source_id=$2 AND upstream_key=$3",
            owner.as_str(), input.source_id.as_str(), input.upstream_key).fetch_optional(&self.pool).await?
            .ok_or_else(|| ManagedError::Conflict("Resource key is already bound to another source".to_owned()))?;
        if row.kind != kind || row.resource_key != input.resource_key {
            return Err(ManagedError::Conflict(
                "Upstream resource already has a different binding".to_owned(),
            ));
        }
        Ok(ManagedResourceId::new(row.id))
    }
}
