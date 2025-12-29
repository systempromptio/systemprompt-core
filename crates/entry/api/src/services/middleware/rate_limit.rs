use crate::services::middleware::context::{ContextExtractor, ContextMiddleware};
use axum::Router;
use systemprompt_models::config::RateLimitConfig;
use tower_governor::key_extractor::SmartIpKeyExtractor;
use tracing::warn;

pub trait RouterExt<S> {
    fn with_rate_limit(self, rate_config: &RateLimitConfig, per_second: u64) -> Self;
    fn with_auth_middleware<E>(self, middleware: ContextMiddleware<E>) -> Self
    where
        E: ContextExtractor + Clone + Send + Sync + 'static;
}

impl<S> RouterExt<S> for Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn with_rate_limit(self, rate_config: &RateLimitConfig, per_second: u64) -> Self {
        if rate_config.disabled {
            return self;
        }

        let rate_limit_result = tower_governor::governor::GovernorConfigBuilder::default()
            .per_second(per_second)
            .burst_size((per_second * rate_config.burst_multiplier) as u32)
            .key_extractor(SmartIpKeyExtractor)
            .use_headers()
            .finish();

        if let Some(rate_limit) = rate_limit_result {
            self.layer(tower_governor::GovernorLayer::new(rate_limit))
        } else {
            warn!("Failed to configure rate limiting - rate limiting disabled for this route");
            self
        }
    }

    fn with_auth_middleware<E>(self, middleware: ContextMiddleware<E>) -> Self
    where
        E: ContextExtractor + Clone + Send + Sync + 'static,
    {
        self.layer(axum::middleware::from_fn(move |req, next| {
            let middleware = middleware.clone();
            async move { middleware.handle(req, next).await }
        }))
    }
}
