use systemprompt_extension::reserved::{
    CoreId, RESERVED_PATHS, ReservedPath, ReservedReach, nested_rows_are_bounded,
    nested_under_kernel,
};

const KERNEL_MCP: ReservedPath = ReservedPath {
    owner: CoreId::Kernel,
    path: "/api/v1/mcp",
    reach: ReservedReach::Prefix,
};

fn member_row(reach: ReservedReach) -> ReservedPath {
    ReservedPath {
        owner: CoreId::McpHttp,
        path: "/api/v1/mcp/registry",
        reach,
    }
}

#[test]
fn every_member_row_nested_under_a_kernel_row_is_bounded() {
    assert!(nested_rows_are_bounded(RESERVED_PATHS));
    for entry in RESERVED_PATHS
        .iter()
        .filter(|entry| entry.owner != CoreId::Kernel)
        .filter(|entry| nested_under_kernel(entry, RESERVED_PATHS))
    {
        assert_ne!(
            entry.reach,
            ReservedReach::Prefix,
            "{} is nested under a kernel row",
            entry.path
        );
    }
}

#[test]
fn the_live_table_holds_nested_member_rows() {
    let nested: Vec<&str> = RESERVED_PATHS
        .iter()
        .filter(|entry| entry.owner != CoreId::Kernel)
        .filter(|entry| nested_under_kernel(entry, RESERVED_PATHS))
        .map(|entry| entry.path)
        .collect();
    assert!(nested.contains(&"/api/v1/mcp/registry"));
    assert!(nested.contains(&"/api/v1/admin/notifications"));
}

#[test]
fn a_nested_prefix_row_is_refused() {
    let table = [KERNEL_MCP, member_row(ReservedReach::Prefix)];
    assert!(!nested_rows_are_bounded(&table));
}

#[test]
fn a_nested_exact_or_subtree_row_is_accepted() {
    for reach in [ReservedReach::Exact, ReservedReach::Subtree] {
        let table = [KERNEL_MCP, member_row(reach)];
        assert!(nested_rows_are_bounded(&table));
    }
}

#[test]
fn a_member_prefix_row_outside_kernel_rows_is_accepted() {
    let table = [
        KERNEL_MCP,
        ReservedPath {
            owner: CoreId::MarketplaceHttp,
            path: "/api/v1/marketplace",
            reach: ReservedReach::Prefix,
        },
    ];
    assert!(nested_rows_are_bounded(&table));
}

#[test]
fn a_row_under_an_exact_kernel_row_is_not_nested() {
    let kernel = ReservedPath {
        owner: CoreId::Kernel,
        path: "/api/v1",
        reach: ReservedReach::Exact,
    };
    assert!(!nested_under_kernel(
        &member_row(ReservedReach::Prefix),
        &[kernel]
    ));
}
