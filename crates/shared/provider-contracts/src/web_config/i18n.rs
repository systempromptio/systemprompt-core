//! `SiteI18nConfig` for a site's default and supported locales.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::LocaleCode;

use super::WebConfigError;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SiteI18nConfig {
    pub default_locale: LocaleCode,
    pub supported_locales: Vec<LocaleCode>,
}

impl Default for SiteI18nConfig {
    fn default() -> Self {
        let default_locale = LocaleCode::english();
        Self {
            supported_locales: vec![default_locale.clone()],
            default_locale,
        }
    }
}

impl SiteI18nConfig {
    pub fn validate(&self) -> Result<(), WebConfigError> {
        if !self.supported_locales.contains(&self.default_locale) {
            return Err(WebConfigError::UnsupportedDefaultLocale {
                default_locale: self.default_locale.clone(),
            });
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
