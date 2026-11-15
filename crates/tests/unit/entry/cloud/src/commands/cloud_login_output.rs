//! Tests for the cloud login output projection.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::Path;

use systemprompt_cli::cloud::auth::build_login_output;
use systemprompt_models::api::cloud::UserMeResponse;

fn response(json: serde_json::Value) -> UserMeResponse {
    serde_json::from_value(json).unwrap()
}

#[test]
fn minimal_response_maps_user_and_paths() {
    let response = response(serde_json::json!({
        "user": {"id": "u1", "email": "a@b.test", "name": null}
    }));

    let out = build_login_output(
        &response,
        Path::new("/home/x/.sp/credentials.json"),
        Path::new("/home/x/.sp/tenants.json"),
    );

    assert_eq!(out.user.id, "u1");
    assert_eq!(out.user.email, "a@b.test");
    assert!(out.user.name.is_none());
    assert!(out.customer.is_none());
    assert!(out.tenants.is_empty());
    assert_eq!(out.credentials_path, "/home/x/.sp/credentials.json");
    assert_eq!(out.tenants_path, "/home/x/.sp/tenants.json");
}

#[test]
fn full_response_maps_customer_tenants_and_plan() {
    let response = response(serde_json::json!({
        "user": {"id": "u1", "email": "a@b.test", "name": "Ada"},
        "customer": {"id": "cus_9"},
        "tenants": [{
            "id": "t1",
            "name": "prod",
            "database_url": "postgres://localhost/prod",
            "region": "lhr",
            "hostname": "prod.example.com",
            "plan": {"name": "launch", "memory_mb": 512, "volume_gb": 3}
        }]
    }));

    let out = build_login_output(&response, Path::new("/c"), Path::new("/t"));

    assert_eq!(out.user.name.as_deref(), Some("Ada"));
    assert_eq!(out.customer.unwrap().id, "cus_9");
    assert_eq!(out.tenants.len(), 1);
    let tenant = &out.tenants[0];
    assert_eq!(tenant.name, "prod");
    assert_eq!(tenant.region.as_deref(), Some("lhr"));
    assert_eq!(tenant.hostname.as_deref(), Some("prod.example.com"));
    let plan = tenant.plan.as_ref().unwrap();
    assert_eq!(plan.name, "launch");
    assert_eq!(plan.memory_mb, 512);
    assert_eq!(plan.volume_gb, 3);
}
