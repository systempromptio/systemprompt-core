//! Unit tests for API client types
//!
//! Tests cover:
//! - CloudApiClient creation and accessors
//! - SubscriptionStatus enum variants and serialization
//! - TenantInfo, TenantSecrets, UserInfo structures
//! - Plan, RegistryToken, DeployResponse, CheckoutResponse
//! - ProvisioningEventType, ProvisioningEvent
//! - Tenant, TenantStatus types

use systemprompt_cloud::{
    CheckoutResponse, CloudApiClient, DeployResponse, Plan, ProvisioningEvent,
    ProvisioningEventType, RegistryToken, SubscriptionStatus, Tenant, TenantInfo, TenantSecrets,
    TenantStatus, UserInfo, UserMeResponse,
};

// ============================================================================
// CloudApiClient Tests
// ============================================================================

#[test]
fn test_cloud_api_client_new() {
    let client = CloudApiClient::new("https://api.test.com", "test_token_123");

    assert_eq!(client.api_url(), "https://api.test.com");
    assert_eq!(client.token(), "test_token_123");
}

#[test]
fn test_cloud_api_client_with_trailing_slash() {
    let client = CloudApiClient::new("https://api.test.com/", "token");
    assert_eq!(client.api_url(), "https://api.test.com/");
}

#[test]
fn test_cloud_api_client_empty_url() {
    let client = CloudApiClient::new("", "token");
    assert_eq!(client.api_url(), "");
}

#[test]
fn test_cloud_api_client_empty_token() {
    let client = CloudApiClient::new("https://api.test.com", "");
    assert_eq!(client.token(), "");
}

#[test]
fn test_cloud_api_client_debug() {
    let client = CloudApiClient::new("https://api.test.com", "token");
    let debug_str = format!("{:?}", client);
    assert!(debug_str.contains("CloudApiClient"));
}

// ============================================================================
// SubscriptionStatus Tests
// ============================================================================

#[test]
fn test_subscription_status_deserialization() {
    let active: SubscriptionStatus = serde_json::from_str("\"active\"").unwrap();
    assert!(matches!(active, SubscriptionStatus::Active));

    let trialing: SubscriptionStatus = serde_json::from_str("\"trialing\"").unwrap();
    assert!(matches!(trialing, SubscriptionStatus::Trialing));

    let past_due: SubscriptionStatus = serde_json::from_str("\"past_due\"").unwrap();
    assert!(matches!(past_due, SubscriptionStatus::PastDue));

    let paused: SubscriptionStatus = serde_json::from_str("\"paused\"").unwrap();
    assert!(matches!(paused, SubscriptionStatus::Paused));

    let canceled: SubscriptionStatus = serde_json::from_str("\"canceled\"").unwrap();
    assert!(matches!(canceled, SubscriptionStatus::Canceled));
}

#[test]
fn test_subscription_status_debug() {
    let status = SubscriptionStatus::Active;
    let debug_str = format!("{:?}", status);
    assert!(debug_str.contains("Active"));
}

#[test]
fn test_subscription_status_clone() {
    let original = SubscriptionStatus::Trialing;
    let cloned = original.clone();
    assert!(matches!(cloned, SubscriptionStatus::Trialing));
}

#[test]
fn test_subscription_status_copy() {
    let original = SubscriptionStatus::PastDue;
    let copied = original;
    assert!(matches!(original, SubscriptionStatus::PastDue));
    assert!(matches!(copied, SubscriptionStatus::PastDue));
}

// ============================================================================
// UserInfo Tests
// ============================================================================

#[test]
fn test_user_info_deserialization() {
    let json = r#"{"id": "user-123", "email": "test@example.com"}"#;
    let user: UserInfo = serde_json::from_str(json).unwrap();

    assert_eq!(user.id, "user-123");
    assert_eq!(user.email, "test@example.com");
    assert!(user.name.is_none());
}

#[test]
fn test_user_info_with_name() {
    let json = r#"{"id": "user-123", "email": "test@example.com", "name": "Test User"}"#;
    let user: UserInfo = serde_json::from_str(json).unwrap();

    assert_eq!(user.name, Some("Test User".to_string()));
}

#[test]
fn test_user_info_debug() {
    let json = r#"{"id": "user-123", "email": "test@example.com"}"#;
    let user: UserInfo = serde_json::from_str(json).unwrap();
    let debug_str = format!("{:?}", user);
    assert!(debug_str.contains("UserInfo"));
}

// ============================================================================
// TenantInfo Tests
// ============================================================================

#[test]
fn test_tenant_info_minimal() {
    let json = r#"{"id": "tenant-123", "name": "My Tenant"}"#;
    let tenant: TenantInfo = serde_json::from_str(json).unwrap();

    assert_eq!(tenant.id, "tenant-123");
    assert_eq!(tenant.name, "My Tenant");
    assert!(tenant.subscription_id.is_none());
    assert!(tenant.subscription_status.is_none());
    assert!(tenant.app_id.is_none());
    assert!(tenant.hostname.is_none());
    assert!(tenant.region.is_none());
    assert!(tenant.plan.is_none());
}

#[test]
fn test_tenant_info_full() {
    let json = r#"{
        "id": "tenant-123",
        "name": "Full Tenant",
        "subscription_id": "sub-456",
        "subscription_status": "active",
        "app_id": "app-789",
        "hostname": "tenant.example.com",
        "region": "iad",
        "plan": {"name": "Pro", "memory_mb": 512, "volume_gb": 10}
    }"#;
    let tenant: TenantInfo = serde_json::from_str(json).unwrap();

    assert_eq!(tenant.id, "tenant-123");
    assert_eq!(tenant.subscription_id, Some("sub-456".to_string()));
    assert!(tenant.subscription_status.is_some());
    assert_eq!(tenant.app_id, Some("app-789".to_string()));
    assert_eq!(tenant.hostname, Some("tenant.example.com".to_string()));
    assert_eq!(tenant.region, Some("iad".to_string()));
    assert!(tenant.plan.is_some());
}

#[test]
fn test_tenant_info_clone() {
    let json = r#"{"id": "tenant-123", "name": "My Tenant"}"#;
    let original: TenantInfo = serde_json::from_str(json).unwrap();
    let cloned = original.clone();

    assert_eq!(cloned.id, original.id);
    assert_eq!(cloned.name, original.name);
}

// ============================================================================
// TenantSecrets Tests
// ============================================================================

#[test]
fn test_tenant_secrets_minimal() {
    let json = r#"{
        "jwt_secret": "secret123",
        "database_url": "postgres://localhost/db",
        "app_url": "https://app.example.com"
    }"#;
    let secrets: TenantSecrets = serde_json::from_str(json).unwrap();

    assert_eq!(secrets.jwt_secret, "secret123");
    assert_eq!(secrets.database_url, "postgres://localhost/db");
    assert_eq!(secrets.app_url, "https://app.example.com");
    assert!(secrets.anthropic_api_key.is_none());
    assert!(secrets.openai_api_key.is_none());
    assert!(secrets.gemini_api_key.is_none());
}

#[test]
fn test_tenant_secrets_with_api_keys() {
    let json = r#"{
        "jwt_secret": "secret",
        "database_url": "postgres://db",
        "app_url": "https://app.com",
        "anthropic_api_key": "anthropic_key",
        "openai_api_key": "openai_key",
        "gemini_api_key": "gemini_key"
    }"#;
    let secrets: TenantSecrets = serde_json::from_str(json).unwrap();

    assert_eq!(secrets.anthropic_api_key, Some("anthropic_key".to_string()));
    assert_eq!(secrets.openai_api_key, Some("openai_key".to_string()));
    assert_eq!(secrets.gemini_api_key, Some("gemini_key".to_string()));
}

#[test]
fn test_tenant_secrets_serialization_skips_none() {
    let secrets = TenantSecrets {
        jwt_secret: "secret".to_string(),
        database_url: "postgres://db".to_string(),
        app_url: "https://app.com".to_string(),
        anthropic_api_key: None,
        openai_api_key: None,
        gemini_api_key: None,
    };

    let json = serde_json::to_string(&secrets).unwrap();
    assert!(!json.contains("anthropic_api_key"));
    assert!(!json.contains("openai_api_key"));
    assert!(!json.contains("gemini_api_key"));
}

#[test]
fn test_tenant_secrets_clone() {
    let original = TenantSecrets {
        jwt_secret: "secret".to_string(),
        database_url: "postgres://db".to_string(),
        app_url: "https://app.com".to_string(),
        anthropic_api_key: Some("key".to_string()),
        openai_api_key: None,
        gemini_api_key: None,
    };

    let cloned = original.clone();
    assert_eq!(cloned.jwt_secret, original.jwt_secret);
    assert_eq!(cloned.anthropic_api_key, original.anthropic_api_key);
}

// ============================================================================
// UserMeResponse Tests
// ============================================================================

#[test]
fn test_user_me_response_minimal() {
    let json = r#"{
        "user": {"id": "user-1", "email": "user@test.com"}
    }"#;
    let response: UserMeResponse = serde_json::from_str(json).unwrap();

    assert_eq!(response.user.id, "user-1");
    assert!(response.customer.is_none());
    assert!(response.tenants.is_empty());
}

#[test]
fn test_user_me_response_with_tenants() {
    let json = r#"{
        "user": {"id": "user-1", "email": "user@test.com"},
        "tenants": [
            {"id": "t1", "name": "Tenant 1"},
            {"id": "t2", "name": "Tenant 2"}
        ]
    }"#;
    let response: UserMeResponse = serde_json::from_str(json).unwrap();

    assert_eq!(response.tenants.len(), 2);
    assert_eq!(response.tenants[0].id, "t1");
    assert_eq!(response.tenants[1].id, "t2");
}

#[test]
fn test_user_me_response_with_customer() {
    let json = r#"{
        "user": {"id": "user-1", "email": "user@test.com"},
        "customer": {"id": "cus-123"}
    }"#;
    let response: UserMeResponse = serde_json::from_str(json).unwrap();

    assert!(response.customer.is_some());
    assert_eq!(response.customer.unwrap().id, "cus-123");
}

// ============================================================================
// Tenant Tests
// ============================================================================

#[test]
fn test_tenant_minimal() {
    let json = r#"{"id": "t-123", "name": "Test Tenant"}"#;
    let tenant: Tenant = serde_json::from_str(json).unwrap();

    assert_eq!(tenant.id, "t-123");
    assert_eq!(tenant.name, "Test Tenant");
    assert!(tenant.fly_app_name.is_none());
    assert!(tenant.fly_hostname.is_none());
}

#[test]
fn test_tenant_with_fly_info() {
    let json = r#"{
        "id": "t-123",
        "name": "Test Tenant",
        "fly_app_name": "my-fly-app",
        "fly_hostname": "my-fly-app.fly.dev"
    }"#;
    let tenant: Tenant = serde_json::from_str(json).unwrap();

    assert_eq!(tenant.fly_app_name, Some("my-fly-app".to_string()));
    assert_eq!(tenant.fly_hostname, Some("my-fly-app.fly.dev".to_string()));
}

// ============================================================================
// TenantStatus Tests
// ============================================================================

#[test]
fn test_tenant_status_minimal() {
    let json = r#"{"status": "running"}"#;
    let status: TenantStatus = serde_json::from_str(json).unwrap();

    assert_eq!(status.status, "running");
    assert!(status.message.is_none());
    assert!(status.app_url.is_none());
    assert!(status.secrets_url.is_none());
}

#[test]
fn test_tenant_status_full() {
    let json = r#"{
        "status": "ready",
        "message": "Deployment complete",
        "app_url": "https://app.example.com",
        "secrets_url": "/api/v1/tenants/t-123/secrets"
    }"#;
    let status: TenantStatus = serde_json::from_str(json).unwrap();

    assert_eq!(status.status, "ready");
    assert_eq!(status.message, Some("Deployment complete".to_string()));
    assert_eq!(status.app_url, Some("https://app.example.com".to_string()));
    assert_eq!(status.secrets_url, Some("/api/v1/tenants/t-123/secrets".to_string()));
}

// ============================================================================
// Plan Tests
// ============================================================================

#[test]
fn test_plan_deserialization() {
    let json = r#"{
        "id": "plan-pro",
        "name": "Pro",
        "paddle_price_id": "pri_123",
        "memory_mb_default": 512,
        "volume_gb": 10
    }"#;
    let plan: Plan = serde_json::from_str(json).unwrap();

    assert_eq!(plan.id, "plan-pro");
    assert_eq!(plan.name, "Pro");
    assert_eq!(plan.paddle_price_id, "pri_123");
    assert_eq!(plan.memory_mb_default, 512);
    assert_eq!(plan.volume_gb, 10);
}

#[test]
fn test_plan_with_max_tenants() {
    let json = r#"{
        "id": "plan-starter",
        "name": "Starter",
        "paddle_price_id": "pri_456",
        "max_tenants": 5
    }"#;
    let plan: Plan = serde_json::from_str(json).unwrap();

    assert_eq!(plan.max_tenants, Some(5));
}

#[test]
fn test_plan_clone() {
    let json = r#"{"id": "plan-1", "name": "Test", "paddle_price_id": "pri_1"}"#;
    let original: Plan = serde_json::from_str(json).unwrap();
    let cloned = original.clone();

    assert_eq!(cloned.id, original.id);
}

// ============================================================================
// RegistryToken Tests
// ============================================================================

#[test]
fn test_registry_token_deserialization() {
    let json = r#"{
        "registry": "registry.fly.io",
        "username": "x",
        "token": "token123",
        "repository": "systemprompt-images",
        "tag": "tenant-abc"
    }"#;
    let token: RegistryToken = serde_json::from_str(json).unwrap();

    assert_eq!(token.registry, "registry.fly.io");
    assert_eq!(token.username, "x");
    assert_eq!(token.token, "token123");
    assert_eq!(token.repository, "systemprompt-images");
    assert_eq!(token.tag, "tenant-abc");
}

#[test]
fn test_registry_token_debug() {
    let json = r#"{"registry": "r", "username": "u", "token": "t", "repository": "repo", "tag": "tag"}"#;
    let token: RegistryToken = serde_json::from_str(json).unwrap();
    let debug_str = format!("{:?}", token);
    assert!(debug_str.contains("RegistryToken"));
}

// ============================================================================
// DeployResponse Tests
// ============================================================================

#[test]
fn test_deploy_response_minimal() {
    let json = r#"{"status": "deployed"}"#;
    let response: DeployResponse = serde_json::from_str(json).unwrap();

    assert_eq!(response.status, "deployed");
    assert!(response.app_url.is_none());
}

#[test]
fn test_deploy_response_with_url() {
    let json = r#"{"status": "deployed", "app_url": "https://app.fly.dev"}"#;
    let response: DeployResponse = serde_json::from_str(json).unwrap();

    assert_eq!(response.app_url, Some("https://app.fly.dev".to_string()));
}

// ============================================================================
// CheckoutResponse Tests
// ============================================================================

#[test]
fn test_checkout_response_deserialization() {
    let json = r#"{
        "checkout_url": "https://checkout.paddle.com/checkout/123",
        "transaction_id": "txn-abc-123"
    }"#;
    let response: CheckoutResponse = serde_json::from_str(json).unwrap();

    assert_eq!(response.checkout_url, "https://checkout.paddle.com/checkout/123");
    assert_eq!(response.transaction_id, "txn-abc-123");
}

// ============================================================================
// ProvisioningEventType Tests
// ============================================================================

#[test]
fn test_provisioning_event_type_deserialization() {
    let tenant_created: ProvisioningEventType = serde_json::from_str("\"tenant_created\"").unwrap();
    assert!(matches!(tenant_created, ProvisioningEventType::TenantCreated));

    let vm_started: ProvisioningEventType = serde_json::from_str("\"vm_provisioning_started\"").unwrap();
    assert!(matches!(vm_started, ProvisioningEventType::VmProvisioningStarted));

    let vm_progress: ProvisioningEventType = serde_json::from_str("\"vm_provisioning_progress\"").unwrap();
    assert!(matches!(vm_progress, ProvisioningEventType::VmProvisioningProgress));

    let vm_provisioned: ProvisioningEventType = serde_json::from_str("\"vm_provisioned\"").unwrap();
    assert!(matches!(vm_provisioned, ProvisioningEventType::VmProvisioned));

    let ready: ProvisioningEventType = serde_json::from_str("\"tenant_ready\"").unwrap();
    assert!(matches!(ready, ProvisioningEventType::TenantReady));

    let failed: ProvisioningEventType = serde_json::from_str("\"provisioning_failed\"").unwrap();
    assert!(matches!(failed, ProvisioningEventType::ProvisioningFailed));
}

#[test]
fn test_provisioning_event_type_clone() {
    let original = ProvisioningEventType::TenantReady;
    let cloned = original.clone();
    assert!(matches!(cloned, ProvisioningEventType::TenantReady));
}

#[test]
fn test_provisioning_event_type_copy() {
    let original = ProvisioningEventType::VmProvisioned;
    let copied = original;
    assert!(matches!(original, ProvisioningEventType::VmProvisioned));
    assert!(matches!(copied, ProvisioningEventType::VmProvisioned));
}

// ============================================================================
// ProvisioningEvent Tests
// ============================================================================

#[test]
fn test_provisioning_event_minimal() {
    let json = r#"{
        "tenant_id": "t-123",
        "event_type": "tenant_created",
        "status": "created"
    }"#;
    let event: ProvisioningEvent = serde_json::from_str(json).unwrap();

    assert_eq!(event.tenant_id, "t-123");
    assert!(matches!(event.event_type, ProvisioningEventType::TenantCreated));
    assert_eq!(event.status, "created");
    assert!(event.message.is_none());
    assert!(event.app_url.is_none());
}

#[test]
fn test_provisioning_event_full() {
    let json = r#"{
        "tenant_id": "t-456",
        "event_type": "tenant_ready",
        "status": "ready",
        "message": "Tenant is ready for use",
        "app_url": "https://t-456.fly.dev"
    }"#;
    let event: ProvisioningEvent = serde_json::from_str(json).unwrap();

    assert_eq!(event.tenant_id, "t-456");
    assert!(matches!(event.event_type, ProvisioningEventType::TenantReady));
    assert_eq!(event.message, Some("Tenant is ready for use".to_string()));
    assert_eq!(event.app_url, Some("https://t-456.fly.dev".to_string()));
}

#[test]
fn test_provisioning_event_clone() {
    let json = r#"{"tenant_id": "t-1", "event_type": "tenant_created", "status": "ok"}"#;
    let original: ProvisioningEvent = serde_json::from_str(json).unwrap();
    let cloned = original.clone();

    assert_eq!(cloned.tenant_id, original.tenant_id);
    assert_eq!(cloned.status, original.status);
}
