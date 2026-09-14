//! Claiming and exact assignment preparation for evaluator executions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::*;

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
        let assignment = self
            .repositories
            .assignments
            .get(&worker, &lease)
            .await
            .map_err(internal)?;
        let access = self
            .repositories
            .capabilities
            .issue(owner, &lease)
            .await
            .map_err(internal)?;
        let suffix = safe_suffix(&record.id);
        let directory = self.config.workspace_root.join(&suffix);
        std::fs::create_dir(&directory)?;
        let workspace_guard = WorkspaceDirectory(directory.clone());
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
        write_private(&directory.join("client.env"), format!("ANTHROPIC_BASE_URL=http://eval-relay-{suffix}:8090\nANTHROPIC_AUTH_TOKEN={}\nANTHROPIC_CUSTOM_HEADERS=x-session-id: {}\nSYSTEMPROMPT_EXECUTION_ID={}\n", access.expose_token(), access.session_id, record.id).as_bytes())?;
        self.append_event(
            &worker,
            &lease,
            0,
            ExecutionStage::Provisioning,
            "Installed exact managed assignment bundles",
        )
        .await?;
        let network_name = format!("eval-net-{suffix}");
        let relay_name = format!("eval-relay-{suffix}");
        let client_name = format!("eval-client-{suffix}");
        let mut network = ExecutionNetwork::create(
            self.config.docker.clone(),
            network_name.clone(),
            owner.as_str(),
            record.id.as_str(),
        )?;
        network.start_relay(
            &self.config.relay_image,
            &self.config.relay_control_network,
            &self.config.relay_upstream,
            relay_name.clone(),
        )?;
        let variant = assignment
            .spec
            .variants
            .get(usize::try_from(record.variant_index).map_err(internal)?)
            .cloned()
            .ok_or_else(|| SchedulerError::config_error("Assignment variant is unavailable"))?;
        let client = NativeClient::builder(variant.client, variant.model.clone())
            .limits(ExecutionLimits::default())
            .build()
            .map_err(internal)?;
        let launch = ContainerLaunch::builder(self.config.docker.clone(), directory.clone())
            .image(self.config.client_image.clone())
            .network(network.name().to_owned())
            .name(client_name.clone())
            .ownership(owner.as_str(), record.id.as_str())
            .build()?;
        let case = match assignment.case.clone() {
            ResourceContent::Case(case) => case,
            _ => return Err(SchedulerError::config_error("Assignment case type changed")),
        };
        let rubric = match assignment.rubric.clone() {
            ResourceContent::Rubric(rubric) => rubric,
            _ => {
                return Err(SchedulerError::config_error(
                    "Assignment rubric type changed",
                ));
            },
        };
        install_case_fixtures(&case, &home.join("work"))?;
        let baseline = workspace_state(&home.join("work"))?;
        let traffic = match assignment.spec.execution_mode {
            systemprompt_evaluation::experiments::ExecutionMode::Fixture => {
                EvaluationTrafficClass::Fixture
            },
            systemprompt_evaluation::experiments::ExecutionMode::Live => {
                EvaluationTrafficClass::LiveEvaluation
            },
        };
        self.repositories
            .gateway
            .set_traffic_class(owner, &lease, traffic)
            .await
            .map_err(internal)?;
        Ok(Some(PreparedExecution {
            worker,
            record,
            lease,
            assignment,
            directory,
            _workspace_guard: workspace_guard,
            home,
            installed_skill_state,
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
}
