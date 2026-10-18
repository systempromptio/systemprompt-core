//! `GET /bridge/plugins/{id}/archive/{sha256}` — the packed plugin tarball,
//! its immutable caching headers, the in-process memo and the stale-digest
//! conflict.

use std::io::Read;

use axum::body::{Body, to_bytes};
use axum::http::{Request, header};
use http::StatusCode;
use systemprompt_test_fixtures::seed_admin_credential;
use tower::ServiceExt;

use super::routes_bridge_plugin_file::bundle_router_and_pool;

fn authed_get(uri: &str, token: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .expect("request build")
}

async fn current_sha(app: axum::Router, token: &str) -> anyhow::Result<String> {
    let resp = app.oneshot(authed_get("/bridge/manifest", token)).await?;
    anyhow::ensure!(resp.status() == StatusCode::OK, "manifest: {}", resp.status());
    let envelope: serde_json::Value =
        serde_json::from_slice(&to_bytes(resp.into_body(), 4 * 1024 * 1024).await?)?;
    let payload: serde_json::Value = serde_json::from_str(
        envelope["payload"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("payload missing"))?,
    )?;
    payload["plugins"]
        .as_array()
        .and_then(|plugins| {
            plugins
                .iter()
                .find(|p| p["id"].as_str() == Some("cov-plugin"))
        })
        .and_then(|p| p["sha256"].as_str())
        .map(str::to_owned)
        .ok_or_else(|| anyhow::anyhow!("cov-plugin digest missing: {payload}"))
}

fn entry_paths(gz: &[u8]) -> anyhow::Result<Vec<String>> {
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(gz));
    let mut paths = Vec::new();
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.to_string_lossy().into_owned();
        let mut body = String::new();
        entry.read_to_string(&mut body)?;
        if path == "skills/covskill/SKILL.md" {
            anyhow::ensure!(body.contains("Cov skill instructions."), "{body}");
        }
        paths.push(path);
    }
    Ok(paths)
}

#[tokio::test]
async fn archive_for_the_current_digest_is_an_immutable_tarball_of_the_bundle()
-> anyhow::Result<()> {
    let (app, pool) = bundle_router_and_pool().await?;
    let cred = seed_admin_credential(
        &pool,
        &format!("archive-ok-{}@example.invalid", uuid::Uuid::new_v4()),
    )
    .await?;
    let sha = current_sha(app.clone(), cred.jwt.as_str()).await?;
    let uri = format!("/bridge/plugins/cov-plugin/archive/{sha}");

    let first = app.clone().oneshot(authed_get(&uri, cred.jwt.as_str())).await?;
    assert_eq!(first.status(), StatusCode::OK);
    let headers = first.headers().clone();
    assert_eq!(
        headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()),
        Some("application/gzip")
    );
    assert_eq!(
        headers.get(header::CACHE_CONTROL).and_then(|v| v.to_str().ok()),
        Some("private, max-age=31536000, immutable")
    );
    assert_eq!(
        headers.get(header::ETAG).and_then(|v| v.to_str().ok()),
        Some(format!("\"{sha}\"").as_str())
    );
    let first_bytes = to_bytes(first.into_body(), 16 * 1024 * 1024).await?;
    let paths = entry_paths(&first_bytes)?;
    assert!(
        paths.iter().any(|p| p == "skills/covskill/SKILL.md"),
        "{paths:?}"
    );
    assert!(
        paths.iter().any(|p| p == ".claude-plugin/plugin.json"),
        "{paths:?}"
    );

    let upper = format!("/bridge/plugins/cov-plugin/archive/{}", sha.to_uppercase());
    let second = app.oneshot(authed_get(&upper, cred.jwt.as_str())).await?;
    assert_eq!(
        second.status(),
        StatusCode::OK,
        "the digest compares case-insensitively"
    );
    let second_bytes = to_bytes(second.into_body(), 16 * 1024 * 1024).await?;
    assert_eq!(first_bytes, second_bytes, "the memo serves the same packing");
    Ok(())
}

#[tokio::test]
async fn archive_for_a_superseded_digest_is_a_conflict() -> anyhow::Result<()> {
    let (app, pool) = bundle_router_and_pool().await?;
    let cred = seed_admin_credential(
        &pool,
        &format!("archive-stale-{}@example.invalid", uuid::Uuid::new_v4()),
    )
    .await?;
    let stale = "0".repeat(64);
    let resp = app
        .oneshot(authed_get(
            &format!("/bridge/plugins/cov-plugin/archive/{stale}"),
            cred.jwt.as_str(),
        ))
        .await?;
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    let body = String::from_utf8_lossy(&to_bytes(resp.into_body(), 64 * 1024).await?).into_owned();
    assert!(body.contains("stale_plugin_archive"), "{body}");
    Ok(())
}

#[tokio::test]
async fn archive_for_an_unknown_plugin_is_not_found() -> anyhow::Result<()> {
    let (app, pool) = bundle_router_and_pool().await?;
    let cred = seed_admin_credential(
        &pool,
        &format!("archive-miss-{}@example.invalid", uuid::Uuid::new_v4()),
    )
    .await?;
    let resp = app
        .oneshot(authed_get(
            &format!("/bridge/plugins/no-such-plugin/archive/{}", "a".repeat(64)),
            cred.jwt.as_str(),
        ))
        .await?;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    Ok(())
}
