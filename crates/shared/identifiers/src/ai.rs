//! AI subsystem identifiers (requests, messages, configs, safety findings,
//! quota buckets, gateway policies).

crate::define_id!(AiRequestId, generate, schema);
crate::define_id!(MessageId, generate, schema);
crate::define_id!(ConfigId, generate, schema);
crate::define_id!(AiSafetyFindingId, generate, schema);
crate::define_id!(AiQuotaBucketId, generate, schema);
crate::define_id!(AiGatewayPolicyId, generate, schema);
