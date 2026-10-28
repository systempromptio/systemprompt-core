//! The `tools/list` shape of the document tools.
//!
//! Descriptions are one line and the working detail lives in
//! `documents_guide`, so the list stays small. Edit ops are listed by name
//! with their fields left open; the `editing` topic carries their shapes.
//! A tool advertises no output schema and no artifact-viewer `_meta`, since
//! results arrive as summary text and untagged structured content and this
//! server serves only the document-preview resource, which `documents_export`
//! names.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;
use std::sync::Arc;

use rmcp::model::Tool;
use serde_json::{Map, Value as JsonValue, json};
use systemprompt_mcp::McpToolHandler;

use super::{
    AccessHandler, CommentHandler, CopyHandler, CreateHandler, DocumentsMcpTools, EditHandler,
    ExportHandler, GuideHandler, HistoryHandler, ListHandler, ProposalsHandler, ReadHandler,
    SearchHandler, ShareHandler, TrashHandler, UntrashHandler, UpdateHandler, UploadHandler,
    preview_tool_meta,
};

const OP_NAMES: [&str; 8] = [
    "replace_text",
    "insert_after",
    "delete",
    "str_replace",
    "set_cells",
    "insert_slide",
    "set_text",
    "set_html",
];

// JSON: MCP tool `inputSchema` — the ops array is described in the guide.
fn ops_schema() -> JsonValue {
    json!({
        "type": "array",
        "items": {
            "type": "object",
            "properties": { "op": { "enum": OP_NAMES } },
            "required": ["op"]
        }
    })
}

// JSON: walks an arbitrary JSON Schema document for $ref strings
fn refs(value: &JsonValue, out: &mut BTreeSet<String>) {
    match value {
        JsonValue::Object(map) => {
            for (key, inner) in map {
                if key == "$ref"
                    && let Some(target) = inner.as_str().and_then(|r| r.rsplit('/').next())
                {
                    out.insert(target.to_owned());
                }
                refs(inner, out);
            }
        },
        JsonValue::Array(items) => items.iter().for_each(|item| refs(item, out)),
        _ => {},
    }
}

// JSON: edits the schemars output in place; the schema has no typed shape
fn prune_definitions(schema: &mut Map<String, JsonValue>) {
    for key in ["$defs", "definitions"] {
        let Some(JsonValue::Object(mut defs)) = schema.remove(key) else {
            continue;
        };
        loop {
            let mut used = BTreeSet::new();
            refs(&JsonValue::Object(schema.clone()), &mut used);
            refs(&JsonValue::Object(defs.clone()), &mut used);
            let before = defs.len();
            defs.retain(|name, _| used.contains(name));
            if defs.len() == before {
                break;
            }
        }
        if !defs.is_empty() {
            schema.insert(key.to_owned(), JsonValue::Object(defs));
        }
    }
}

// JSON: the enum values of a `$ref` target, when that definition is an enum
fn ref_enum(property: &Map<String, JsonValue>, defs: &Map<String, JsonValue>) -> Option<JsonValue> {
    let name = property.get("$ref")?.as_str()?.rsplit('/').next()?;
    defs.get(name)?.get("enum").cloned()
}

// JSON: narrows one schemars property schema to what a caller needs
fn slim(property: &mut JsonValue, defs: &Map<String, JsonValue>) {
    let JsonValue::Object(map) = property else {
        return;
    };
    map.remove("default");
    map.remove("format");
    if let Some(JsonValue::Array(options)) = map.get("anyOf") {
        let concrete: Vec<JsonValue> = options
            .iter()
            .filter(|option| option.get("type").and_then(JsonValue::as_str) != Some("null"))
            .cloned()
            .collect();
        if let [single] = concrete.as_slice()
            && let JsonValue::Object(inner) = single.clone()
        {
            map.remove("anyOf");
            map.extend(inner);
        }
    }
    if let Some(JsonValue::Array(types)) =
        map.get("type").filter(|_| !map.contains_key("description"))
    {
        let concrete: Vec<JsonValue> = types.iter().filter(|t| *t != "null").cloned().collect();
        if let [single] = concrete.as_slice() {
            map.insert("type".to_owned(), single.clone());
        }
    }
    if let Some(values) = ref_enum(map, defs) {
        map.remove("$ref");
        map.insert("enum".to_owned(), values);
    }
    if let Some(JsonValue::Object(properties)) = map.get_mut("properties") {
        properties.values_mut().for_each(|inner| slim(inner, defs));
    }
}

// JSON: schemars output in, compacted schemars output out
fn compact_input(schema: &Map<String, JsonValue>) -> Map<String, JsonValue> {
    let mut schema = schema.clone();
    schema.remove("$schema");
    schema.remove("title");
    let defs = match schema.get_mut("$defs") {
        Some(JsonValue::Object(defs)) => {
            let snapshot = defs.clone();
            defs.values_mut().for_each(|def| slim(def, &snapshot));
            defs.clone()
        },
        _ => Map::new(),
    };
    if let Some(JsonValue::Object(properties)) = schema.get_mut("properties") {
        properties
            .values_mut()
            .for_each(|property| slim(property, &defs));
        if properties.contains_key("ops") {
            properties.insert("ops".to_owned(), ops_schema());
        }
    }
    prune_definitions(&mut schema);
    schema
}

fn compact(mut tool: Tool, keep_meta: bool) -> Tool {
    tool.input_schema = Arc::new(compact_input(&tool.input_schema));
    tool.output_schema = None;
    if !keep_meta {
        tool.meta = None;
    }
    tool
}

pub(super) fn tool_list(tools: &DocumentsMcpTools) -> Vec<Tool> {
    let server = tools.server_name();
    let mut export = ExportHandler { tools }.tool_definition(server);
    export.meta = Some(preview_tool_meta(server));
    vec![
        compact(ListHandler { tools }.tool_definition(server), false),
        compact(SearchHandler { tools }.tool_definition(server), false),
        compact(CreateHandler { tools }.tool_definition(server), false),
        compact(ReadHandler { tools }.tool_definition(server), false),
        compact(EditHandler { tools }.tool_definition(server), false),
        compact(CommentHandler { tools }.tool_definition(server), false),
        compact(HistoryHandler { tools }.tool_definition(server), false),
        compact(ProposalsHandler { tools }.tool_definition(server), false),
        compact(ShareHandler { tools }.tool_definition(server), false),
        compact(AccessHandler { tools }.tool_definition(server), false),
        compact(CopyHandler { tools }.tool_definition(server), false),
        compact(UpdateHandler { tools }.tool_definition(server), false),
        compact(TrashHandler { tools }.tool_definition(server), false),
        compact(UntrashHandler { tools }.tool_definition(server), false),
        compact(export, true),
        compact(UploadHandler { tools }.tool_definition(server), false),
        compact(GuideHandler { tools }.tool_definition(server), false),
    ]
}
