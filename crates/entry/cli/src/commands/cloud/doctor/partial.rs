//! The preflight over a profile whose variables are not all set on this host.
//!
//! A profile with every value resolved runs the full preflight. Otherwise the
//! profile is inspected ([`ManagedProfile::inspect`]): every profile rule and
//! every doctor check runs when the key paths it reads all resolve here, with
//! exactly the code boot runs, and a failure blocks the deploy as it does for
//! a fully resolved profile. A rule or check that reads an unresolved value is
//! reported as not evaluated, naming the variables it needs, so the report is
//! never a clean pass built from checks that did not run.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::path::Path;

use systemprompt_manifest::Profile;
use systemprompt_manifest::profile::{Evaluation, Inspection, ManagedProfile, ProfileError};

use super::checks::{
    check_database_reachable, check_extension_configs, check_governance_hook_url,
    check_proxy_topology, check_required_secrets, check_signing_key,
};
use super::{
    CheckResult, DoctorReport, distributed, plane_checks, providers_check, resolve_secrets_from,
    restore_checks, run,
};

const HOOK_URL_INPUTS: &[&str] = &[
    "governance",
    "server.external_url",
    "server.host",
    "server.port",
    "server.extra_cors_origins",
];

pub async fn run_managed(
    managed: &ManagedProfile,
    profile_dir: &Path,
    distributed: bool,
) -> DoctorReport {
    if managed.unresolved().is_empty() {
        return match managed.profile() {
            Ok(profile) => run(&profile, profile_dir, distributed).await,
            Err(error) => DoctorReport {
                checks: vec![CheckResult::fail("profile", format!("{error}"))],
            },
        };
    }
    match managed.inspect() {
        Ok(inspection) => run_partial(&inspection, profile_dir, distributed).await,
        Err(error) => DoctorReport {
            checks: vec![CheckResult::fail("profile", format!("{error}"))],
        },
    }
}

type ProfileCheck = fn(&Profile) -> CheckResult;

const PROFILE_CHECKS: &[(&str, &[&str], ProfileCheck)] = &[
    (
        "proxy-topology",
        &["target", "cloud", "server.trusted_proxies"],
        check_proxy_topology,
    ),
    ("hook-url", HOOK_URL_INPUTS, check_governance_hook_url),
    (
        "instance-id",
        &["target", "server.instance_id", "server.trusted_proxies"],
        distributed::check_instance_id,
    ),
    (
        "trusted-proxies",
        &["target", "server.trusted_proxies"],
        distributed::check_trusted_proxies,
    ),
    (
        "storage-backup",
        &["server.role", "storage"],
        restore_checks::check_storage_backup,
    ),
    (
        "extension-config",
        &["paths", "security.signing_key_path", "extensions"],
        check_extension_configs,
    ),
    (
        "services-source",
        &["services"],
        plane_checks::check_services_source,
    ),
];

const SECRETS_DEPENDENT: &[&str] = &[
    "secrets",
    "signing-key",
    "database",
    "schema-version",
    "providers",
    "config-planes",
];

async fn run_partial(
    inspection: &Inspection<'_>,
    profile_dir: &Path,
    distributed: bool,
) -> DoctorReport {
    let mut checks = profile_rule_checks(inspection);
    for (name, inputs, check) in PROFILE_CHECKS {
        checks.push(outcome(name, inspection.evaluate(inputs, check)));
    }
    match inspection.evaluate(&["secrets"], |p| p.secrets.clone()) {
        Evaluation::Evaluated(config) => {
            let secrets = resolve_secrets_from(config.as_ref(), profile_dir, &mut checks).await;
            secrets_checks(inspection, profile_dir, &secrets, distributed, &mut checks).await;
        },
        Evaluation::NotEvaluated { variables } => {
            for name in SECRETS_DEPENDENT {
                checks.push(CheckResult::not_evaluated(name, &label(&variables)));
            }
        },
    }
    DoctorReport { checks }
}

async fn secrets_checks(
    inspection: &Inspection<'_>,
    profile_dir: &Path,
    secrets: &HashMap<String, String>,
    distributed: bool,
    checks: &mut Vec<CheckResult>,
) {
    checks.push(check_required_secrets(secrets));
    checks.push(outcome(
        "signing-key",
        inspection.evaluate(&["security.signing_key_path"], |p| {
            check_signing_key(p, profile_dir, secrets)
        }),
    ));
    checks.push(check_database_reachable(secrets).await);
    checks.push(restore_checks::check_schema_not_ahead(secrets).await);
    checks.push(outcome(
        "providers",
        inspection.evaluate(&["paths", "services"], |p| providers_check(p, secrets)),
    ));
    match inspection.evaluate(&["config_planes"], |p| {
        plane_checks::check_config_planes(p, secrets)
    }) {
        Evaluation::Evaluated(pending) => checks.push(pending.await),
        Evaluation::NotEvaluated { variables } => {
            checks.push(CheckResult::not_evaluated(
                "config-planes",
                &label(&variables),
            ));
        },
    }
    if distributed {
        match inspection.evaluate(&["server.host", "server.port"], |p| {
            distributed::run(p, secrets)
        }) {
            Evaluation::Evaluated(pending) => checks.extend(pending.await),
            Evaluation::NotEvaluated { variables } => {
                checks.push(CheckResult::not_evaluated(
                    "distributed",
                    &label(&variables),
                ));
            },
        }
    }
}

fn profile_rule_checks(inspection: &Inspection<'_>) -> Vec<CheckResult> {
    let report = inspection.validate();
    let mut checks = Vec::with_capacity(report.not_evaluated.len() + 1);
    if report.errors.is_empty() {
        checks.push(CheckResult::pass(
            "profile",
            "every profile rule whose inputs resolve on this host passed",
        ));
    } else {
        checks.push(CheckResult::fail(
            "profile",
            format!(
                "{}",
                ProfileError::Validation {
                    name: inspection_name(inspection),
                    errors: report.errors,
                }
            ),
        ));
    }
    for skipped in report.not_evaluated {
        checks.push(CheckResult::not_evaluated(
            skipped.name,
            &label(&skipped.variables),
        ));
    }
    checks
}

fn inspection_name(inspection: &Inspection<'_>) -> String {
    match inspection.evaluate(&["name"], |p| p.name.clone()) {
        Evaluation::Evaluated(name) => name,
        Evaluation::NotEvaluated { .. } => String::from("<unresolved>"),
    }
}

fn outcome(name: &'static str, evaluation: Evaluation<CheckResult>) -> CheckResult {
    match evaluation {
        Evaluation::Evaluated(result) => result,
        Evaluation::NotEvaluated { variables } => {
            CheckResult::not_evaluated(name, &label(&variables))
        },
    }
}

fn label(variables: &[String]) -> String {
    variables
        .iter()
        .map(|name| format!("${{{name}}}"))
        .collect::<Vec<_>>()
        .join(", ")
}
