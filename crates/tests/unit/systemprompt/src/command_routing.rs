//! Every command a member contributes keeps the place of execution recorded
//! for it in the routing baseline: the routing class (local only, read-only
//! remote, mutating remote) its descriptor resolves to through the assembled
//! command line equals the baseline row, and every member leaf of the command
//! baseline has a row. A descriptor change that moves a command between this
//! machine and a tenant fails here until the baseline row changes with it.

use std::collections::BTreeSet;

use systemprompt::reference;
use systemprompt_cli::kernel::{CommandRegistry, classify_command_line, kernel_commands};
use systemprompt_extension::cli::RoutingClass;

use crate::probe::profile;

const COMMANDS: &str = include_str!("../fixtures/baseline/commands.tsv");
const ROUTING: &str = include_str!("../fixtures/baseline/command_routing.tsv");

struct Row<'a> {
    path: &'a str,
    args: &'a str,
    routing: &'a str,
}

fn rows() -> Vec<Row<'static>> {
    ROUTING
        .lines()
        .skip(1)
        .filter(|line| !line.is_empty())
        .map(|line| {
            let columns: Vec<&str> = line.split('\t').collect();
            assert_eq!(columns.len(), 3, "malformed routing row: {line}");
            Row {
                path: columns[0],
                args: columns[1],
                routing: columns[2],
            }
        })
        .collect()
}

fn member_leaves() -> BTreeSet<&'static str> {
    COMMANDS
        .lines()
        .skip(1)
        .filter(|line| !line.is_empty())
        .map(|line| line.split('\t').collect::<Vec<_>>())
        .filter(|columns| columns[1] == "leaf" && columns[3] != "kernel")
        .map(|columns| columns[0])
        .collect()
}

const fn routing_name(class: RoutingClass) -> &'static str {
    match class {
        RoutingClass::LocalOnly => "local-only",
        RoutingClass::ReadOnly => "read-only",
        RoutingClass::Mutating => "mutating",
    }
}

#[test]
fn every_member_leaf_has_a_recorded_routing_class() {
    let leaves = member_leaves();
    let recorded: BTreeSet<&str> = rows().iter().map(|row| row.path).collect();
    assert!(leaves.len() > 90, "the command baseline was read: {}", leaves.len());
    let missing: Vec<&&str> = leaves.difference(&recorded).collect();
    let stale: Vec<&&str> = recorded.difference(&leaves).collect();
    assert!(
        missing.is_empty() && stale.is_empty(),
        "member leaves without a routing row {missing:#?}; routing rows for no member leaf \
         {stale:#?}"
    );
}

#[test]
fn a_member_commands_routing_class_equals_the_baseline() {
    let declared = reference()
        .declare()
        .expect("the reference composition declares");
    let registry = CommandRegistry::build(&declared.commands(), &kernel_commands())
        .expect("the registry accepts the reference commands");
    let profile = profile(&[]);
    let mut differences = Vec::new();
    for row in rows() {
        let argv: Vec<String> = row
            .path
            .split(' ')
            .chain(row.args.split(' ').filter(|arg| *arg != "-"))
            .map(str::to_owned)
            .collect();
        let described = classify_command_line(&registry, &profile, &argv)
            .unwrap_or_else(|error| panic!("{argv:?} is classified: {error:#}"));
        let routing = routing_name(described.descriptor.routing_class());
        if routing != row.routing {
            differences.push(format!(
                "{}: baseline {}, descriptor {routing}",
                row.path, row.routing
            ));
        }
    }
    assert!(
        differences.is_empty(),
        "routing class changed without the baseline: {differences:#?}"
    );
}

#[test]
fn core_skills_runs_on_this_machine() {
    let skills: Vec<Row<'_>> = rows()
        .into_iter()
        .filter(|row| row.path.starts_with("core skills "))
        .collect();
    assert_eq!(skills.len(), 2);
    assert!(skills.iter().all(|row| row.routing == "local-only"));
}
