//! Device evidence schemas retain exact readback and separate native sessions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.
use super::builder::Document;
use crate::routes::evaluation::consumer;
use systemprompt_marketplace::managed::consumer::{
    ConsumerAttribution, ConsumerInvocationRequest, ConsumerSessionBinding,
};
use systemprompt_models::feedback::receipts::{
    ConsumerInstallationPlan, ConsumerReceiptRequest, ConsumerReceiptResponse,
    SessionBindingRequest,
};
pub(super) fn register(d: &mut Document) {
    d.add::<(), consumer::Enrollment>("/consumer-devices/enrollment", "post", 200, true);
    d.add::<(), consumer::admin::CredentialIssueResponse>(
        "/consumer-devices/{id}/credential",
        "post",
        200,
        false,
    );
    d.idempotent("/consumer-devices/{id}/credential");
    d.add::<(), ()>("/consumer-devices/{id}/revocation", "post", 204, false);
    d.add::<consumer::admin::Grant, ()>("/resources/{id}/consumer-grants", "post", 204, false);
    d.add::<(), ConsumerInstallationPlan>(
        "/consumer/resources/{resource}/publications/{publication}/bundle",
        "get",
        200,
        true,
    );
    d.query::<consumer::BundleQuery>(
        "/consumer/resources/{resource}/publications/{publication}/bundle",
        "get",
    );
    d.add::<ConsumerReceiptRequest, ConsumerReceiptResponse>(
        "/consumer/receipts",
        "post",
        200,
        true,
    );
    d.add::<(), ConsumerReceiptResponse>("/consumer/receipts/{id}", "get", 200, true);
    d.add::<SessionBindingRequest, ConsumerSessionBinding>(
        "/consumer/session-bindings",
        "post",
        200,
        true,
    );
    d.add::<(), ConsumerSessionBinding>("/consumer/session-bindings/{id}", "get", 200, true);
    d.add::<ConsumerInvocationRequest, ConsumerAttribution>(
        "/consumer/invocations",
        "post",
        200,
        true,
    );
    d.add::<(), ConsumerAttribution>("/consumer/invocations/{id}", "get", 200, true);
    d.query::<consumer::BundleQuery>("/consumer/invocations/{id}", "get");
}
