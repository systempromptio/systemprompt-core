//! Smoke tests for generator public entry points that only need config
//! singletons (not a full WebConfig fixture). These hit the sitemap
//! provider's async constructor and `generate_feed_with_providers`'
//! `Config::get()` path. Tests that need a full `web/config.yaml` are
//! left to integration suites.

use systemprompt_generator::{
    DefaultSitemapProvider, generate_feed_with_providers,
};
use systemprompt_test_fixtures::ensure_test_bootstrap;

#[tokio::test]
async fn generate_feed_with_providers_empty_slice_errors() {
    let _ = ensure_test_bootstrap();
    let r = generate_feed_with_providers(&[]).await;
    assert!(r.is_err(), "expected error when no providers registered");
}

#[tokio::test]
async fn default_sitemap_provider_async_new_reads_bootstrap_config() {
    let boot = ensure_test_bootstrap();
    let p = DefaultSitemapProvider::new(&boot.app_paths).await.unwrap();
    let _ = format!("{p:?}");
}

#[tokio::test]
async fn default_sitemap_provider_provider_id_stable() {
    use systemprompt_provider_contracts::SitemapProvider;
    let boot = ensure_test_bootstrap();
    let p = DefaultSitemapProvider::new(&boot.app_paths).await.unwrap();
    assert_eq!(p.provider_id(), "default-sitemap");
}
