use std::sync::Arc;

use systemprompt_manifest::Config;
use systemprompt_marketplace::MarketplaceFilter;
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{
    TestBootstrap, fixture_admitted, fixture_context_builder,
};

pub(crate) async fn context_on(
    boot: &TestBootstrap,
    config: Config,
    filter: Arc<dyn MarketplaceFilter>,
) -> Arc<AppContext> {
    let builder = fixture_context_builder(boot, config, fixture_admitted())
        .expect("fixture context builder over the bootstrap");
    Arc::new(
        builder
            .with_marketplace_filter(filter)
            .build()
            .await
            .expect("fixture context over the bootstrap"),
    )
}
