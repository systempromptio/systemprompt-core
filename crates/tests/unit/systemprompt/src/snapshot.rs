//! T1 snapshot, feature shape `full` (plus `slack`/`teams` when the build
//! carries them): the member routes, the command paths and the jobs the
//! reference composition admits equal the baseline captured before the
//! composition contract, except for the differences listed in the
//! expected-delta tables, each with its reason. An unlisted difference fails,
//! and so does a listed difference that no longer occurs.
//!
//! Routes compare the served paths of the admitted route sets and gateway
//! routes (a gateway route is served below `/v1`) with the baseline rows a
//! member owns. The kernel's own routes are mounted by the API server, not
//! admitted. The `slack`/`teams` rows are compared when the build admits
//! those members.
//! Commands compare every path of the assembled command line (kernel and
//! member commands). Jobs compare the admitted jobs' names.

use std::collections::BTreeSet;

use clap::Command;
use systemprompt::reference;
use systemprompt_cli::kernel::{CommandRegistry, kernel_commands};
use systemprompt_extension::router::full_path;
use systemprompt_models::modules::ApiPaths;

use crate::probe::profile;

const ROUTES: &str = include_str!("../fixtures/baseline/routes.tsv");
const COMMANDS: &str = include_str!("../fixtures/baseline/commands.tsv");
const JOBS: &str = include_str!("../fixtures/baseline/jobs.tsv");

const OPT_IN_SHAPES: [&str; 2] = ["slack", "teams"];

const MARKDOWN_ALTERNATE: &str = "the content Markdown alternate is one static route answering 404 unless \
     negotiation is on and the suffix matches, so the route shape no longer depends on the profile \
     (sheet amendment 35)";
const PLUGINS_REMOVED: &str = "the plugins subcommand is removed with the registration mechanism it \
     inspected";
const RESEAL_ADDED: &str = "admin secrets reseal re-seals pre-0.88 ciphertext and gateway journal \
     segments under each member's key (amendments 512, 545)";

const ROUTE_DELTA: &[(&str, Side, &str)] = &[
    (
        "GET /api/v1/content/{source_id}/{slug}/md",
        Side::OnlyBaseline,
        MARKDOWN_ALTERNATE,
    ),
    (
        "GET /api/v1/content/{source_id}/{slug}/{suffix}",
        Side::OnlyAdmitted,
        MARKDOWN_ALTERNATE,
    ),
];
const COMMAND_DELTA: &[(&str, Side, &str)] = &[
    (
        "plugins capabilities templates",
        Side::OnlyBaseline,
        PLUGINS_REMOVED,
    ),
    (
        "plugins capabilities tools",
        Side::OnlyBaseline,
        PLUGINS_REMOVED,
    ),
    ("plugins validate", Side::OnlyBaseline, PLUGINS_REMOVED),
    ("admin secrets", Side::OnlyAdmitted, RESEAL_ADDED),
    ("admin secrets reseal", Side::OnlyAdmitted, RESEAL_ADDED),
];
const JOB_DELTA: &[(&str, Side, &str)] = &[];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    OnlyBaseline,
    OnlyAdmitted,
}

fn column(tsv: &str, index: usize, keep: impl Fn(&[&str]) -> bool) -> BTreeSet<String> {
    tsv.lines()
        .skip(1)
        .filter(|line| !line.is_empty())
        .map(|line| line.split('\t').collect::<Vec<_>>())
        .filter(|columns| keep(columns))
        .map(|columns| columns[index].to_owned())
        .collect()
}

fn assert_matches(
    what: &str,
    baseline: &BTreeSet<String>,
    admitted: &BTreeSet<String>,
    delta: &[(&str, Side, &str)],
) {
    let mut found: Vec<(String, Side)> = baseline
        .difference(admitted)
        .map(|key| (key.clone(), Side::OnlyBaseline))
        .chain(
            admitted
                .difference(baseline)
                .map(|key| (key.clone(), Side::OnlyAdmitted)),
        )
        .collect();
    found.sort_by(|a, b| a.0.cmp(&b.0));
    let unexplained: Vec<&(String, Side)> = found
        .iter()
        .filter(|(key, side)| !delta.iter().any(|(k, s, _)| k == key && s == side))
        .collect();
    let stale: Vec<&(&str, Side, &str)> = delta
        .iter()
        .filter(|(k, s, _)| !found.iter().any(|(key, side)| key == k && side == s))
        .collect();
    assert!(
        unexplained.is_empty() && stale.is_empty(),
        "{what}: unexplained differences {unexplained:#?}; listed differences that do not occur \
         {stale:#?}"
    );
}

fn route_key(method: &str, path: &str) -> String {
    format!("{method} {path}")
}

#[test]
fn admitted_member_routes_equal_the_baseline() {
    let admitted = reference()
        .build(&profile(&[]))
        .expect("the reference composition admits");
    let carried: BTreeSet<&str> = admitted
        .built()
        .members()
        .map(|member| member.metadata.id)
        .filter(|id| OPT_IN_SHAPES.contains(id))
        .collect();
    let baseline: BTreeSet<String> = ROUTES
        .lines()
        .skip(1)
        .filter(|line| !line.is_empty())
        .map(|line| line.split('\t').collect::<Vec<_>>())
        .filter(|c| c[7] != "kernel" && (!OPT_IN_SHAPES.contains(&c[7]) || carried.contains(c[7])))
        .map(|c| route_key(c[0], c[1]))
        .collect();
    let members =
        admitted.routes().iter().flat_map(|admitted| {
            let base = admitted.set.base();
            admitted.set.defs().iter().map(move |def| {
                route_key(&def.method.to_string(), &full_path(base, def.path.as_str()))
            })
        });
    let gateway = admitted.gateway_routes().iter().flat_map(|gateway| {
        gateway.decl.router.defs().iter().map(|def| {
            route_key(
                &def.method.to_string(),
                &format!("{}{}", ApiPaths::GATEWAY_BASE, def.path.as_str()),
            )
        })
    });
    let routes: BTreeSet<String> = members.chain(gateway).collect();
    assert!(
        baseline.len() > 30,
        "the baseline was read: {}",
        baseline.len()
    );
    assert_matches("routes", &baseline, &routes, ROUTE_DELTA);
}

fn command_paths(command: &Command, prefix: &str, out: &mut BTreeSet<String>) {
    for sub in command.get_subcommands() {
        if sub.get_name() == "help" {
            continue;
        }
        let path = if prefix.is_empty() {
            sub.get_name().to_owned()
        } else {
            format!("{prefix} {}", sub.get_name())
        };
        command_paths(sub, &path, out);
        out.insert(path);
    }
}

#[test]
fn admitted_command_paths_equal_the_baseline() {
    let declared = reference()
        .declare()
        .expect("the reference composition declares");
    let registry = CommandRegistry::build(&declared.commands(), &kernel_commands())
        .expect("the registry accepts the reference commands");
    let root = systemprompt_cli::args::command(&registry).expect("the command line assembles");
    let mut paths = BTreeSet::new();
    command_paths(&root, "", &mut paths);
    let baseline = column(COMMANDS, 0, |_| true);
    assert!(
        baseline.len() > 400,
        "the baseline was read: {}",
        baseline.len()
    );
    assert_matches("commands", &baseline, &paths, COMMAND_DELTA);
}

#[test]
fn admitted_jobs_equal_the_baseline() {
    let admitted = reference()
        .build(&profile(&[]))
        .expect("the reference composition admits");
    let jobs: BTreeSet<String> = admitted
        .built()
        .jobs()
        .map(|(_, job)| job.job().name().to_owned())
        .collect();
    let baseline = column(JOBS, 0, |_| true);
    assert!(
        baseline.len() > 40,
        "the baseline was read: {}",
        baseline.len()
    );
    assert_matches("jobs", &baseline, &jobs, JOB_DELTA);
}
