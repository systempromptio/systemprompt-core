//! The kernel floor: the members `runtime` links whatever capability
//! features a build enables. With only `core,runtime` they are exactly
//! `reference()`, listed as the `runtime` row of
//! `scripts/composition-members.txt`, which CI compares with a
//! `core,runtime` build; this workspace unifies the facade's features to
//! `reference`, so here they are checked as present, in this order.

use systemprompt_test_fixtures::repo_path;

fn kernel_members() -> Vec<String> {
    let path = repo_path("scripts/composition-members.txt");
    let listed =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    listed
        .lines()
        .find_map(|line| {
            let mut fields = line.split_whitespace();
            (fields.next() == Some("runtime")).then(|| fields.next())?
        })
        .expect("the runtime composition is listed")
        .split(',')
        .map(str::to_owned)
        .collect()
}

#[test]
fn reference_holds_every_kernel_member_in_kernel_order() {
    let kernel_members = kernel_members();
    let ids = systemprompt::reference().member_ids();
    let kernel: Vec<&str> = ids
        .iter()
        .copied()
        .filter(|id| kernel_members.iter().any(|member| member == id))
        .collect();
    assert_eq!(kernel, kernel_members, "{ids:?}");
}
