use crate::{TemplateDefinition, TemplateProvider};

#[derive(Debug, Clone, Copy, Default)]
pub struct EmbeddedDefaultsProvider;

impl EmbeddedDefaultsProvider {
    pub const PRIORITY: u32 = 1000;
}

impl TemplateProvider for EmbeddedDefaultsProvider {
    fn provider_id(&self) -> &'static str {
        "embedded-defaults"
    }

    fn priority(&self) -> u32 {
        Self::PRIORITY
    }

    fn templates(&self) -> Vec<TemplateDefinition> {
        vec![TemplateDefinition::embedded(
            "homepage",
            include_str!("../defaults/templates/homepage.html"),
        )
        .with_priority(Self::PRIORITY)
        .for_content_type("homepage")]
    }
}
