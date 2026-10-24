//! The image factory builds from a catalog hop and `ai.default_image_model`.

use systemprompt_ai::UpstreamTarget;
use systemprompt_ai::services::providers::{ImageProviderFactory, ImageProviderParams};
use systemprompt_manifest::services::catalog::{ResolvedCatalog, ResolvedHop};
use systemprompt_test_fixtures::catalog::CatalogFixture;
use systemprompt_test_fixtures::ensure_test_secrets_bootstrap;
use systemprompt_wire::WireProtocol;

use crate::services::providers::mock_http;

fn hop<'c>(catalog: &'c ResolvedCatalog, model: &str) -> &'c ResolvedHop {
    catalog
        .deployments(model)
        .first()
        .unwrap_or_else(|| panic!("{model} is deployed in the default catalog"))
}

fn create(
    catalog: &ResolvedCatalog,
    model: &str,
) -> systemprompt_ai::error::Result<systemprompt_ai::services::providers::BoxedImageProvider> {
    let hop = hop(catalog, model);
    ImageProviderFactory::create(&ImageProviderParams {
        catalog,
        hop,
        target: UpstreamTarget::resolve(catalog, hop, "test-key").expect("api key resolves"),
    })
}

mod create_tests {
    use super::*;

    #[test]
    fn create_gemini_image_provider_succeeds() {
        let catalog = mock_http::default_catalog();
        let provider = create(&catalog, "gemini-2.5-flash-image").expect("should succeed");
        assert_eq!(provider.name(), "gemini-image");
        assert_eq!(provider.default_model(), "gemini-2.5-flash-image");
    }

    #[test]
    fn create_openai_image_provider_succeeds() {
        let catalog = mock_http::default_catalog();
        let provider = create(&catalog, "gpt-image-1").expect("should succeed");
        assert_eq!(provider.name(), "openai-image");
        assert_eq!(provider.default_model(), "gpt-image-1");
    }

    #[test]
    fn create_fails_when_the_provider_is_disabled() {
        let catalog = CatalogFixture::embedded_default()
            .with_provider("gemini", |p| p.enabled = false)
            .build();
        let error = create(&catalog, "gemini-2.5-flash-image")
            .err()
            .expect("should be an error");
        assert!(error.to_string().contains("disabled"), "{error}");
    }

    #[test]
    fn anthropic_protocol_does_not_support_images() {
        let catalog = mock_http::default_catalog();
        let error = create(&catalog, "claude-sonnet-5")
            .err()
            .expect("should be an error");
        assert!(
            error
                .to_string()
                .contains("does not support image generation")
        );
    }

    #[test]
    fn the_default_image_model_is_the_providers_default() {
        let catalog = mock_http::default_catalog();
        let provider = create(&catalog, "gemini-3-pro-image-preview").expect("should succeed");
        assert_eq!(provider.default_model(), "gemini-3-pro-image-preview");
    }
}

mod from_catalog_tests {
    use super::*;

    #[test]
    fn no_default_image_model_means_no_provider() {
        let catalog = mock_http::default_catalog();
        assert!(
            ImageProviderFactory::from_catalog(&catalog)
                .expect("no error")
                .is_none()
        );
    }

    #[test]
    fn the_default_image_model_picks_its_first_enabled_deployment() {
        ensure_test_secrets_bootstrap();
        let catalog = CatalogFixture::embedded_default()
            .default_image_model("gpt-image-1")
            .build();
        let default = ImageProviderFactory::from_catalog(&catalog)
            .expect("builds")
            .expect("a default image model yields a provider");
        assert_eq!(default.provider, "openai");
        assert_eq!(default.client.default_model(), "gpt-image-1");
    }

    #[test]
    fn a_default_image_model_on_a_disabled_provider_is_refused() {
        ensure_test_secrets_bootstrap();
        let catalog = CatalogFixture::embedded_default()
            .with_provider("openai", |p| p.enabled = false)
            .default_image_model("gpt-image-1")
            .build();
        let err = ImageProviderFactory::from_catalog(&catalog).unwrap_err();
        assert!(err.to_string().contains("gpt-image-1"), "{err}");
    }
}

mod supports_image_generation_tests {
    use super::*;

    #[test]
    fn image_protocols_supported() {
        assert!(ImageProviderFactory::supports_image_generation(
            WireProtocol::Gemini
        ));
        assert!(ImageProviderFactory::supports_image_generation(
            WireProtocol::OpenAiChat
        ));
        assert!(ImageProviderFactory::supports_image_generation(
            WireProtocol::OpenAiResponses
        ));
    }

    #[test]
    fn anthropic_not_supported() {
        assert!(!ImageProviderFactory::supports_image_generation(
            WireProtocol::Anthropic
        ));
    }
}
