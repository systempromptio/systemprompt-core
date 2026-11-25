//! The kernel floor: the members `runtime` links whatever capability
//! features a build enables. With only `core,runtime` they are exactly
//! `reference()`; this workspace unifies the facade's features to
//! `reference`, so here they are checked as present, in this order.

const KERNEL_MEMBERS: &[&str] = &[
    "database",
    "events",
    "users",
    "logging",
    "authz",
    "oauth",
    "identity",
    "mcp_host",
    "documents",
    "files",
    "gateway",
    "scheduler",
    "gateway-http",
];

#[test]
fn reference_holds_every_kernel_member_in_kernel_order() {
    let ids = systemprompt::reference().member_ids();
    let kernel: Vec<&str> = ids
        .iter()
        .copied()
        .filter(|id| KERNEL_MEMBERS.contains(id))
        .collect();
    assert_eq!(kernel, KERNEL_MEMBERS, "{ids:?}");
}
