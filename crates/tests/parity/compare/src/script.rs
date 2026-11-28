//! What the trace builder needs from a scenario's `turns.json`: per turn, the
//! call ids and text it answers with and the status it faults with.

use serde_json::Value;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScriptTurn {
    pub calls: Vec<String>,
    pub text: String,
    pub status: Option<u16>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScenarioScript {
    pub turns: Vec<ScriptTurn>,
}

impl ScenarioScript {
    pub fn parse(turns_json: &str) -> anyhow::Result<Self> {
        let document: Value = serde_json::from_str(turns_json)?;
        let turns = document
            .get("turns")
            .and_then(Value::as_array)
            .map(|turns| turns.iter().map(turn).collect())
            .unwrap_or_default();
        Ok(Self { turns })
    }
}

fn turn(value: &Value) -> ScriptTurn {
    let mut parsed = ScriptTurn::default();
    for part in value
        .get("respond")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if let Some(text) = part.get("text").and_then(Value::as_str) {
            parsed.text.push_str(text);
        }
        if let Some(id) = part.pointer("/tool_call/id").and_then(Value::as_str) {
            parsed.calls.push(id.to_owned());
        }
    }
    parsed.status = value
        .pointer("/fault/status")
        .and_then(Value::as_u64)
        .and_then(|status| u16::try_from(status).ok());
    parsed
}
