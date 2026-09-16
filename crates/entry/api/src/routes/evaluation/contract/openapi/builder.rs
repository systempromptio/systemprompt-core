//! Generate schemas, cursor parameters and uniform problem responses together.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.
use schemars::generate::SchemaSettings;
use schemars::{JsonSchema, SchemaGenerator};
// JSON: OpenAPI itself is a standards-defined dynamic document. Endpoint bodies
// and responses below are generated from concrete Rust JsonSchema
// implementations.
use serde_json::{Map, Value, json};
pub(super) struct Document {
    generator: SchemaGenerator,
    paths: Map<String, Value>,
}
impl Document {
    pub(super) fn new() -> Self {
        let mut settings = SchemaSettings::draft2020_12();
        settings.definitions_path = "/components/schemas".into();
        Self {
            generator: settings.into_generator(),
            paths: Map::new(),
        }
    }
    pub(super) fn add<I: JsonSchema, O: JsonSchema>(
        &mut self,
        path: &str,
        method: &str,
        status: u16,
        consumer: bool,
    ) {
        let output = self.generator.subschema_for::<O>().to_value();
        let problem = self
            .generator
            .subschema_for::<super::super::Problem>()
            .to_value();
        let mut responses = Map::new();
        responses.insert(status.to_string(),if status==204 {json!({"description":"Mutation completed; no response body"})}else{json!({"description":"Retained result or current operation status","content":{"application/json":{"schema":output}}})});
        for code in [400, 401, 403, 404, 409, 413, 415, 422, 429, 500] {
            responses.insert(code.to_string(),json!({"description":"Problem details","content":{"application/problem+json":{"schema":problem}},"headers":{"Retry-After":{"description":"Retry delay when rate limited","schema":{"type":"string"}}}}));
        }
        let mut parameters = Vec::new();
        for part in path.split('/') {
            if part.starts_with('{') && part.ends_with('}') {
                parameters.push(json!({"name":&part[1..part.len()-1],"in":"path","required":true,"schema":{"type":"string","minLength":1,"maxLength":512}}));
            }
        }
        let security = if consumer {
            json!([{"deviceCredential":[]}])
        } else {
            json!([{"adminBearer":[]},{"adminCookie":[]}])
        };
        let mut operation = json!({"operationId":format!("{}_{}",method,path.trim_start_matches('/').replace(['/', '{','}','-'],"_")),"tags":[if consumer{"Consumer evidence"}else{"Optimization administration"}],"security":security,"parameters":parameters,"responses":responses});
        if method != "get" {
            parameters.push(json!({"name":"Origin","in":"header","required":false,"schema":{"type":"string","format":"uri"},"description":"Required and exact-match validated whenever a Cookie header is present."}));
            operation["parameters"] = json!(parameters);
            operation["description"] = json!(
                "Administrative cookie mutations require an exact Origin match. Device evidence derives consumer identity from the credential; organizational ownership is retained server-side. Human publication approval remains separate from evaluation."
            );
        }
        if std::any::type_name::<I>() != "()" {
            let input = self.generator.subschema_for::<I>().to_value();
            operation["requestBody"] =
                json!({"required":true,"content":{"application/json":{"schema":input}}});
        }
        self.paths
            .entry(path.to_owned())
            .or_insert_with(|| json!({}))[method] = operation;
    }
    pub(super) fn query<Q: JsonSchema>(&mut self, path: &str, method: &str) {
        let schema = Q::json_schema(&mut self.generator).to_value();
        let required = schema
            .get("required")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
            for (name, schema) in properties {
                self.parameter(path,method,json!({"name":name,"in":"query","required":required.iter().any(|field|field.as_str()==Some(name)),"schema":schema}));
            }
        }
    }
    pub(super) fn parameter(&mut self, path: &str, method: &str, parameter: Value) {
        if let Some(parameters) = self
            .paths
            .get_mut(path)
            .and_then(|path| path.get_mut(method))
            .and_then(|operation| operation.get_mut("parameters"))
            .and_then(Value::as_array_mut)
        {
            parameters.push(parameter);
        }
    }
    pub(super) fn idempotent(&mut self, path: &str) {
        self.parameter(path,"post",json!({"name":"Idempotency-Key","in":"header","required":true,"schema":{"type":"string","minLength":1,"maxLength":200},"description":"Owner-scoped operation identity. Identical retries recover retained inputs/results; conflicting reuse is HTTP 409. Status is available at /operations/{id}. Credential tokens are delivered once; lost delivery requires deliberate rotation with a new key."}));
        self.operation_headers(path);
    }
    pub(super) fn operation_headers(&mut self, path: &str) {
        let responses = &mut self.paths[path]["post"]["responses"];
        let mut pending = responses["200"].clone();
        pending["description"] = json!(
            "Operation is already leased; poll its Location instead of replaying the mutation."
        );
        responses["202"] = pending;
        for status in ["200", "202"] {
            responses[status]["headers"] = json!({"Location":{"description":"Owner-scoped durable operation status","schema":{"type":"string","format":"uri-reference"}}});
        }
    }
    pub(super) fn stream<T: JsonSchema>(&mut self, path: &str) {
        self.add::<(), T>(path, "get", 200, false);
        let schema = self.generator.subschema_for::<T>().to_value();
        self.paths[path]["get"]["responses"]["200"] = json!({"description":"Server-sent snapshot and resync events. Reconnect uses durable composite generations; notifications are wake-up hints. Closing the connection releases its core connection guard.","content":{"text/event-stream":{"schema":{"type":"string"},"x-event-data-schema":schema}}});
        self.parameter(path,"get",json!({"name":"Last-Event-ID","in":"header","required":false,"schema":{"type":"string","maxLength":96,"pattern":"^[0-9]+[.][0-9]+[.][0-9]+$"}}));
    }
    pub(super) fn finish(self) -> Value {
        json!({"openapi":"3.1.0","jsonSchemaDialect":"https://json-schema.org/draft/2020-12/schema","info":{"title":"Skill optimization feedback API","version":"1.0.0","description":"Organizational inventory, device-authenticated evidence, verified evaluation and snapshot analytics. Native targets remain disabled without verified isolation and metering. All errors use problem details; cookie writes require matching Origin. No automatic publication."},"servers":[{"url":"/api/v1"}],"paths":self.paths,"components":{"schemas":self.generator.definitions(),"securitySchemes":{"adminBearer":{"type":"http","scheme":"bearer","description":"Authenticated administrator credential; consumer grants do not confer administration."},"adminCookie":{"type":"apiKey","in":"cookie","name":"access_token","description":"Existing authenticated admin session; mutations require exact Origin."},"deviceCredential":{"type":"http","scheme":"bearer","bearerFormat":"sp_device_…","description":"Credential bound to an enrolled, non-revoked device. A user bridge secret or submitted device ID is insufficient."}}}})
    }
}
