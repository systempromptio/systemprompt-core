//! Unit tests for systemprompt-cloud crate

#[cfg(test)]
mod api_client_tests;
#[cfg(test)]
mod checkout_flow;
#[cfg(test)]
mod cli_session;
#[cfg(test)]
mod constants;
#[cfg(test)]
mod constants_extra;
#[cfg(test)]
#[cfg(test)]
mod credentials;
#[cfg(test)]
mod credentials_bootstrap_error;
#[cfg(test)]
mod credentials_bootstrap_flow;
#[cfg(test)]
mod credentials_bootstrap_paths;
#[cfg(test)]
mod deploy_dockerfile;
#[cfg(test)]
mod deploy_dockerfile_extensions;
#[cfg(test)]
mod deploy_validation;
#[cfg(test)]
mod discovered_project;
#[cfg(test)]
mod docker_cli;
#[cfg(test)]
mod environment;
#[cfg(test)]
mod error;
#[cfg(test)]
mod error_extra;
#[cfg(test)]
mod jwt;
#[cfg(test)]
mod oauth_flow;
#[cfg(test)]
mod paths;
#[cfg(test)]
mod profile_authoring;
#[cfg(test)]
mod project_paths;
#[cfg(test)]
mod provisioning_wait;
#[cfg(test)]
mod secrets_env;
#[cfg(test)]
mod session_key;
#[cfg(test)]
mod session_store;
#[cfg(test)]
mod stored_tenant_extra;
#[cfg(test)]
mod streams_sse;
#[cfg(test)]
mod tenant_api_tests;
#[cfg(test)]
mod tenant_provisioning;
#[cfg(test)]
mod tenant_token_retry;
#[cfg(test)]
mod tenants;

mod trusted_proxies;
#[cfg(test)]
mod wire_format;
