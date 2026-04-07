use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use systemprompt_identifiers::{ArtifactId, ContextId, MessageId, TaskId};
use systemprompt_models::a2a::*;

fn minimal_task() -> Task {
    Task {
        id: TaskId::generate(),
        context_id: ContextId::generate(),
        status: TaskStatus {
            state: TaskState::Working,
            message: None,
            timestamp: None,
        },
        history: None,
        artifacts: None,
        metadata: None,
        created_at: None,
        last_modified: None,
    }
}

fn full_task() -> Task {
    let message = Message {
        role: MessageRole::User,
        parts: vec![Part::Text(TextPart {
            text: "Hello, this is a test message with some content".to_string(),
        })],
        message_id: MessageId::generate(),
        task_id: Some(TaskId::generate()),
        context_id: ContextId::generate(),
        metadata: None,
        extensions: None,
        reference_task_ids: None,
    };

    let artifact = Artifact {
        id: ArtifactId::generate(),
        title: Some("Test Artifact".to_string()),
        description: Some("A test artifact description".to_string()),
        parts: vec![Part::Text(TextPart {
            text: "Artifact content".to_string(),
        })],
        extensions: vec![],
        metadata: ArtifactMetadata {
            artifact_type: "text".to_string(),
            context_id: ContextId::generate(),
            created_at: "2026-04-02".to_string(),
            task_id: TaskId::generate(),
            rendering_hints: None,
            source: None,
            mcp_execution_id: None,
            mcp_schema: None,
            is_internal: None,
            fingerprint: None,
            tool_name: None,
            execution_index: None,
            skill_id: None,
            skill_name: None,
        },
    };

    Task {
        id: TaskId::generate(),
        context_id: ContextId::generate(),
        status: TaskStatus {
            state: TaskState::Completed,
            message: Some(message.clone()),
            timestamp: Some(chrono::Utc::now()),
        },
        history: Some(vec![message]),
        artifacts: Some(vec![artifact]),
        metadata: None,
        created_at: Some(chrono::Utc::now()),
        last_modified: Some(chrono::Utc::now()),
    }
}

fn agent_card() -> AgentCard {
    AgentCard {
        name: "benchmark-agent".to_string(),
        description: "An agent for benchmarking".to_string(),
        supported_interfaces: vec![AgentInterface {
            url: "https://example.com/a2a".to_string(),
            protocol_binding: ProtocolBinding::JsonRpc,
            protocol_version: "1.0.0".to_string(),
        }],
        version: "1.0.0".to_string(),
        capabilities: AgentCapabilities::default(),
        skills: vec![
            AgentSkill {
                id: "search".to_string(),
                name: "Web Search".to_string(),
                description: "Search the web".to_string(),
                tags: vec!["search".to_string(), "web".to_string()],
                examples: Some(vec!["search for rust".to_string()]),
                input_modes: None,
                output_modes: None,
                security: None,
            },
            AgentSkill {
                id: "code".to_string(),
                name: "Code Generation".to_string(),
                description: "Generate code".to_string(),
                tags: vec!["code".to_string()],
                examples: None,
                input_modes: None,
                output_modes: None,
                security: None,
            },
        ],
        default_input_modes: vec!["text".to_string()],
        default_output_modes: vec!["text".to_string()],
        ..Default::default()
    }
}

fn bench_serialize(c: &mut Criterion) {
    let mut group = c.benchmark_group("serialize");

    let minimal = minimal_task();
    let json = serde_json::to_string(&minimal).unwrap();
    group.throughput(Throughput::Bytes(json.len() as u64));
    group.bench_function("task_minimal", |b| {
        b.iter(|| serde_json::to_string(&minimal).unwrap())
    });

    let full = full_task();
    let json = serde_json::to_string(&full).unwrap();
    group.throughput(Throughput::Bytes(json.len() as u64));
    group.bench_function("task_full", |b| {
        b.iter(|| serde_json::to_string(&full).unwrap())
    });

    let card = agent_card();
    let json = serde_json::to_string(&card).unwrap();
    group.throughput(Throughput::Bytes(json.len() as u64));
    group.bench_function("agent_card", |b| {
        b.iter(|| serde_json::to_string(&card).unwrap())
    });

    group.finish();
}

fn bench_deserialize(c: &mut Criterion) {
    let mut group = c.benchmark_group("deserialize");

    let minimal_json = serde_json::to_string(&minimal_task()).unwrap();
    group.throughput(Throughput::Bytes(minimal_json.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("task_minimal", minimal_json.len()),
        &minimal_json,
        |b, json| b.iter(|| serde_json::from_str::<Task>(json).unwrap()),
    );

    let full_json = serde_json::to_string(&full_task()).unwrap();
    group.throughput(Throughput::Bytes(full_json.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("task_full", full_json.len()),
        &full_json,
        |b, json| b.iter(|| serde_json::from_str::<Task>(json).unwrap()),
    );

    let card_json = serde_json::to_string(&agent_card()).unwrap();
    group.throughput(Throughput::Bytes(card_json.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("agent_card", card_json.len()),
        &card_json,
        |b, json| b.iter(|| serde_json::from_str::<AgentCard>(json).unwrap()),
    );

    group.finish();
}

criterion_group!(benches, bench_serialize, bench_deserialize);
criterion_main!(benches);
