#![allow(clippy::all)]

#[cfg(test)]
mod agent_health;
#[cfg(test)]
mod agent_health_i18n;
#[cfg(test)]
mod fixture_verdicts;
#[cfg(test)]
mod force_dark;
#[cfg(test)]
mod hermes_profile;
#[cfg(test)]
mod host_reapply;
#[cfg(test)]
mod inconclusive_state;
#[cfg(test)]
mod wire_hosts;
#[cfg(test)]
mod wire_ipc;
#[cfg(test)]
mod wire_payloads;

#[cfg(test)]
mod semantic_state;

#[cfg(test)]
fn repo_path(relative: &str) -> std::path::PathBuf {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(5)
        .expect("crates/tests/unit/bridge/verdicts sits five levels under the repo root");
    let path = root.join(relative);
    assert!(
        path.exists(),
        "{} does not exist under the repo root",
        path.display()
    );
    path
}
