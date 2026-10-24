//! Typed failures of AI-config validation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use thiserror::Error;

/// A rejected AI configuration: which rule failed and the values that broke it.
#[derive(Debug, Error)]
pub enum AiConfigError {
    #[error("{}", no_providers_message(.enabled_providers, .withheld))]
    NoProvidersEnabled {
        enabled_providers: Vec<String>,
        withheld: Vec<String>,
    },

    #[error(
        "No default model: set `gateway.default_model` in services/ai/gateway.yaml to a model \
         in services/ai/models.yaml"
    )]
    NoDefaultModel,

    #[error(
        "Default model '{model}' has no deployment the in-process service can reach.\nIts \
         deployments: {deployments:?}\nEndpoints with connectivity: {connected:?}{}\nFix: \
         deploy '{model}' on an enabled provider with a platform credential whose secret is \
         set, or change `gateway.default_model`",
        withheld_suffix(.withheld)
    )]
    DefaultModelNoConnectivity {
        model: String,
        deployments: Vec<String>,
        connected: Vec<String>,
        withheld: Vec<String>,
    },

    #[error("MCP connect timeout must be greater than 0")]
    ZeroMcpConnectTimeout,

    #[error("MCP execution timeout must be greater than 0")]
    ZeroMcpRequestTimeout,
}

fn withheld_suffix(withheld: &[String]) -> String {
    if withheld.is_empty() {
        String::new()
    } else {
        format!("\nWithheld: {}", withheld.join("; "))
    }
}

fn no_providers_message(enabled_providers: &[String], withheld: &[String]) -> String {
    let mut message = String::from("No AI provider endpoint is usable in-process.\n\n");
    if withheld.is_empty() {
        message.push_str(
            "To fix, declare a platform credential in services/ai/credentials.yaml, a provider \
             using it in services/ai/providers.yaml, and deploy a model on one of its \
             endpoints in services/ai/deployments.yaml; then add the credential's secret to \
             your secrets.json.\n",
        );
    } else {
        message.push_str("Endpoints withheld:\n");
        for reason in withheld {
            message.push_str(&format!("  - {reason}\n"));
        }
        message.push_str("\nTo fix: add the required secrets to your secrets.json file\n");
    }
    message.push_str(&format!("\nEnabled providers: {enabled_providers:?}"));
    message
}
