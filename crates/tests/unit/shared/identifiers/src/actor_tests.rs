//! Coverage for the `Actor` principal/surface attribution type and its
//! `ActorKind` / `ActorKindTag` enums.

use systemprompt_identifiers::{Actor, ActorKind, ActorKindTag, UserId};

fn user() -> UserId {
    UserId::new("user_abc")
}

#[test]
fn user_actor_audit_columns() {
    let actor = Actor::user(user());
    assert_eq!(actor.kind, ActorKind::User);
    assert_eq!(actor.audit_columns(), ("user", "user_abc"));
}

#[test]
fn anonymous_actor_audit_columns() {
    let actor = Actor::anonymous(UserId::new("anonymous_42"));
    assert_eq!(actor.kind, ActorKind::Anonymous);
    assert_eq!(actor.audit_columns(), ("anonymous", "anonymous_42"));
}

#[test]
fn system_actor_audit_columns() {
    let actor = Actor::system(UserId::new("system_admin"));
    assert_eq!(actor.kind, ActorKind::System);
    assert_eq!(actor.audit_columns(), ("system", "system_admin"));
}

#[test]
fn job_actor_uses_job_name_as_actor_id() {
    let actor = Actor::job(user(), "database_cleanup");
    assert_eq!(
        actor.kind,
        ActorKind::Job {
            job_name: "database_cleanup".to_owned()
        }
    );
    assert_eq!(actor.audit_columns(), ("job", "database_cleanup"));
}

#[test]
fn mcp_actor_uses_server_name_as_actor_id() {
    let actor = Actor::mcp(user(), "filesystem");
    assert_eq!(
        actor.kind,
        ActorKind::Mcp {
            server_name: "filesystem".to_owned()
        }
    );
    assert_eq!(actor.audit_columns(), ("mcp", "filesystem"));
}

#[test]
fn agent_actor_uses_agent_id_as_actor_id() {
    let actor = Actor::agent(user(), "developer_agent");
    assert_eq!(
        actor.kind,
        ActorKind::Agent {
            agent_id: "developer_agent".to_owned()
        }
    );
    assert_eq!(actor.audit_columns(), ("agent", "developer_agent"));
}

#[test]
fn from_tool_name_mcp_prefix_extracts_server() {
    let actor = Actor::from_tool_name(user(), None, "mcp__filesystem__read_file");
    assert_eq!(
        actor.kind,
        ActorKind::Mcp {
            server_name: "filesystem".to_owned()
        }
    );
    assert_eq!(actor.audit_columns(), ("mcp", "filesystem"));
}

#[test]
fn from_tool_name_mcp_prefix_takes_priority_over_agent() {
    let actor = Actor::from_tool_name(user(), Some("some_agent"), "mcp__github__create_issue");
    assert_eq!(
        actor.kind,
        ActorKind::Mcp {
            server_name: "github".to_owned()
        }
    );
}

#[test]
fn from_tool_name_empty_server_falls_through_to_agent() {
    let actor = Actor::from_tool_name(user(), Some("a1"), "mcp____tool");
    assert_eq!(
        actor.kind,
        ActorKind::Agent {
            agent_id: "a1".to_owned()
        }
    );
}

#[test]
fn from_tool_name_with_agent_id_yields_agent() {
    let actor = Actor::from_tool_name(user(), Some("my_agent"), "Bash");
    assert_eq!(
        actor.kind,
        ActorKind::Agent {
            agent_id: "my_agent".to_owned()
        }
    );
}

#[test]
fn from_tool_name_empty_agent_id_yields_user() {
    let actor = Actor::from_tool_name(user(), Some(""), "Bash");
    assert_eq!(actor.kind, ActorKind::User);
}

#[test]
fn from_tool_name_no_agent_yields_user() {
    let actor = Actor::from_tool_name(user(), None, "Read");
    assert_eq!(actor.kind, ActorKind::User);
}

#[test]
fn actor_kind_as_str_all_variants() {
    assert_eq!(ActorKind::User.as_str(), "user");
    assert_eq!(ActorKind::Anonymous.as_str(), "anonymous");
    assert_eq!(ActorKind::System.as_str(), "system");
    assert_eq!(
        ActorKind::Job {
            job_name: "j".to_owned()
        }
        .as_str(),
        "job"
    );
    assert_eq!(
        ActorKind::Mcp {
            server_name: "s".to_owned()
        }
        .as_str(),
        "mcp"
    );
    assert_eq!(
        ActorKind::Agent {
            agent_id: "a".to_owned()
        }
        .as_str(),
        "agent"
    );
}

#[test]
fn actor_kind_display_matches_as_str() {
    assert_eq!(format!("{}", ActorKind::System), "system");
    assert_eq!(
        format!(
            "{}",
            ActorKind::Mcp {
                server_name: "fs".to_owned()
            }
        ),
        "mcp"
    );
}

#[test]
fn actor_kind_tag_maps_each_variant() {
    assert_eq!(ActorKind::User.tag(), ActorKindTag::User);
    assert_eq!(ActorKind::Anonymous.tag(), ActorKindTag::Anonymous);
    assert_eq!(ActorKind::System.tag(), ActorKindTag::System);
    assert_eq!(
        ActorKind::Job {
            job_name: "j".to_owned()
        }
        .tag(),
        ActorKindTag::Job
    );
    assert_eq!(
        ActorKind::Mcp {
            server_name: "s".to_owned()
        }
        .tag(),
        ActorKindTag::Mcp
    );
    assert_eq!(
        ActorKind::Agent {
            agent_id: "a".to_owned()
        }
        .tag(),
        ActorKindTag::Agent
    );
}

#[test]
fn actor_kind_tag_as_str_and_display() {
    assert_eq!(ActorKindTag::User.as_str(), "user");
    assert_eq!(ActorKindTag::Anonymous.as_str(), "anonymous");
    assert_eq!(ActorKindTag::System.as_str(), "system");
    assert_eq!(ActorKindTag::Job.as_str(), "job");
    assert_eq!(ActorKindTag::Mcp.as_str(), "mcp");
    assert_eq!(ActorKindTag::Agent.as_str(), "agent");
    assert_eq!(format!("{}", ActorKindTag::Agent), "agent");
}

#[test]
fn actor_kind_serde_tagged_roundtrip() {
    let kind = ActorKind::Job {
        job_name: "cleanup".to_owned(),
    };
    let json = serde_json::to_value(&kind).unwrap();
    assert_eq!(json["kind"], "job");
    assert_eq!(json["job_name"], "cleanup");
    let back: ActorKind = serde_json::from_value(json).unwrap();
    assert_eq!(back, kind);
}

#[test]
fn actor_kind_tag_serde_snake_case() {
    let json = serde_json::to_string(&ActorKindTag::Anonymous).unwrap();
    assert_eq!(json, "\"anonymous\"");
    let back: ActorKindTag = serde_json::from_str(&json).unwrap();
    assert_eq!(back, ActorKindTag::Anonymous);
}
