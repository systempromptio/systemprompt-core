//! The built-in `traffic` subject dimension.
//!
//! Every caller holds `production`, so a request that names no traffic class
//! is attributed to it. `evaluation` is held by no user: it is a key-bound
//! value, reachable only through an API key an issuer bound to it, and a
//! header naming it without that binding is refused like any value the caller
//! does not hold. Production usage therefore cannot be relabelled as
//! evaluation by the caller.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_identifiers::UserId;
use systemprompt_models::attribution::TrafficClass;

use super::AuthzError;
use super::subject::{SubjectAttributeProvider, SubjectDimension};
use super::types::RuleType;

pub const TRAFFIC_PRECEDENCE: u16 = 400;

const KEY_BOUND: &[&str] = &[TrafficClass::Evaluation.as_str()];

/// Core's provider for the `traffic` dimension.
#[derive(Debug, Clone, Copy, Default)]
pub struct TrafficProvider;

#[async_trait]
impl SubjectAttributeProvider for TrafficProvider {
    fn dimension(&self) -> SubjectDimension {
        SubjectDimension {
            rule_type: RuleType::extension_static(TrafficClass::DIMENSION),
            label: "Traffic",
            precedence: TRAFFIC_PRECEDENCE,
        }
    }

    async fn values_for(&self, _user_id: &UserId) -> Result<Vec<String>, AuthzError> {
        Ok(vec![TrafficClass::Production.as_str().to_owned()])
    }

    fn key_bound_values(&self) -> &'static [&'static str] {
        KEY_BOUND
    }
}

crate::register_subject_attribute_provider!(|_ctx| Arc::new(TrafficProvider));
