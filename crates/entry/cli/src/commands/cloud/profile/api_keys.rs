use anyhow::{Result, bail};
use systemprompt_logging::CliService;

use crate::interactive::Prompter;

#[derive(Debug)]
pub struct ApiKeys {
    pub gemini: Option<String>,
    pub anthropic: Option<String>,
    pub openai: Option<String>,
}

impl ApiKeys {
    pub fn from_options(
        gemini: Option<String>,
        anthropic: Option<String>,
        openai: Option<String>,
    ) -> Result<Self> {
        if gemini.is_none() && anthropic.is_none() && openai.is_none() {
            bail!(
                "At least one AI provider API key is required.\nProvide: --anthropic-key, \
                 --openai-key, or --gemini-key"
            );
        }
        Ok(Self {
            gemini,
            anthropic,
            openai,
        })
    }

    pub const fn selected_provider(&self) -> &'static str {
        if self.anthropic.is_some() {
            "anthropic"
        } else if self.openai.is_some() {
            "openai"
        } else if self.gemini.is_some() {
            "gemini"
        } else {
            "anthropic"
        }
    }
}

pub fn collect_api_keys(prompter: &dyn Prompter) -> Result<ApiKeys> {
    CliService::info("At least one AI provider API key is required.");

    let providers = vec![
        "Google AI (Gemini) - https://aistudio.google.com/app/apikey".to_owned(),
        "Anthropic (Claude) - https://console.anthropic.com/api-keys".to_owned(),
        "OpenAI (GPT) - https://platform.openai.com/api-keys".to_owned(),
    ];

    let selection = prompter.select("Select your AI provider", &providers)?;

    match selection {
        0 => {
            let key = prompter.password("Gemini API Key")?;
            if key.is_empty() {
                bail!("API key is required");
            }
            Ok(ApiKeys {
                gemini: Some(key),
                anthropic: None,
                openai: None,
            })
        },
        1 => {
            let key = prompter.password("Anthropic API Key")?;
            if key.is_empty() {
                bail!("API key is required");
            }
            Ok(ApiKeys {
                gemini: None,
                anthropic: Some(key),
                openai: None,
            })
        },
        2 => {
            let key = prompter.password("OpenAI API Key")?;
            if key.is_empty() {
                bail!("API key is required");
            }
            Ok(ApiKeys {
                gemini: None,
                anthropic: None,
                openai: Some(key),
            })
        },
        _ => bail!("Invalid selection"),
    }
}
