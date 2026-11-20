//! The reference composition is a topological order of the members'
//! dependency edges, so schema installation (which runs in composition
//! order) creates every referenced table before the table that references it.

const EDGES: &[(&str, &str)] = &[
    ("users", "events"),
    ("logging", "database"),
    ("logging", "users"),
    ("authz", "users"),
    ("oauth", "users"),
    ("identity", "users"),
    ("identity", "oauth"),
    ("mcp", "users"),
    ("ai", "users"),
    ("ai", "mcp"),
    ("agent", "users"),
    ("agent", "oauth"),
    ("agent", "mcp"),
    ("agent", "ai"),
    ("analytics", "users"),
    ("analytics", "agent"),
    ("analytics", "authz"),
    ("content", "users"),
    ("content", "analytics"),
    ("files", "users"),
    ("files", "content"),
    ("brain", "users"),
    ("notifications", "users"),
    ("managed_resources", "ai"),
    ("managed_resources", "users"),
    ("gateway", "ai"),
    ("agent_runs", "users"),
    ("generator", "content"),
    ("scheduler", "users"),
    ("content-http", "content"),
    ("analytics-http", "analytics"),
    ("analytics-http", "content"),
    ("mcp-http", "mcp"),
];

#[test]
fn reference_order_respects_every_dependency_edge() {
    let ids = systemprompt::reference().member_ids();
    let position = |id: &str| ids.iter().position(|member| *member == id);
    for (dependent, dependency) in EDGES {
        let (Some(dependent_at), Some(dependency_at)) = (position(dependent), position(dependency))
        else {
            panic!("{dependent} or {dependency} missing from the reference composition: {ids:?}");
        };
        assert!(
            dependency_at < dependent_at,
            "{dependency} must precede {dependent}: {ids:?}"
        );
    }
}

#[test]
fn database_is_first() {
    assert_eq!(
        systemprompt::reference().member_ids().first(),
        Some(&"database")
    );
}
