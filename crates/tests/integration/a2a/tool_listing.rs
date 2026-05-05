use crate::common::context::TestContext;
use serde_json::json;

#[tokio::test]
async fn test_agent_registry_includes_tools() {
    let ctx = TestContext::new().await.expect("Failed to create test context");

    let response = ctx.http_client
        .get(&format!("{}/api/v1/agents/registry", ctx.base_url))
        .header("Authorization", format!("Bearer {}", ctx.auth_token))
        .send()
        .await
        .expect("Failed to get agent registry");

    assert_eq!(response.status(), 200, "Registry should be accessible");

    let body: serde_json::Value = response.json().await
        .expect("Failed to parse registry response");

    let agents = body.get("data")
        .and_then(|d| d.as_array())
        .expect("Registry should have data array");

    assert!(!agents.is_empty(), "Should have at least one agent");

    let edward = agents.iter()
        .find(|a| {
            a.get("name")
                .and_then(|n| n.as_str())
                .map(|n| n == "edward")
                .unwrap_or(false)
        })
        .expect("Should have edward agent");

    let skills = edward.get("skills")
        .and_then(|s| s.as_array());

    if let Some(skills) = skills {
        assert!(
            !skills.is_empty(),
            "Edward agent should have skills/tools from MCP servers"
        );

        let has_context_retrieval = skills.iter().any(|skill| {
            skill.get("name")
                .and_then(|n| n.as_str())
                .map(|n| n == "context_retrieval")
                .unwrap_or(false)
        });

        assert!(
            has_context_retrieval,
            "Edward should have context_retrieval tool from tyingshoelaces server"
        );

        for skill in skills {
            assert!(
                skill.get("name").is_some(),
                "Each skill should have a name"
            );

            assert!(
                skill.get("description").is_some(),
                "Each skill should have a description"
            );
        }
    } else {
        panic!("Edward agent should have skills field with MCP tools");
    }
}

#[tokio::test]
async fn test_agent_card_respects_permissions() {
    let ctx = TestContext::new().await.expect("Failed to create test context");

    let anon_token_response = ctx.http_client
        .post(&format!("{}/api/v1/core/oauth/session", ctx.base_url))
        .send()
        .await
        .expect("Failed to generate anonymous token");

    let token_data: serde_json::Value = anon_token_response.json().await
        .expect("Failed to parse token response");

    let anon_token = token_data.get("access_token")
        .and_then(|t| t.as_str())
        .expect("Should have access_token");

    let card_response = ctx.http_client
        .get(&format!("{}/api/v1/agents/edward/.well-known/agent-card.json", ctx.base_url))
        .header("Authorization", format!("Bearer {}", anon_token))
        .send()
        .await
        .expect("Failed to get agent card");

    assert_eq!(card_response.status(), 200, "Should get agent card");

    let card: serde_json::Value = card_response.json().await
        .expect("Failed to parse agent card");

    let capabilities = card.get("capabilities")
        .expect("Agent card should have capabilities");

    let tools = capabilities.get("tools")
        .and_then(|t| t.as_array())
        .expect("Capabilities should have tools array");

    assert!(
        !tools.is_empty(),
        "Anonymous user should see tools from servers with 'anonymous' scope"
    );

    for tool in tools {
        let tool_name = tool.get("name")
            .and_then(|n| n.as_str())
            .expect("Tool should have name");

        assert!(
            tool.get("description").is_some(),
            "Tool {} should have description",
            tool_name
        );

        assert!(
            tool.get("inputSchema").is_some(),
            "Tool {} should have inputSchema",
            tool_name
        );
    }
}

#[tokio::test]
async fn test_registry_filters_unauthorized_tools() {
    let ctx = TestContext::new().await.expect("Failed to create test context");

    let anon_token_response = ctx.http_client
        .post(&format!("{}/api/v1/core/oauth/session", ctx.base_url))
        .send()
        .await
        .expect("Failed to generate anonymous token");

    let token_data: serde_json::Value = anon_token_response.json().await
        .expect("Failed to parse token response");

    let anon_token = token_data.get("access_token")
        .and_then(|t| t.as_str())
        .expect("Should have access_token");

    let anon_response = ctx.http_client
        .get(&format!("{}/api/v1/agents/registry", ctx.base_url))
        .header("Authorization", format!("Bearer {}", anon_token))
        .send()
        .await
        .expect("Failed to get registry with anon token");

    assert_eq!(anon_response.status(), 200);

    let anon_body: serde_json::Value = anon_response.json().await
        .expect("Failed to parse anon registry response");

    let anon_agents = anon_body.get("data")
        .and_then(|d| d.as_array())
        .expect("Should have data array");

    let admin_response = ctx.http_client
        .get(&format!("{}/api/v1/agents/registry", ctx.base_url))
        .header("Authorization", format!("Bearer {}", ctx.auth_token))
        .send()
        .await
        .expect("Failed to get registry with admin token");

    let admin_body: serde_json::Value = admin_response.json().await
        .expect("Failed to parse admin registry response");

    let admin_agents = admin_body.get("data")
        .and_then(|d| d.as_array())
        .expect("Should have data array");

    for agent in anon_agents {
        let agent_name = agent.get("name")
            .and_then(|n| n.as_str())
            .expect("Agent should have name");

        let anon_skills = agent.get("skills")
            .and_then(|s| s.as_array())
            .map(|arr| arr.len())
            .unwrap_or(0);

        let admin_agent = admin_agents.iter()
            .find(|a| {
                a.get("name")
                    .and_then(|n| n.as_str())
                    .map(|n| n == agent_name)
                    .unwrap_or(false)
            });

        if let Some(admin_agent) = admin_agent {
            let admin_skills = admin_agent.get("skills")
                .and_then(|s| s.as_array())
                .map(|arr| arr.len())
                .unwrap_or(0);

            assert!(
                admin_skills >= anon_skills,
                "Admin should have same or more tools than anonymous user for agent {}",
                agent_name
            );
        }
    }
}

#[tokio::test]
async fn test_tool_schemas_are_valid() {
    let ctx = TestContext::new().await.expect("Failed to create test context");

    let card_response = ctx.http_client
        .get(&format!("{}/api/v1/agents/edward/.well-known/agent-card.json", ctx.base_url))
        .header("Authorization", format!("Bearer {}", ctx.auth_token))
        .send()
        .await
        .expect("Failed to get agent card");

    let card: serde_json::Value = card_response.json().await
        .expect("Failed to parse agent card");

    let tools = card.get("capabilities")
        .and_then(|c| c.get("tools"))
        .and_then(|t| t.as_array())
        .expect("Should have tools");

    for tool in tools {
        let tool_name = tool.get("name")
            .and_then(|n| n.as_str())
            .expect("Tool should have name");

        let input_schema = tool.get("inputSchema")
            .expect(&format!("Tool {} should have inputSchema", tool_name));

        assert!(
            input_schema.get("type").is_some(),
            "Tool {} inputSchema should have type",
            tool_name
        );

        if let Some(properties) = input_schema.get("properties") {
            assert!(
                properties.is_object(),
                "Tool {} properties should be an object",
                tool_name
            );
        }

        if let Some(required) = input_schema.get("required") {
            assert!(
                required.is_array(),
                "Tool {} required should be an array",
                tool_name
            );
        }
    }
}

#[tokio::test]
async fn test_multiple_mcp_servers_aggregated() {
    let ctx = TestContext::new().await.expect("Failed to create test context");

    let card_response = ctx.http_client
        .get(&format!("{}/api/v1/agents/edward/.well-known/agent-card.json", ctx.base_url))
        .header("Authorization", format!("Bearer {}", ctx.auth_token))
        .send()
        .await
        .expect("Failed to get agent card");

    let card: serde_json::Value = card_response.json().await
        .expect("Failed to parse agent card");

    let tools = card.get("capabilities")
        .and_then(|c| c.get("tools"))
        .and_then(|t| t.as_array())
        .expect("Should have tools");

    let server_names: std::collections::HashSet<String> = tools.iter()
        .filter_map(|tool| {
            tool.get("serverName")
                .and_then(|s| s.as_str())
                .map(|s| s.to_string())
        })
        .collect();

    assert!(
        server_names.len() > 1,
        "Edward should aggregate tools from multiple MCP servers, found servers: {:?}",
        server_names
    );

    let tool_names: Vec<String> = tools.iter()
        .filter_map(|tool| {
            tool.get("name")
                .and_then(|n| n.as_str())
                .map(|n| n.to_string())
        })
        .collect();

    let unique_names: std::collections::HashSet<_> = tool_names.iter().collect();

    assert_eq!(
        tool_names.len(),
        unique_names.len(),
        "Should not have duplicate tool names"
    );
}
