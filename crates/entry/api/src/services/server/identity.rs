//! The fields every probe body carries to name the node that answered: its
//! instance, core's version and the build the embedding binary named.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::json;
use systemprompt_runtime::AppContext;

// JSON: the probe bodies are open objects each handler extends with its own
// fields.
pub(super) fn node_identity(ctx: &AppContext) -> serde_json::Map<String, serde_json::Value> {
    let mut fields = serde_json::Map::new();
    fields.insert("instance".to_owned(), json!(ctx.config().instance_id));
    fields.insert("version".to_owned(), json!(env!("CARGO_PKG_VERSION")));
    fields.insert("build".to_owned(), json!(ctx.build_identity()));
    fields
}
