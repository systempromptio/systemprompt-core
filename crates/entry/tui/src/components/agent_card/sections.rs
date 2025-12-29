use ratatui::prelude::*;
use systemprompt_models::{AgentCard, McpToolsParams, SecurityScheme};

use super::metadata::shorten_path;
use crate::state::AgentDisplayMetadata;

pub fn build_status_line(info: &crate::state::AgentInfo) -> Vec<Line<'static>> {
    use crate::state::AgentConnectionStatus;

    let (status_text, status_style) = match &info.status {
        AgentConnectionStatus::Connected => ("Connected", Style::default().fg(Color::Green)),
        AgentConnectionStatus::Connecting => ("Connecting...", Style::default().fg(Color::Yellow)),
        AgentConnectionStatus::Disconnected => {
            ("Disconnected", Style::default().fg(Color::DarkGray))
        },
        AgentConnectionStatus::Error(e) => (e.as_str(), Style::default().fg(Color::Red)),
    };
    vec![Line::from(vec![
        Span::styled("Status: ", Style::default().fg(Color::Cyan)),
        Span::styled(status_text.to_string(), status_style),
    ])]
}

pub fn build_provider_line(card: &AgentCard) -> Vec<Line<'static>> {
    card.provider.as_ref().map_or_else(Vec::new, |provider| {
        vec![Line::from(vec![
            Span::styled("Provider: ", Style::default().fg(Color::Cyan)),
            Span::styled(
                provider.organization.clone(),
                Style::default().fg(Color::White),
            ),
        ])]
    })
}

pub fn build_description_section(card: &AgentCard) -> Vec<Line<'static>> {
    vec![
        Line::from(vec![Span::styled(
            "Description",
            Style::default().fg(Color::Cyan).bold(),
        )]),
        Line::from(vec![Span::styled(
            card.description.clone(),
            Style::default().fg(Color::White),
        )]),
    ]
}

pub fn build_capabilities_section(card: &AgentCard) -> Vec<Line<'static>> {
    let mut cap_items: Vec<&str> = Vec::new();
    if card.capabilities.streaming.unwrap_or(false) {
        cap_items.push("Streaming");
    }
    if card.capabilities.push_notifications.unwrap_or(false) {
        cap_items.push("Push");
    }
    if card.capabilities.state_transition_history.unwrap_or(false) {
        cap_items.push("History");
    }

    let caps_span = if cap_items.is_empty() {
        Span::styled("None", Style::default().fg(Color::DarkGray))
    } else {
        Span::styled(cap_items.join(", "), Style::default().fg(Color::Green))
    };

    vec![Line::from(vec![
        Span::styled("Capabilities: ", Style::default().fg(Color::Cyan).bold()),
        caps_span,
    ])]
}

pub fn build_io_modes_line(card: &AgentCard) -> Vec<Line<'static>> {
    if card.default_input_modes.is_empty() && card.default_output_modes.is_empty() {
        return Vec::new();
    }
    let input_modes = card.default_input_modes.join(", ");
    let output_modes = card.default_output_modes.join(", ");
    vec![Line::from(vec![
        Span::styled("I/O: ", Style::default().fg(Color::Cyan)),
        Span::styled(
            format!("{} → {}", input_modes, output_modes),
            Style::default().fg(Color::DarkGray),
        ),
    ])]
}

pub fn build_security_section(card: &AgentCard) -> Vec<Line<'static>> {
    let Some(security_schemes) = &card.security_schemes else {
        return Vec::new();
    };
    if security_schemes.is_empty() {
        return Vec::new();
    }

    let mut lines = vec![Line::from(vec![Span::styled(
        "Security",
        Style::default().fg(Color::Cyan).bold(),
    )])];

    for (name, scheme) in security_schemes {
        let scheme_info = format_security_scheme(scheme);
        lines.push(Line::from(vec![
            Span::styled("  • ", Style::default().fg(Color::DarkGray)),
            Span::styled(name.clone(), Style::default().fg(Color::Blue)),
            Span::styled(": ", Style::default().fg(Color::DarkGray)),
            Span::styled(scheme_info, Style::default().fg(Color::White)),
        ]));
    }
    lines.push(Line::from(""));
    lines
}

fn format_security_scheme(scheme: &SecurityScheme) -> String {
    match scheme {
        SecurityScheme::OAuth2 { flows, .. } => {
            let mut flow_types: Vec<&str> = Vec::new();
            if flows.authorization_code.is_some() {
                flow_types.push("auth_code");
            }
            if flows.client_credentials.is_some() {
                flow_types.push("client_creds");
            }
            if flows.implicit.is_some() {
                flow_types.push("implicit");
            }
            if flows.password.is_some() {
                flow_types.push("password");
            }
            format!("OAuth2 ({})", flow_types.join(", "))
        },
        SecurityScheme::ApiKey { location, .. } => format!("API Key ({})", location),
        SecurityScheme::Http { scheme, .. } => format!("HTTP ({})", scheme),
        SecurityScheme::OpenIdConnect { .. } => "OpenID Connect".to_string(),
        SecurityScheme::MutualTls { .. } => "Mutual TLS".to_string(),
    }
}

pub fn build_skills_section(
    card: &AgentCard,
    metadata: Option<&AgentDisplayMetadata>,
) -> Vec<Line<'static>> {
    if card.skills.is_empty() {
        return Vec::new();
    }

    let mut lines = vec![Line::from(vec![Span::styled(
        format!("Skills ({})", card.skills.len()),
        Style::default().fg(Color::Cyan).bold(),
    )])];

    for skill in &card.skills {
        lines.push(Line::from(vec![
            Span::styled("  • ", Style::default().fg(Color::DarkGray)),
            Span::styled(skill.name.clone(), Style::default().fg(Color::Yellow)),
        ]));

        if let Some(meta) = metadata {
            if let Some(path) = meta.skill_paths.get(&skill.id) {
                lines.push(Line::from(vec![Span::styled(
                    format!("    {}", shorten_path(path)),
                    Style::default().fg(Color::Blue).italic(),
                )]));
            }
        }

        if !skill.description.is_empty() {
            let desc = if skill.description.len() > 60 {
                format!("{}...", &skill.description[..60])
            } else {
                skill.description.clone()
            };
            lines.push(Line::from(vec![Span::styled(
                format!("    {}", desc),
                Style::default().fg(Color::DarkGray).italic(),
            )]));
        }
        if !skill.tags.is_empty() {
            lines.push(Line::from(vec![Span::styled(
                format!("    [{}]", skill.tags.join(", ")),
                Style::default().fg(Color::DarkGray),
            )]));
        }
    }
    lines.push(Line::from(""));
    lines
}

pub fn build_mcp_servers_section(
    card: &AgentCard,
    metadata: Option<&AgentDisplayMetadata>,
) -> Vec<Line<'static>> {
    let Some(extensions) = &card.capabilities.extensions else {
        return Vec::new();
    };

    let mcp_params = extensions
        .iter()
        .find(|ext| ext.uri == "systemprompt:mcp-tools")
        .and_then(|ext| ext.params.as_ref())
        .and_then(|p| serde_json::from_value::<McpToolsParams>(p.clone()).ok());

    let Some(params) = mcp_params else {
        return Vec::new();
    };
    if params.servers.is_empty() {
        return Vec::new();
    }

    let mut lines = vec![Line::from(vec![Span::styled(
        format!("MCP Servers ({})", params.servers.len()),
        Style::default().fg(Color::Cyan).bold(),
    )])];

    for server in &params.servers {
        let status_color = if server.status == "connected" {
            Color::Green
        } else {
            Color::DarkGray
        };
        lines.push(Line::from(vec![
            Span::styled("  • ", Style::default().fg(Color::DarkGray)),
            Span::styled(server.name.clone(), Style::default().fg(Color::Magenta)),
            Span::styled(
                format!(" [{}]", server.status),
                Style::default().fg(status_color),
            ),
        ]));

        if let Some(meta) = metadata {
            if let Some(config_path) = meta.mcp_server_paths.get(&server.name) {
                lines.push(Line::from(vec![
                    Span::styled("    config: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        config_path.clone(),
                        Style::default().fg(Color::Blue).italic(),
                    ),
                ]));
            }
        }

        if let Some(tools) = &server.tools {
            if !tools.is_empty() {
                let tool_names: Vec<String> = tools.iter().map(|t| t.name.clone()).collect();
                lines.push(Line::from(vec![Span::styled(
                    format!("    Tools: {}", tool_names.join(", ")),
                    Style::default().fg(Color::DarkGray),
                )]));
            }
        }
    }
    lines.push(Line::from(""));
    lines
}

pub fn build_docs_line(card: &AgentCard) -> Vec<Line<'static>> {
    card.documentation_url
        .as_ref()
        .map_or_else(Vec::new, |doc_url| {
            vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled("Docs: ", Style::default().fg(Color::Cyan)),
                    Span::styled(
                        doc_url.clone(),
                        Style::default().fg(Color::Blue).underlined(),
                    ),
                ]),
            ]
        })
}
