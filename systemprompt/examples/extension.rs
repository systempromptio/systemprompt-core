//! Minimal `Extension` implementation showing the compile-time plugin model.
//!
//! Run with: `cargo run -p systemprompt --example extension --features core`

use systemprompt::extension::prelude::*;

#[derive(Default)]
struct DemoExtension;

impl Extension for DemoExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "demo-extension",
            name: "Demo Extension",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![SchemaDefinition::new(
            "demo_extension_state",
            "CREATE TABLE IF NOT EXISTS demo_extension_state (id TEXT PRIMARY KEY)",
        )]
    }

    fn router(&self, _ctx: &dyn ExtensionContext) -> Option<ExtensionRouter> {
        None
    }
}

register_extension!(DemoExtension);

fn main() {
    tracing_subscriber::fmt::init();
    let ext = DemoExtension;
    let meta = ext.metadata();
    tracing::info!(
        id = meta.id,
        name = meta.name,
        version = meta.version,
        schemas = ext.schemas().len(),
        "registered demo extension"
    );
}
