use serde_json::{Value, json};
use uuid::Uuid;

use super::models::{
    AnthropicGatewayRequest, AnthropicGatewayResponse, AnthropicGatewayUsage, OpenAiGatewayMessage,
    OpenAiGatewayRequest, OpenAiGatewayResponse,
};

pub fn to_openai_request(
    req: &AnthropicGatewayRequest,
    upstream_model: &str,
) -> OpenAiGatewayRequest {
    let mut messages: Vec<OpenAiGatewayMessage> = Vec::new();

    if let Some(system) = &req.system {
        let content = match system {
            Value::String(s) => Value::String(s.clone()),
            other => other.clone(),
        };
        messages.push(OpenAiGatewayMessage {
            role: "system".to_string(),
            content,
        });
    }

    for msg in &req.messages {
        messages.push(OpenAiGatewayMessage {
            role: msg.role.clone(),
            content: msg.content.clone(),
        });
    }

    OpenAiGatewayRequest {
        model: upstream_model.to_string(),
        messages,
        max_tokens: req.max_tokens,
        temperature: req.temperature,
        top_p: req.top_p,
        stop: req.stop_sequences.clone(),
        stream: req.stream,
        tools: req.tools.clone(),
        tool_choice: req.tool_choice.clone(),
    }
}

pub fn from_openai_response(
    openai: OpenAiGatewayResponse,
    model: &str,
) -> AnthropicGatewayResponse {
    let choice = openai.choices.into_iter().next();

    let mut content: Vec<Value> = Vec::new();
    let mut stop_reason = None;

    if let Some(ch) = choice {
        stop_reason = ch.finish_reason.map(|r| {
            if r == "stop" {
                "end_turn".to_string()
            } else {
                r
            }
        });

        if let Some(text) = ch.message.content.filter(|s| !s.is_empty()) {
            content.push(json!({ "type": "text", "text": text }));
        }

        for tool_call in ch.message.tool_calls {
            let input: Value = serde_json::from_str(&tool_call.function.arguments)
                .unwrap_or(Value::Object(serde_json::Map::new()));
            content.push(json!({
                "type": "tool_use",
                "id": tool_call.id,
                "name": tool_call.function.name,
                "input": input,
            }));
        }
    }

    let usage = openai.usage.map_or(
        AnthropicGatewayUsage {
            input_tokens: 0,
            output_tokens: 0,
        },
        |u| AnthropicGatewayUsage {
            input_tokens: u.prompt_tokens,
            output_tokens: u.completion_tokens,
        },
    );

    AnthropicGatewayResponse {
        id: format!("msg_{}", Uuid::new_v4().simple()),
        r#type: "message".to_string(),
        role: "assistant".to_string(),
        content,
        model: model.to_string(),
        stop_reason,
        stop_sequence: None,
        usage,
    }
}
