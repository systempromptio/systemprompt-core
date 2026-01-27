use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HomepageConfig {
    pub hero: HeroConfig,
    pub integrations: IntegrationsConfig,
    pub features: FeaturesConfig,
    pub how_it_works: HowItWorksConfig,
    pub use_cases: UseCasesConfig,
    pub technical: TechnicalConfig,
    pub comparison: ComparisonConfig,
    pub pricing: PricingConfig,
    pub faq: FaqConfig,
    pub final_cta: FinalCtaConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeroConfig {
    pub title: String,
    pub subtitle: String,
    pub cta: String,
    pub cta_secondary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationsConfig {
    pub label: String,
    pub brands: Vec<IntegrationBrand>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationBrand {
    pub name: String,
    pub logo: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeaturesConfig {
    pub categories: Vec<FeatureCategory>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureCategory {
    pub name: String,
    pub features: Vec<Feature>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feature {
    pub title: String,
    pub description: String,
    pub icon: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HowItWorksConfig {
    pub title: String,
    pub steps: Vec<HowItWorksStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HowItWorksStep {
    pub number: String,
    pub title: String,
    pub description: String,
    pub code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UseCasesConfig {
    pub title: String,
    pub cases: Vec<UseCase>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UseCase {
    pub title: String,
    pub description: String,
    pub icon: String,
    pub code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechnicalConfig {
    pub title: String,
    pub subtitle: String,
    pub specs: Vec<TechnicalSpec>,
    pub extension_code: String,
    pub cta: String,
    pub cta_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechnicalSpec {
    pub title: String,
    pub description: String,
    pub icon: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonConfig {
    pub title: String,
    pub superagent: ComparisonSide,
    pub harness: ComparisonSide,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonSide {
    pub title: String,
    pub items: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricingConfig {
    pub title: String,
    pub tiers: Vec<PricingTier>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricingTier {
    pub name: String,
    pub price: String,
    pub description: String,
    pub features: Vec<String>,
    pub cta: String,
    pub cta_url: String,
    #[serde(default)]
    pub highlight: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaqConfig {
    pub title: String,
    pub items: Vec<FaqItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaqItem {
    pub question: String,
    pub answer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinalCtaConfig {
    pub title: String,
    pub subtitle: String,
    pub button: String,
}
