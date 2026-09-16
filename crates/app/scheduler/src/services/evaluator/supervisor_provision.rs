//! Workspace and network provisioning for a claimed evaluator execution.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{
    BTreeMap, CaseContent, EvalExecutionId, EvaluationTrafficClass, EvaluatorSupervisor,
    ExecutionAssignment, ExecutionLease, ExecutionLimits, ExecutionNetwork, NativeClient, PathBuf,
    ResourceContent, RubricContent, SchedulerError, SchedulerResult, UserId, VariantSpec,
    WorkspaceDirectory, internal, materialize_root, materialize_skills, workspace_state,
    write_private,
};
use systemprompt_evaluation::repository::experiments::ExecutionAccess;

pub(super) struct ProvisionedWorkspace {
    pub directory: PathBuf,
    pub guard: WorkspaceDirectory,
    pub home: PathBuf,
    pub installed_skill_state: BTreeMap<String, String>,
}

#[derive(Clone, Copy)]
pub(super) struct WorkspaceRequest<'a> {
    pub assignment: &'a ExecutionAssignment,
    pub access: &'a ExecutionAccess,
    pub execution_id: &'a EvalExecutionId,
    pub suffix: &'a str,
    pub client: &'a NativeClient,
}

impl EvaluatorSupervisor {
    pub(super) fn provision_workspace(
        &self,
        request: WorkspaceRequest<'_>,
    ) -> SchedulerResult<ProvisionedWorkspace> {
        let WorkspaceRequest {
            assignment,
            access,
            execution_id,
            suffix,
            client,
        } = request;
        let directory = self.config.workspace_root.join(suffix);
        let guard = WorkspaceDirectory::create(directory.clone())?;
        let home = directory.join("home");
        std::fs::create_dir(&home)?;
        let adapter = client.adapter().map_err(internal)?;
        let skill_relative = adapter.skill_directory();
        if skill_relative.is_empty()
            || skill_relative.starts_with('/')
            || skill_relative.contains(['\\', ':'])
            || skill_relative
                .split('/')
                .any(|part| matches!(part, "" | "." | ".."))
        {
            return Err(SchedulerError::config_error(
                "Adapter skill path must be relative to its isolated home",
            ));
        }
        let skill_directory = home.join(skill_relative);
        materialize_skills(&assignment.skill_bundle.manifest, &skill_directory)?;
        let installed_skill_state = workspace_state(&skill_directory)?;
        materialize_root(&assignment.configuration.manifest, &home.join("work"))?;
        let relay_url = format!("http://eval-relay-{suffix}:8090");
        let context = super::super::adapters::AdapterContext {
            relay_url: &relay_url,
            execution_token: access.expose_token(),
            session_id: &access.session_id,
            execution_id,
            target: client
                .admitted_target(&self.config.client_image)
                .map_err(internal)?,
            frozen: assignment.spec.frozen.as_ref().ok_or_else(|| {
                SchedulerError::config_error("Frozen execution environment is missing")
            })?,
        };
        let environment = serde_json::json!({
            "native_target": context.target,
            "frozen": context.frozen,
            "configuration_digest": assignment.configuration.digest,
        });
        write_private(
            &directory.join("native-environment.json"),
            &serde_json::to_vec(&environment).map_err(internal)?,
        )?;
        let configuration = adapter.configuration(&context).map_err(internal)?;
        configuration.validate().map_err(internal)?;
        write_adapter_configuration(&directory, skill_relative, &configuration)?;
        Ok(ProvisionedWorkspace {
            directory,
            guard,
            home,
            installed_skill_state,
        })
    }

    pub(super) fn provision_network(
        &self,
        owner: &UserId,
        lease: &ExecutionLease,
        network_name: &str,
        relay_name: &str,
    ) -> SchedulerResult<ExecutionNetwork> {
        let mut network = ExecutionNetwork::create_fenced(
            self.config.docker.clone(),
            network_name.to_owned(),
            owner.as_str(),
            lease,
        )?;
        network.start_relay(
            &self.config.relay_image,
            &self.config.relay_control_network,
            &self.config.relay_upstream,
            relay_name.to_owned(),
        )?;
        Ok(network)
    }
}

fn write_adapter_configuration(
    directory: &std::path::Path,
    skill_relative: &str,
    configuration: &systemprompt_evaluation::experiments::execution::EvidenceArchive,
) -> SchedulerResult<()> {
    for (relative, file) in &configuration.files {
        if file.executable
            || (relative != "client.env" && !relative.starts_with("home/"))
            || relative == "home/work"
            || relative.starts_with("home/work/")
            || relative == &format!("home/{skill_relative}")
            || relative.starts_with(&format!("home/{skill_relative}/"))
        {
            return Err(SchedulerError::config_error(
                "Adapter configuration must not overwrite work or skills",
            ));
        }
        let path = directory.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        write_private(&path, &file.bytes)?;
        if std::fs::read(&path)? != file.bytes {
            return Err(SchedulerError::config_error(
                "Adapter configuration readback failed",
            ));
        }
    }
    if !configuration.files.contains_key("client.env") {
        return Err(SchedulerError::config_error(
            "Adapter must provide an isolated environment",
        ));
    }
    Ok(())
}

pub(super) fn variant_client(
    assignment: &ExecutionAssignment,
    variant_index: i32,
) -> SchedulerResult<(VariantSpec, NativeClient)> {
    let variant = assignment
        .spec
        .variants
        .get(usize::try_from(variant_index).map_err(internal)?)
        .cloned()
        .ok_or_else(|| SchedulerError::config_error("Assignment variant is unavailable"))?;
    let client = NativeClient::builder(variant.client, variant.model.clone())
        .pinned(
            variant.client_version.clone(),
            variant.worker_image_digest.clone(),
        )
        .limits(ExecutionLimits::default())
        .build()
        .map_err(internal)?;
    Ok((variant, client))
}

pub(super) const fn traffic_class(assignment: &ExecutionAssignment) -> EvaluationTrafficClass {
    match assignment.spec.execution_mode {
        systemprompt_evaluation::experiments::ExecutionMode::Fixture => {
            EvaluationTrafficClass::Fixture
        },
        systemprompt_evaluation::experiments::ExecutionMode::Live => {
            EvaluationTrafficClass::LiveEvaluation
        },
    }
}

pub(super) fn case_and_rubric(
    assignment: &ExecutionAssignment,
) -> SchedulerResult<(CaseContent, RubricContent)> {
    let ResourceContent::Case(case) = assignment.case.clone() else {
        return Err(SchedulerError::config_error("Assignment case type changed"));
    };
    let ResourceContent::Rubric(rubric) = assignment.rubric.clone() else {
        return Err(SchedulerError::config_error(
            "Assignment rubric type changed",
        ));
    };
    Ok((case, rubric))
}
