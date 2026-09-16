//! Claiming and exact assignment preparation for evaluator executions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::provision::{WorkspaceRequest, case_and_rubric, traffic_class, variant_client};
use super::{
    BTreeMap, CaseContent, ContainerLaunch, EvalWorkerId, EvaluatorSupervisor, ExecutionAssignment,
    ExecutionLease, ExecutionNetwork, ExecutionRecord, ExecutionStage, NativeClient, PathBuf,
    RubricContent, SchedulerResult, StageEvent, UserId, VariantSpec, WorkerRecord,
    WorkspaceDirectory, install_case_fixtures, internal, safe_suffix, workspace_state,
};
use systemprompt_evaluation::repository::experiments::ExecutionAccess;

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
    pub skill_directory: PathBuf,
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
        let variant_index = record.variant_index;
        match self
            .prepare_claimed(owner, worker, record, lease.clone())
            .await
        {
            Ok(run) => Ok(Some(run)),
            Err(error) => {
                self.block_execution(
                    owner,
                    &lease,
                    variant_index,
                    &super::failures::diagnostic("preparation", &error),
                )
                .await?;
                Err(error)
            },
        }
    }

    async fn prepare_claimed(
        &self,
        owner: &UserId,
        worker: WorkerRecord,
        record: ExecutionRecord,
        lease: ExecutionLease,
    ) -> SchedulerResult<PreparedExecution> {
        let (assignment, access) = self.assignment_and_access(owner, &worker, &lease).await?;
        systemprompt_evaluation::capabilities::admit_experiment(&assignment.spec)
            .map_err(internal)?;
        let (variant, client) = variant_client(&assignment, record.variant_index)?;
        client
            .admitted_target(&self.config.client_image)
            .map_err(internal)?;
        let suffix = format!("{}-f{}", safe_suffix(&record.id), lease.fencing_token);
        let workspace = self.provision_workspace(WorkspaceRequest {
            assignment: &assignment,
            access: &access,
            execution_id: &record.id,
            suffix: &suffix,
            client: &client,
        })?;
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
        let network = self.provision_network(owner, &lease, &network_name, &relay_name)?;
        self.heartbeat(owner, &lease).await?;
        let launch =
            ContainerLaunch::builder(self.config.docker.clone(), workspace.directory.clone())
                .image(self.config.client_image.clone())
                .network(network.name().to_owned())
                .name(client_name.clone())
                .ownership(owner.as_str(), record.id.as_str())
                .lease(&lease)
                .build()?;
        let (case, rubric) = case_and_rubric(&assignment)?;
        install_case_fixtures(&case, &workspace.home.join("work"))?;
        let baseline = workspace_state(&workspace.home.join("work"))?;
        self.repositories
            .gateway
            .set_traffic_class(owner, &lease, traffic_class(&assignment))
            .await
            .map_err(internal)?;
        let skill_directory = workspace
            .home
            .join(client.adapter().map_err(internal)?.skill_directory());
        Ok(PreparedExecution {
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
            skill_directory,
            client,
            launch,
            case,
            rubric,
            baseline,
        })
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
}
