use proptest::prelude::*;
use systemprompt_identifiers::SkillId;
use systemprompt_models::a2a::*;

use super::identifiers::{arb_artifact_id, arb_context_id, arb_message_id, arb_task_id};

pub fn arb_task_state() -> impl Strategy<Value = TaskState> {
    prop_oneof![
        Just(TaskState::Submitted),
        Just(TaskState::Working),
        Just(TaskState::Completed),
        Just(TaskState::Failed),
        Just(TaskState::Canceled),
        Just(TaskState::Rejected),
        Just(TaskState::InputRequired),
        Just(TaskState::AuthRequired),
    ]
}

pub fn arb_protocol_binding() -> impl Strategy<Value = ProtocolBinding> {
    prop_oneof![
        Just(ProtocolBinding::JsonRpc),
        Just(ProtocolBinding::Grpc),
        Just(ProtocolBinding::HttpJson),
    ]
}

pub fn arb_message_role() -> impl Strategy<Value = MessageRole> {
    prop_oneof![Just(MessageRole::User), Just(MessageRole::Agent),]
}

pub fn arb_text_part() -> impl Strategy<Value = Part> {
    (
        "[a-zA-Z0-9 ]{1,100}",
        proptest::option::of("(text/plain|text/markdown)"),
    )
        .prop_map(|(text, media_type)| Part {
            media_type,
            ..Part::text(text)
        })
}

pub fn arb_file_part() -> impl Strategy<Value = Part> {
    (
        prop_oneof![
            "[a-zA-Z0-9+/]{0,48}".prop_map(PartContent::Raw),
            "https://example\\.com/[a-z]{1,20}".prop_map(PartContent::Url),
        ],
        proptest::option::of("[a-z]{1,20}\\.[a-z]{2,4}"),
        proptest::option::of("(text/plain|application/json|image/png)"),
    )
        .prop_map(|(content, filename, media_type)| Part {
            content,
            filename,
            media_type,
            metadata: None,
        })
}

pub fn arb_data_part() -> impl Strategy<Value = Part> {
    proptest::collection::hash_map("[a-z]{1,10}", "[a-zA-Z0-9]{1,20}", 0..5).prop_map(|map| {
        let data: serde_json::Map<String, serde_json::Value> = map
            .into_iter()
            .map(|(k, v)| (k, serde_json::Value::String(v)))
            .collect();
        Part::data(serde_json::Value::Object(data))
    })
}

pub fn arb_part() -> impl Strategy<Value = Part> {
    prop_oneof![arb_text_part(), arb_file_part(), arb_data_part(),]
}

pub fn arb_message() -> impl Strategy<Value = Message> {
    (
        arb_message_role(),
        proptest::collection::vec(arb_part(), 1..4),
        arb_message_id(),
        proptest::option::of(arb_task_id()),
        arb_context_id(),
    )
        .prop_map(|(role, parts, message_id, task_id, context_id)| Message {
            role,
            parts,
            message_id,
            task_id,
            context_id,
            metadata: None,
            extensions: None,
            reference_task_ids: None,
        })
}

pub fn arb_task_status() -> impl Strategy<Value = TaskStatus> {
    arb_task_state().prop_map(|state| TaskStatus {
        state,
        message: None,
        timestamp: None,
    })
}

pub fn arb_task_type() -> impl Strategy<Value = TaskType> {
    prop_oneof![Just(TaskType::McpExecution), Just(TaskType::AgentMessage),]
}

pub fn arb_task_metadata() -> impl Strategy<Value = TaskMetadata> {
    (
        arb_task_type(),
        "[a-z-]{1,20}",
        "[0-9]{4}-[0-9]{2}-[0-9]{2}",
    )
        .prop_map(|(task_type, agent_name, created_at)| TaskMetadata {
            task_type,
            agent_name,
            created_at,
            updated_at: None,
            started_at: None,
            completed_at: None,
            execution_time_ms: None,
            tool_name: None,
            mcp_server_name: None,
            input_tokens: None,
            output_tokens: None,
            model: None,
            execution_steps: None,
            extensions: serde_json::Map::new(),
        })
}

pub fn arb_artifact_metadata() -> impl Strategy<Value = ArtifactMetadata> {
    (
        arb_context_id(),
        arb_task_id(),
        "[a-z_]{1,20}",
        "[0-9]{4}-[0-9]{2}-[0-9]{2}",
    )
        .prop_map(
            |(context_id, task_id, artifact_type, created_at)| ArtifactMetadata {
                artifact_type,
                context_id,
                created_at,
                task_id,
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
        )
}

pub fn arb_artifact() -> impl Strategy<Value = Artifact> {
    (
        arb_artifact_id(),
        proptest::option::of("[a-zA-Z ]{1,30}"),
        proptest::option::of("[a-zA-Z ]{1,50}"),
        proptest::collection::vec(arb_part(), 1..3),
        arb_artifact_metadata(),
    )
        .prop_map(|(id, title, description, parts, metadata)| Artifact {
            id: SkillId::new(id),
            title,
            description,
            parts,
            extensions: vec![],
            metadata,
        })
}

pub fn arb_task() -> impl Strategy<Value = Task> {
    (
        arb_task_id(),
        arb_context_id(),
        arb_task_status(),
        proptest::option::of(proptest::collection::vec(arb_message(), 1..3)),
        proptest::option::of(proptest::collection::vec(arb_artifact(), 1..3)),
        proptest::option::of(arb_task_metadata()),
    )
        .prop_map(
            |(id, context_id, status, history, artifacts, metadata)| Task {
                id,
                context_id,
                status,
                history,
                artifacts,
                metadata,
                created_at: None,
                last_modified: None,
            },
        )
}

pub fn arb_agent_skill() -> impl Strategy<Value = AgentSkill> {
    (
        "[a-z-]{1,20}",
        "[a-zA-Z ]{1,30}",
        "[a-zA-Z ]{1,50}",
        proptest::collection::vec("[a-z]{1,10}", 0..5),
    )
        .prop_map(|(id, name, description, tags)| AgentSkill {
            id: SkillId::new(id),
            name,
            description,
            tags,
            examples: None,
            input_modes: None,
            output_modes: None,
            security_requirements: None,
        })
}

pub fn arb_agent_capabilities() -> impl Strategy<Value = AgentCapabilities> {
    (
        proptest::option::of(any::<bool>()),
        proptest::option::of(any::<bool>()),
        proptest::option::of(any::<bool>()),
    )
        .prop_map(
            |(streaming, push_notifications, extended_agent_card)| AgentCapabilities {
                streaming,
                push_notifications,
                extensions: None,
                extended_agent_card,
            },
        )
}

pub fn arb_api_key_location() -> impl Strategy<Value = ApiKeyLocation> {
    prop_oneof![
        Just(ApiKeyLocation::Query),
        Just(ApiKeyLocation::Header),
        Just(ApiKeyLocation::Cookie),
    ]
}

pub fn arb_oauth_flows() -> impl Strategy<Value = OAuthFlows> {
    let scopes = proptest::collection::hash_map("[a-z]{1,8}", "[a-zA-Z ]{1,20}", 0..3);
    prop_oneof![
        (scopes.clone(), any::<bool>()).prop_map(|(scopes, pkce_required)| {
            OAuthFlows::AuthorizationCode(AuthorizationCodeOAuthFlow {
                authorization_url: "https://example.com/authorize".to_string(),
                token_url: "https://example.com/token".to_string(),
                refresh_url: None,
                scopes,
                pkce_required,
            })
        }),
        scopes.clone().prop_map(|scopes| {
            OAuthFlows::ClientCredentials(ClientCredentialsOAuthFlow {
                token_url: "https://example.com/token".to_string(),
                refresh_url: Some("https://example.com/refresh".to_string()),
                scopes,
            })
        }),
        scopes.prop_map(|scopes| {
            OAuthFlows::DeviceCode(DeviceCodeOAuthFlow {
                device_authorization_url: "https://example.com/device".to_string(),
                token_url: "https://example.com/token".to_string(),
                refresh_url: None,
                scopes,
            })
        }),
    ]
}

pub fn arb_security_scheme() -> impl Strategy<Value = SecurityScheme> {
    prop_oneof![
        ("[a-z]{1,10}", arb_api_key_location()).prop_map(|(name, location)| {
            SecurityScheme::ApiKey(ApiKeySecurityScheme {
                name,
                location,
                description: None,
            })
        }),
        Just(SecurityScheme::HttpAuth(HttpAuthSecurityScheme {
            scheme: "bearer".to_string(),
            bearer_format: Some("JWT".to_string()),
            description: None,
        })),
        arb_oauth_flows().prop_map(|flows| {
            SecurityScheme::OAuth2(OAuth2SecurityScheme {
                description: None,
                flows,
                oauth2_metadata_url: None,
            })
        }),
        Just(SecurityScheme::OpenIdConnect(OpenIdConnectSecurityScheme {
            open_id_connect_url: "https://example.com/.well-known/openid-configuration".to_string(),
            description: None,
        })),
        Just(SecurityScheme::MutualTls(MutualTlsSecurityScheme {
            description: None
        })),
    ]
}

pub fn arb_agent_interface() -> impl Strategy<Value = AgentInterface> {
    ("https://example\\.com/[a-z]{1,10}", arb_protocol_binding()).prop_map(
        |(url, protocol_binding)| AgentInterface {
            url,
            protocol_binding,
            tenant: None,
            protocol_version: PROTOCOL_VERSION.to_string(),
        },
    )
}

pub fn arb_agent_card() -> impl Strategy<Value = AgentCard> {
    (
        "[a-zA-Z ]{1,30}",
        "[a-zA-Z ]{1,50}",
        proptest::collection::vec(arb_agent_interface(), 1..3),
        arb_agent_capabilities(),
        proptest::collection::vec(arb_agent_skill(), 0..5),
    )
        .prop_map(
            |(name, description, supported_interfaces, capabilities, skills)| AgentCard {
                name,
                description,
                supported_interfaces,
                version: "1.0.0".to_string(),
                icon_url: None,
                provider: None,
                documentation_url: None,
                capabilities,
                security_schemes: None,
                security_requirements: None,
                default_input_modes: vec!["text".to_string()],
                default_output_modes: vec!["text".to_string()],
                skills,
                signatures: None,
            },
        )
}
