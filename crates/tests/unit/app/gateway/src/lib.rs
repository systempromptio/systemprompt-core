//! Unit tests for the systemprompt-gateway crate: protocol adapters, outbound
//! dispatch and retry, failover, pricing, safety, stream tap, audit journal,
//! and the gateway-policy spec, loader, ingestion and extension contracts.

#[cfg(test)]
mod abandon_guard;
#[cfg(test)]
mod audit_payload;
#[cfg(test)]
mod canonical_request;
#[cfg(test)]
mod canonical_response;
#[cfg(test)]
mod failover;
#[cfg(test)]
mod google_credentials;
#[cfg(test)]
mod image_fetch;
#[cfg(test)]
mod inbound_anthropic;
#[cfg(test)]
mod inbound_anthropic_deep;
#[cfg(test)]
mod inbound_anthropic_render;
#[cfg(test)]
mod inbound_openai;
#[cfg(test)]
mod inbound_openai_chat;
#[cfg(test)]
mod inbound_openai_deep;
#[cfg(test)]
mod inbound_openai_render;
#[cfg(test)]
mod inbound_stream_abort;
#[cfg(test)]
mod inbound_stream_usage;
#[cfg(test)]
mod inbound_tool_choice;
#[cfg(test)]
mod inbound_trait_defaults;
#[cfg(test)]
mod inspect_equals_send;
#[cfg(test)]
mod journal_open;
#[cfg(test)]
mod openai_passthrough;
#[cfg(test)]
mod outbound;
#[cfg(test)]
mod outbound_deep;
#[cfg(test)]
mod outbound_passthrough;
#[cfg(test)]
mod outbound_passthrough_terminal;
#[cfg(test)]
mod outbound_refused_betas;
#[cfg(test)]
mod outbound_refused_fields;
#[cfg(test)]
mod outbound_retry;
#[cfg(test)]
mod outbound_vertex;
#[cfg(test)]
mod parse;
#[cfg(test)]
mod policies;
#[cfg(test)]
mod pricing;
#[cfg(test)]
mod pricing_cache_table;
#[cfg(test)]
mod prompt_cache_control;
#[cfg(test)]
mod prompt_recovery;
#[cfg(test)]
mod prompt_recovery_transport;
#[cfg(test)]
mod registry;
#[cfg(test)]
mod route_match_descriptor;
#[cfg(test)]
mod route_requirements;
#[cfg(test)]
mod safety;
#[cfg(test)]
mod signature_cache;
#[cfg(test)]
mod stream_tap;
#[cfg(test)]
mod stream_tap_accumulator;
#[cfg(test)]
mod support;
#[cfg(test)]
mod upstream_error;
