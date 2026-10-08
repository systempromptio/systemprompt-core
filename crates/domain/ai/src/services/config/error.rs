//! Typed failures of AI-config validation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use thiserror::Error;

/// A rejected AI policy: which rule failed and the values that broke it.
#[derive(Debug, Error)]
pub enum AiConfigError {
    #[error("{}", no_providers_message(.policy_providers, .unresolved_secrets))]
    NoProvidersEnabled {
        policy_providers: Vec<String>,
        unresolved_secrets: Vec<String>,
    },

    #[error(
        "Default provider '{provider}' must be an enabled entry under ai.providers.\nEnabled \
         policy providers: {enabled:?}\nFix: enable '{provider}' or change 'default_provider'"
    )]
    DefaultProviderNotEnabled {
        provider: String,
        enabled: Vec<String>,
    },

    #[error(
        "Default provider '{provider}' has no connectivity in the profile registry.\nProviders \
         with connectivity: {connected:?}\nFix: add a `providers` registry entry named \
         '{provider}' and its api_key secret{}",
        withheld_suffix(.withheld.as_deref())
    )]
    DefaultProviderNoConnectivity {
        provider: String,
        connected: Vec<String>,
        withheld: Option<String>,
    },

    #[error("MCP connect timeout must be greater than 0")]
    ZeroMcpConnectTimeout,

    #[error("MCP execution timeout must be greater than 0")]
    ZeroMcpRequestTimeout,
}

fn withheld_suffix(withheld: Option<&str>) -> String {
    withheld.map_or_else(String::new, |message| format!("\nWithheld: {message}"))
}

fn no_providers_message(policy_providers: &[String], unresolved_secrets: &[String]) -> String {
    let mut message = String::from("No AI providers are enabled.\n\n");

    if unresolved_secrets.is_empty() {
        message.push_str(
            "To fix, enable a provider in your AI policy and declare its connectivity in the \
             profile `providers` registry:\n\n  ai:\n    default_provider: gemini\n    \
             providers:\n      gemini:\n        enabled: true\n\nAnd add the matching \
             credential to your secrets.json.\n",
        );
    } else {
        message.push_str("Providers with unresolved secrets:\n");
        for unresolved in unresolved_secrets {
            message.push_str(&format!("  - {unresolved}\n"));
        }
        message.push_str("\nTo fix: add the required API keys to your secrets.json file\n");
    }

    message.push_str(&format!(
        "\nProviders defined in AI policy: {policy_providers:?}"
    ));
    message
}
