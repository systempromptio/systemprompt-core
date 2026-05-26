//! Drives `generate_feed_with_providers` against an in-memory mock provider
//! to cover the cross-provider aggregation, channel-from-metadata
//! construction, XML emission, and empty-feed error path in
//! `rss/generator.rs`.

use std::path::PathBuf;
use std::sync::{Arc, Once};

use async_trait::async_trait;
use chrono::{TimeZone, Utc};
use systemprompt_generator::generate_feed_with_providers;
use systemprompt_identifiers::SourceId;
use systemprompt_models::Config;
use systemprompt_models::auth::JwtAudience;
use systemprompt_models::config::RateLimitConfig;
use systemprompt_provider_contracts::{
    ProviderResult, RssFeedContext, RssFeedItem, RssFeedMetadata, RssFeedProvider, RssFeedSpec,
};

static CONFIG_INIT: Once = Once::new();

fn install_test_config() {
    CONFIG_INIT.call_once(|| {
        let _ = Config::install(Config {
            instance_id: "test-instance".to_owned(),
            max_concurrent_streams: 256,
            sitename: "test".to_owned(),
            database_type: "postgres".to_owned(),
            database_url: String::new(),
            database_write_url: None,
            github_link: String::new(),
            github_token: None,
            system_path: String::new(),
            services_path: String::new(),
            bin_path: String::new(),
            skills_path: String::new(),
            settings_path: String::new(),
            content_config_path: String::new(),
            geoip_database_path: None,
            web_path: String::new(),
            web_config_path: String::new(),
            web_metadata_path: String::new(),
            host: "127.0.0.1".to_owned(),
            port: 8080,
            api_server_url: "http://127.0.0.1:8080".to_owned(),
            api_internal_url: "http://127.0.0.1:8080".to_owned(),
            api_external_url: "https://example.test".to_owned(),
            jwt_issuer: "https://test.invalid".to_owned(),
            jwt_access_token_expiration: 3600,
            jwt_refresh_token_expiration: 604_800,
            jwt_audiences: vec![JwtAudience::Bridge],
            allowed_resource_audiences: Vec::new(),
            trusted_issuers: Vec::new(),
            signing_key_path: PathBuf::new(),
            use_https: false,
            rate_limits: RateLimitConfig::default(),
            cors_allowed_origins: Vec::new(),
            trusted_proxies: Vec::new(),
            is_cloud: false,
            content_negotiation: Default::default(),
            security_headers: Default::default(),
            allow_registration: false,
            system_admin_username: "admin".to_owned(),
        });
    });
}

struct MockProvider {
    id: &'static str,
    specs: Vec<RssFeedSpec>,
    items_per_feed: usize,
}

#[async_trait]
impl RssFeedProvider for MockProvider {
    fn provider_id(&self) -> &'static str {
        self.id
    }

    fn feed_specs(&self) -> Vec<RssFeedSpec> {
        self.specs.clone()
    }

    async fn feed_metadata(&self, ctx: &RssFeedContext<'_>) -> ProviderResult<RssFeedMetadata> {
        Ok(RssFeedMetadata {
            title: format!("Feed for {}", ctx.source_name),
            link: format!("{}/feed/{}", ctx.base_url, ctx.source_name),
            description: format!("Description for {}", ctx.source_name),
            language: Some("en".to_owned()),
        })
    }

    async fn fetch_items(
        &self,
        ctx: &RssFeedContext<'_>,
        _limit: i64,
    ) -> ProviderResult<Vec<RssFeedItem>> {
        let items = (0..self.items_per_feed)
            .map(|i| RssFeedItem {
                title: format!("Item {i}"),
                link: format!("{}/item/{i}", ctx.base_url),
                description: format!("Item {i} description"),
                pub_date: Utc.with_ymd_and_hms(2026, 5, 26, 12, 0, 0).unwrap(),
                guid: format!("guid-{i}"),
                author: Some(format!("author-{i}")),
            })
            .collect();
        Ok(items)
    }
}

#[tokio::test]
async fn generate_feed_with_single_provider_emits_xml() {
    install_test_config();

    let provider = Arc::new(MockProvider {
        id: "mock-1",
        specs: vec![RssFeedSpec {
            source_id: SourceId::new("blog"),
            max_items: 10,
            output_filename: "blog.xml".to_owned(),
        }],
        items_per_feed: 3,
    });
    let providers: Vec<Arc<dyn RssFeedProvider>> = vec![provider];

    let feeds = generate_feed_with_providers(&providers)
        .await
        .expect("generate_feed_with_providers must succeed");

    assert_eq!(feeds.len(), 1);
    let feed = &feeds[0];
    assert_eq!(feed.filename, "blog.xml");
    assert_eq!(feed.item_count, 3);
    assert!(feed.xml.contains("<rss"));
    assert!(feed.xml.contains("<channel>"));
    assert!(feed.xml.contains("Item 0"));
    assert!(feed.xml.contains("Item 2"));
}

#[tokio::test]
async fn generate_feed_with_multiple_specs_and_providers() {
    install_test_config();

    let p1 = Arc::new(MockProvider {
        id: "mock-a",
        specs: vec![
            RssFeedSpec {
                source_id: SourceId::new("posts"),
                max_items: 5,
                output_filename: "posts.xml".to_owned(),
            },
            RssFeedSpec {
                source_id: SourceId::new("news"),
                max_items: 5,
                output_filename: "news.xml".to_owned(),
            },
        ],
        items_per_feed: 2,
    });
    let p2 = Arc::new(MockProvider {
        id: "mock-b",
        specs: vec![RssFeedSpec {
            source_id: SourceId::new("changelog"),
            max_items: 5,
            output_filename: "changelog.xml".to_owned(),
        }],
        items_per_feed: 0,
    });
    let providers: Vec<Arc<dyn RssFeedProvider>> = vec![p1, p2];

    let feeds = generate_feed_with_providers(&providers)
        .await
        .expect("multi-provider generation must succeed");

    assert_eq!(feeds.len(), 3, "three feed specs across two providers");
    let names: Vec<_> = feeds.iter().map(|f| f.filename.as_str()).collect();
    assert!(names.contains(&"posts.xml"));
    assert!(names.contains(&"news.xml"));
    assert!(names.contains(&"changelog.xml"));

    let changelog = feeds
        .iter()
        .find(|f| f.filename == "changelog.xml")
        .expect("changelog feed");
    assert_eq!(changelog.item_count, 0);
}

#[tokio::test]
async fn generate_feed_errors_when_no_providers() {
    install_test_config();

    let providers: Vec<Arc<dyn RssFeedProvider>> = Vec::new();
    let err = generate_feed_with_providers(&providers)
        .await
        .expect_err("no providers must error");
    assert!(err.to_string().to_lowercase().contains("rss"));
}

#[tokio::test]
async fn generate_feed_errors_when_provider_yields_no_specs() {
    install_test_config();

    let provider = Arc::new(MockProvider {
        id: "mock-empty",
        specs: Vec::new(),
        items_per_feed: 0,
    });
    let providers: Vec<Arc<dyn RssFeedProvider>> = vec![provider];

    let err = generate_feed_with_providers(&providers)
        .await
        .expect_err("provider returning no specs must error");
    assert!(err.to_string().to_lowercase().contains("rss"));
}
