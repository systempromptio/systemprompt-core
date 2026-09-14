//! Claiming and exact assignment preparation for evaluator executions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{
    BTreeMap, CaseContent, ContainerLaunch, EvalExecutionId, EvalWorkerId, EvaluationTrafficClass,
    EvaluatorSupervisor, ExecutionAssignment, ExecutionLease, ExecutionLimits, ExecutionNetwork,
    ExecutionRecord, ExecutionStage, NativeClient, PathBuf, ResourceContent, RubricContent,
    SchedulerError, SchedulerResult, StageEvent, UserId, VariantSpec, WorkerRecord,
    WorkspaceDirectory, install_case_fixtures, internal, materialize_root, materialize_skills,
    safe_suffix, workspace_state, write_private,
};
use systemprompt_evaluation::repository::experiments::ExecutionAccess;

struct ProvisionedWorkspace {
    directory: PathBuf,
    guard: WorkspaceDirectory,
    home: PathBuf,
    installed_skill_state: BTreeMap<String, String>,
}

pub(super) struct PreparedExecution {
    pub worker: WorkerRecord,
    pub record: ExecutionRecord,
    pub lease: ExecutionLease,
    pub assignment: ExecutionAssignment,
    pub directory: PathBuf,
    pub _workspace_guard: WorkspaceDirectory,
    pub home: PathBuf,
    pub installed_skill_state: BTreeMap<String, String>,
    pub network: ExecutionNetwork,
    pub network_name: String,
    pub relay_name: String,
    pub client_name: String,
    pub variant: VariantSpec,
    pub client: NativeClient,
    pub launch: ContainerLaunch,
    pub case: CaseContent,
    pub rubric: RubricContent,
    pub baseline: BTreeMap<String, String>,
}

impl EvaluatorSupervisor {
    pub(super) async fn prepare_execution(
        &self,
        owner: &UserId,
        worker_id: &EvalWorkerId,
    ) -> SchedulerResult<Option<PreparedExecution>> {
        let worker = self
            .repositories
            .workers
            .get_owned(owner, worker_id, &self.config.environment)
            .await
            .map_err(internal)?;
        self.repositories
            .lifecycle
            .reconcile_restart(owner)
            .await
            .map_err(internal)?;
        self.reconcile_owned_docker(owner).await?;
        let Some((record, lease)) = self.claim_lease(owner, worker_id).await? else {
            return Ok(None);
        };
        let (assignment, access) = self.assignment_and_access(owner, &worker, &lease).await?;
        let suffix = safe_suffix(&record.id);
        let workspace = self.provision_workspace(&assignment, &access, &record.id, &suffix)?;
        self.heartbeat(owner, &lease).await?;
        self.append_event(
            &worker,
            &lease,
            StageEvent {
                sequence: 0,
                stage: ExecutionStage::Provisioning,
                summary: "Installed exact managed assignment bundles",
            },
        )
        .await?;
        let network_name = format!("eval-net-{suffix}");
        let relay_name = format!("eval-relay-{suffix}");
        let client_name = format!("eval-client-{suffix}");
        let network = self.provision_network(owner, &record.id, &network_name, &relay_name)?;
        self.heartbeat(owner, &lease).await?;
        let (variant, client) = variant_client(&assignment, record.variant_index)?;
        let launch =
            ContainerLaunch::builder(self.config.docker.clone(), workspace.directory.clone())
                .image(self.config.client_image.clone())
                .network(network.name().to_owned())
                .name(client_name.clone())
                .ownership(owner.as_str(), record.id.as_str())
                .build()?;
        let (case, rubric) = case_and_rubric(&assignment)?;
        install_case_fixtures(&case, &workspace.home.join("work"))?;
        let baseline = workspace_state(&workspace.home.join("work"))?;
        self.repositories
            .gateway
            .set_traffic_class(owner, &lease, traffic_class(&assignment))
            .await
            .map_err(internal)?;
        Ok(Some(PreparedExecution {
            worker,
            record,
            lease,
            assignment,
            directory: workspace.directory,
            _workspace_guard: workspace.guard,
            home: workspace.home,
            installed_skill_state: workspace.installed_skill_state,
            network,
            network_name,
            relay_name,
            client_name,
            variant,
            client,
            launch,
            case,
            rubric,
            baseline,
        }))
    }

    async fn claim_lease(
        &self,
        owner: &UserId,
        worker_id: &EvalWorkerId,
    ) -> SchedulerResult<Option<(ExecutionRecord, ExecutionLease)>> {
        let Some(record) = self
            .repositories
            .experiments
            .claim(owner, worker_id)
            .await
            .map_err(internal)?
        else {
            return Ok(None);
        };
        let lease = ExecutionLease::builder(record.id.clone(), worker_id.clone())
            .fencing_token(record.fencing_token)
            .build()
            .map_err(internal)?;
        Ok(Some((record, lease)))
    }

    async fn assignment_and_access(
        &self,
        owner: &UserId,
        worker: &WorkerRecord,
        lease: &ExecutionLease,
    ) -> SchedulerResult<(ExecutionAssignment, ExecutionAccess)> {
        let assignment = self
            .repositories
            .assignments
            .get(worker, lease)
            .await
            .map_err(internal)?;
        let access = self
            .repositories
            .capabilities
            .issue(owner, lease)
            .await
            .map_err(internal)?;
        Ok((assignment, access))
    }

    async fn heartbeat(&self, owner: &UserId, lease: &ExecutionLease) -> SchedulerResult<()> {
        self.repositories
            .experiments
            .heartbeat(owner, lease)
            .await
            .map_err(internal)
    }

    fn provision_workspace(
        &self,
        assignment: &ExecutionAssignment,
        access: &ExecutionAccess,
        execution_id: &EvalExecutionId,
        suffix: &str,
    ) -> SchedulerResult<ProvisionedWorkspace> {
        let directory = self.config.workspace_root.join(suffix);
        std::fs::create_dir(&directory)?;
        let guard = WorkspaceDirectory(directory.clone());
        let home = directory.join("home");
        std::fs::create_dir(&home)?;
        materialize_skills(
            &assignment.skill_bundle.manifest,
            &home.join(".claude/skills"),
        )?;
        let installed_skill_state = workspace_state(&home.join(".claude/skills"))?;
        materialize_root(&assignment.configuration.manifest, &home.join("work"))?;
        let mcp = serde_json::json!({"mcpServers":{"evaluation_fixture":{"type":"http","url":format!("http://eval-relay-{suffix}:8090/mcp/evaluation_fixture"),"headers":{"Authorization":format!("Bearer {}", access.expose_token()),"x-session-id":access.session_id.as_str()}}}});
        write_private(
            &home.join(".mcp.json"),
            &serde_json::to_vec(&mcp).map_err(internal)?,
        )?;
        write_private(&directory.join("client.env"), format!("ANTHROPIC_BASE_URL=http://eval-relay-{suffix}:8090\nANTHROPIC_AUTH_TOKEN={}\nANTHROPIC_CUSTOM_HEADERS=x-session-id: {}\nSYSTEMPROMPT_EXECUTION_ID={}\n", access.expose_token(), access.session_id, execution_id).as_bytes())?;
        Ok(ProvisionedWorkspace {
            directory,
            guard,
            home,
            installed_skill_state,
        })
    }

    fn provision_network(
        &self,
        owner: &UserId,
        execution_id: &EvalExecutionId,
        network_name: &str,
        relay_name: &str,
    ) -> SchedulerResult<ExecutionNetwork> {
        let mut network = ExecutionNetwork::create(
            self.config.docker.clone(),
            network_name.to_owned(),
            owner.as_str(),
            execution_id.as_str(),
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

fn variant_client(
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
        .limits(ExecutionLimits::default())
        .build()
        .map_err(internal)?;
    Ok((variant, client))
}

const fn traffic_class(assignment: &ExecutionAssignment) -> EvaluationTrafficClass {
    match assignment.spec.execution_mode {
        systemprompt_evaluation::experiments::ExecutionMode::Fixture => {
            EvaluationTrafficClass::Fixture
        },
        systemprompt_evaluation::experiments::ExecutionMode::Live => {
            EvaluationTrafficClass::LiveEvaluation
        },
    }
}

fn case_and_rubric(
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
