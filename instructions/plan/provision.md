# Shared Registry Provisioning

## Problem

Per-app deploy tokens cannot be created via Fly REST API (only via `flyctl` CLI). Current implementation fails with 404 when calling non-existent `/v1/apps/{app}/tokens` endpoint.

## Solution

Use a single shared Fly registry with tenant-scoped tags. Each tenant gets exactly ONE image tag.

```
registry.fly.io/systemprompt-images:tenant-{tenant_id}
```

## Architecture

```
PUSH FLOW (CLI):
  1. CLI: GET /tenants/{id}/registry-token
  2. API: Returns { registry, username, password, repository, tag }
  3. CLI: docker push registry.fly.io/systemprompt-images:tenant-{id}
  4. CLI: POST /tenants/{id}/deploy { image: "registry.fly.io/systemprompt-images:tenant-{id}" }

DEPLOY FLOW (API):
  1. API: Validate image matches tenant tag
  2. API: Create/update machine with image

CLEANUP FLOW (on tenant delete):
  1. Delete Fly machine (existing)
  2. Delete Fly volume (existing)
  3. Delete Fly app (existing)
  4. Delete registry tag (NEW)
```

---

## One-Time Setup

Create shared registry app:
```bash
fly apps create systemprompt-images --org systemprompt
```

Add to Fly secrets:
```bash
fly secrets set FLY_SHARED_REGISTRY_APP=systemprompt-images -a management-api-sandbox
fly secrets set FLY_SHARED_REGISTRY_APP=systemprompt-images -a management-api-prod
```

---

## File Changes

### 1. FlyConfig - Add shared registry app

**File:** `crates/management-api/src/services/fly/config.rs`

```rust
pub struct FlyConfig {
    pub api_token: String,
    pub org_slug: String,
    pub org_id: String,
    pub machines_api_url: String,
    pub core_image: String,
    pub postgres_app: String,
    pub subdomain_suffix: String,
    pub shared_registry_app: String,  // NEW
    pub enabled: bool,
}

impl FlyConfig {
    pub fn from_env() -> Result<Self> {
        // ...
        Ok(Self {
            // ... existing
            shared_registry_app: std::env::var("FLY_SHARED_REGISTRY_APP")
                .context("FLY_SHARED_REGISTRY_APP is required when FLY_ENABLED=true")?,
            // ...
        })
    }

    pub fn build_image_tag(tenant_id: &TenantId) -> String {
        format!("tenant-{}", tenant_id)
    }

    pub fn build_full_image(&self, tenant_id: &TenantId) -> String {
        format!(
            "registry.fly.io/{}:{}",
            self.shared_registry_app,
            Self::build_image_tag(tenant_id)
        )
    }
}
```

### 2. RegistryTokenResponse - Add tag field

**File:** `crates/management-api/src/models/tenant.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryTokenResponse {
    pub registry: String,
    pub username: String,
    #[serde(skip_serializing)]
    pub password: String,
    pub repository: String,
    pub tag: String,  // NEW
}
```

### 3. get_registry_token - Use org token + shared registry

**File:** `crates/management-api/src/api/tenants/handlers.rs`

```rust
pub async fn get_registry_token(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<TenantId>,
) -> ApiResult<Json<SingleResponse<RegistryTokenResponse>>> {
    let customer = get_customer(&state, user.user_id()).await?;
    let tenant = db::get_tenant(&state.db, id).await?;
    verify_ownership(&tenant, &customer.id)?;

    let Some(ref provisioner) = state.fly_provisioner else {
        return Err(ApiError::service_unavailable("Cloud not enabled"));
    };

    let tag = format!("tenant-{}", tenant.id);

    let response = RegistryTokenResponse {
        registry: "registry.fly.io".to_string(),
        username: "x".to_string(),
        password: provisioner.config().api_token.clone(),
        repository: provisioner.config().shared_registry_app.clone(),
        tag,
    };

    tracing::info!(
        tenant_id = %id,
        repository = %response.repository,
        tag = %response.tag,
        "Registry token requested"
    );

    Ok(Json(SingleResponse::new(response)))
}
```

### 4. deploy - Validate image matches tenant

**File:** `crates/management-api/src/api/tenants/handlers.rs`

```rust
pub async fn deploy(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<TenantId>,
    Json(req): Json<DeployRequest>,
) -> ApiResult<Json<SingleResponse<DeployResponse>>> {
    // ... existing auth checks ...

    let Some(ref provisioner) = state.fly_provisioner else {
        return Err(ApiError::service_unavailable("Cloud not enabled"));
    };

    let expected_image = provisioner.config().build_full_image(&id);
    if req.image != expected_image {
        return Err(ApiError::bad_request(format!(
            "Invalid image. Expected: {}",
            expected_image
        )));
    }

    // ... rest of deploy logic ...
}
```

### 5. Provisioner - Remove deploy token creation

**File:** `crates/management-api/src/services/fly/provisioner.rs`

Remove `create_deploy_token` call from `provision_tenant_internal`:

```rust
async fn provision_tenant_internal(
    &self,
    app_name: &str,
    tenant_id: &TenantId,
    _tenant_name: &str,
    secrets: &TenantSecrets,
    region: &str,
    _memory_mb: u32,
    volume_gb: u32,
    state: &mut ProvisioningState,
) -> Result<ProvisioningResult, FlyError> {
    self.client.create_app(app_name).await?;
    state.app_name = Some(app_name.to_string());

    // REMOVED: create_deploy_token call

    let mut fly_secrets = HashMap::new();
    fly_secrets.insert("DATABASE_URL".to_string(), secrets.database_url.clone());
    fly_secrets.insert("JWT_SECRET".to_string(), secrets.jwt_secret.clone());
    self.client.set_secrets(app_name, fly_secrets).await?;
    tracing::info!(app_name = %app_name, "Cloud secrets configured");

    let volume = self.client.create_volume(app_name, "services", volume_gb, region).await?;
    state.volume_id = Some(volume.id.clone());

    self.client.allocate_shared_ipv4(app_name).await?;
    self.client.allocate_ipv6(app_name).await?;

    let hostname = self.config.build_hostname(tenant_id);
    self.client.add_certificate(app_name, &hostname).await?;

    Ok(ProvisioningResult {
        app_name: app_name.to_string(),
        machine_id: None,
        volume_id: volume.id,
        hostname,
        internal_hostname: FlyConfig::build_internal_hostname(tenant_id),
        status: "awaiting_deploy".to_string(),
    })
}
```

### 6. ProvisioningResult - Remove deploy_token field

**File:** `crates/management-api/src/services/fly/models.rs`

```rust
pub struct ProvisioningResult {
    pub app_name: String,
    pub machine_id: Option<String>,
    pub volume_id: String,
    pub hostname: String,
    pub internal_hostname: String,
    pub status: String,
    // REMOVED: deploy_token
}
```

### 7. TenantFlyMetadata - Remove deploy_token

**File:** `crates/management-api/src/services/fly/models.rs`

```rust
pub struct TenantFlyMetadata {
    pub app_name: String,
    pub machine_id: Option<String>,
    pub volume_id: String,
    pub hostname: String,
    pub internal_hostname: String,
    pub status: FlyAppStatus,
    // REMOVED: deploy_token
}
```

### 8. Tenant Model - Remove deploy_token

**File:** `crates/management-api/src/models/tenant.rs`

Remove `fly_deploy_token` field and `require_deploy_token()` method.

### 9. TenantCloudConfig - Remove deploy_token

**File:** `crates/management-api/src/models/tenant.rs`

```rust
pub struct TenantCloudConfig {
    pub app_name: String,
    pub machine_id: Option<String>,
    pub volume_id: String,
    pub hostname: String,
    pub internal_hostname: String,
    pub status: String,
    // REMOVED: deploy_token
}
```

### 10. DB Queries - Remove fly_deploy_token

**File:** `crates/management-api/src/db/tenants/crud.rs`

Update `TENANT_COLUMNS` to remove `fly_deploy_token`.

Update `update_tenant_fly_metadata` to not set `fly_deploy_token`.

### 11. FlyClient - Remove create_deploy_token

**File:** `crates/management-api/src/services/fly/client/apps.rs`

Remove `create_deploy_token` function and related structs.

### 12. Registry Cleanup - Add delete_registry_tag

**File:** `crates/management-api/src/services/fly/client/registry.rs` (NEW)

```rust
use super::{FlyClient, FlyError};

impl FlyClient {
    pub async fn delete_registry_tag(
        &self,
        repository: &str,
        tag: &str,
    ) -> Result<(), FlyError> {
        self.check_circuit().await?;

        let manifest_url = format!(
            "https://registry.fly.io/v2/{}/manifests/{}",
            repository, tag
        );

        let digest_response = self
            .client
            .head(&manifest_url)
            .header("Authorization", format!("Bearer {}", self.config.api_token))
            .header("Accept", "application/vnd.docker.distribution.manifest.v2+json")
            .send()
            .await?;

        let Some(digest) = digest_response.headers().get("docker-content-digest") else {
            tracing::warn!(repository = %repository, tag = %tag, "No digest found, tag may not exist");
            return Ok(());
        };

        let digest_str = digest.to_str().map_err(|e| {
            FlyError::DeserializationError(format!("Invalid digest header: {e}"))
        })?;

        let delete_url = format!(
            "https://registry.fly.io/v2/{}/manifests/{}",
            repository, digest_str
        );

        let response = self
            .client
            .delete(&delete_url)
            .header("Authorization", format!("Bearer {}", self.config.api_token))
            .send()
            .await?;

        if response.status().is_success() || response.status().as_u16() == 404 {
            self.record_success().await;
            tracing::info!(repository = %repository, tag = %tag, "Registry tag deleted");
            Ok(())
        } else {
            self.record_failure().await;
            Err(FlyError::HttpError {
                status: response.status().as_u16(),
                message: response.text().await.unwrap_or_default(),
            })
        }
    }
}
```

### 13. Tenant Deletion - Add cleanup

**File:** `crates/management-api/src/api/tenants/handlers.rs` or dedicated cleanup module

Update tenant deletion to also delete registry tag:

```rust
async fn cleanup_tenant_resources(
    provisioner: &FlyProvisioner,
    tenant: &Tenant,
) {
    if let Some(ref app_name) = tenant.fly_app_name {
        if let Some(ref machine_id) = tenant.fly_machine_id {
            let _ = provisioner.client().delete_machine(app_name, machine_id).await;
        }
        if let Some(ref volume_id) = tenant.fly_volume_id {
            let _ = provisioner.client().delete_volume(app_name, volume_id).await;
        }
        let _ = provisioner.client().delete_app(app_name).await;
    }

    let tag = format!("tenant-{}", tenant.id);
    let _ = provisioner
        .client()
        .delete_registry_tag(&provisioner.config().shared_registry_app, &tag)
        .await;
}
```

### 14. Migration - Remove fly_deploy_token column

**File:** `crates/management-api/migrations/00000014_remove_deploy_token.sql`

```sql
ALTER TABLE management.tenants DROP COLUMN IF EXISTS fly_deploy_token;
```

---

## Implementation Order

1. [ ] Create `systemprompt-images` app on Fly.io
2. [ ] Add `FLY_SHARED_REGISTRY_APP` env var to sandbox/prod
3. [ ] Update `FlyConfig` to add `shared_registry_app`
4. [ ] Update `RegistryTokenResponse` to add `tag` field
5. [ ] Update `get_registry_token` to use org token + shared registry
6. [ ] Update `deploy` handler to validate image tag
7. [ ] Remove `create_deploy_token` from provisioner
8. [ ] Remove `deploy_token` from `ProvisioningResult`, `TenantFlyMetadata`
9. [ ] Remove `fly_deploy_token` from `Tenant` model
10. [ ] Update DB queries to remove `fly_deploy_token`
11. [ ] Add `delete_registry_tag` to FlyClient
12. [ ] Update tenant deletion to cleanup registry tag
13. [ ] Create migration to drop column
14. [ ] Run migration on sandbox DB
15. [ ] Deploy to sandbox, test full flow
16. [ ] Run migration on production DB
17. [ ] Deploy to production

---

## Testing

```bash
# Create tenant via checkout flow
just tenant

# Verify registry token includes tag
curl -H "Authorization: Bearer $TOKEN" \
  https://api-sandbox.systemprompt.io/api/v1/tenants/{id}/registry-token

# Expected response:
{
  "registry": "registry.fly.io",
  "username": "x",
  "repository": "systemprompt-images",
  "tag": "tenant-{id}"
}

# Push image with tenant tag
docker push registry.fly.io/systemprompt-images:tenant-{id}

# Deploy
curl -X POST -H "Authorization: Bearer $TOKEN" \
  -d '{"image": "registry.fly.io/systemprompt-images:tenant-{id}"}' \
  https://api-sandbox.systemprompt.io/api/v1/tenants/{id}/deploy

# Delete tenant - verify registry tag cleaned up
```
