use serde::{Deserialize, Serialize};
use systemprompt_identifiers::LocaleCode;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SiteI18nConfig {
    pub default_locale: LocaleCode,
    pub supported_locales: Vec<LocaleCode>,
}

impl Default for SiteI18nConfig {
    fn default() -> Self {
        let default_locale = LocaleCode::new("en");
        Self {
            supported_locales: vec![default_locale.clone()],
            default_locale,
        }
    }
}

impl SiteI18nConfig {
    pub fn validate(&self) -> Result<(), String> {
        if !self.supported_locales.contains(&self.default_locale) {
            return Err(format!(
                "default_locale '{}' is not in supported_locales",
                self.default_locale
            ));
        }
        Ok(())
    }

    pub fn locale_prefix(&self, locale: &LocaleCode) -> String {
        if locale == &self.default_locale {
            String::new()
        } else {
            format!("/{locale}")
        }
    }
}
