//! Parsing a model answer into candidates. The structured output is
//! preferred and the text is parsed when it is absent; anything the kind's
//! schema does not admit (a missing `facts`, an extra key, a non-object
//! fact, a subject that is not a string, a malformed span) rejects the
//! whole answer.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::{Map, Value};
use systemprompt_models::brain::{
    BrainModelResponse, FactCandidate, FactSource, KindDecl, SourceSpan, SubjectKey,
};

use super::ModelOutputError;
use super::schema::{CONFIDENCE_KEY, FACTS_KEY, RESERVED_KEYS, SPAN_KEY, SUBJECT_KEY};

pub fn parse_output(
    kind: &KindDecl,
    response: &BrainModelResponse,
    source: &FactSource,
    extractor: &str,
) -> Result<Vec<FactCandidate>, ModelOutputError> {
    let output = match &response.structured {
        Some(value) => value.clone(),
        None => serde_json::from_str(&response.text).map_err(|_| ModelOutputError::NotJson)?,
    };
    let Value::Object(mut root) = output else {
        return Err(schema("output is not an object"));
    };
    if root.len() != 1 {
        return Err(schema("output must hold exactly `facts`"));
    }
    let Some(Value::Array(facts)) = root.remove(FACTS_KEY) else {
        return Err(schema("`facts` is not an array"));
    };
    facts
        .into_iter()
        .map(|fact| parse_fact(kind, fact, source, extractor))
        .collect()
}

// JSON: one fact object of the model's structured output.
fn parse_fact(
    kind: &KindDecl,
    fact: Value,
    source: &FactSource,
    extractor: &str,
) -> Result<FactCandidate, ModelOutputError> {
    let Value::Object(mut object) = fact else {
        return Err(schema("a fact is not an object"));
    };
    if let Some(key) = object
        .keys()
        .find(|key| !RESERVED_KEYS.contains(&key.as_str()) && !kind.fields.contains_key(*key))
    {
        return Err(ModelOutputError::Schema(format!("unexpected key {key}")));
    }
    let Some(Value::String(subject)) = object.remove(SUBJECT_KEY) else {
        return Err(schema("`subject` is not a string"));
    };
    let confidence = match object.remove(CONFIDENCE_KEY) {
        None | Some(Value::Null) => None,
        Some(Value::Number(n)) => n.as_f64().map(|c| c.clamp(0.0, 1.0) as f32),
        Some(_) => return Err(schema("`confidence` is not a number")),
    };
    let source = with_span(source, object.remove(SPAN_KEY))?;
    let fields: Map<String, Value> = object.into_iter().filter(|(_, v)| !v.is_null()).collect();
    let prefix = format!("{}:", kind.subject);
    let id = subject.strip_prefix(&prefix).unwrap_or(&subject).trim();
    if id.is_empty() {
        return Err(schema("`subject` is empty"));
    }
    let mut builder = FactCandidate::builder(
        kind.kind.clone(),
        SubjectKey::new(kind.subject.clone(), id),
        source,
    )
    .fields(fields)
    .extractor(extractor);
    if let Some(confidence) = confidence {
        builder = builder.confidence(confidence);
    }
    Ok(builder.build())
}

// JSON: the optional span object of one fact of the model's output.
fn with_span(source: &FactSource, span: Option<Value>) -> Result<FactSource, ModelOutputError> {
    let span = match span {
        None | Some(Value::Null) => return Ok(source.clone()),
        Some(value) => serde_json::from_value::<SourceSpan>(value)
            .map_err(|_| schema("`span` is malformed"))?,
    };
    match source {
        FactSource::Document {
            document_id,
            revision_id,
            ..
        } => Ok(FactSource::Document {
            document_id: document_id.clone(),
            revision_id: revision_id.clone(),
            span: Some(span),
        }),
        _ => Ok(source.clone()),
    }
}

fn schema(reason: &str) -> ModelOutputError {
    ModelOutputError::Schema(reason.to_owned())
}
