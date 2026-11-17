//! The smallest install member: one schema, one route set, one job and one
//! CLI group, composed onto the reference composition.
//!
//! `DemoExtension::contribute` only declares; the composition admits each
//! contribution against the profile before anything it declares runs.
//!
//! Run with: `cargo run -p systemprompt --example extension --features full --
//! demo status`

use std::process::ExitCode;

use async_trait::async_trait;
use axum::http::Method;
use systemprompt::extension::RouteGroup;
use systemprompt::extension::cli::{
    CliCommandGroup, CommandDecl, CommandDescriptor, CommandError, CommandHost, CommandOutput,
    DescribeCommand,
};
use systemprompt::extension::prelude::*;
use systemprompt::extension::router::{
    ApiBase, BasePath, BodyLimit, RouteLimits, RouteSet, RouteSpec,
};
use systemprompt::traits::{Job, JobContext, JobResult, ProviderResult};
use systemprompt::{RunError, reference};

const LIMITS: RouteLimits = RouteLimits {
    rate: RateLimit::new("demo", RateBudget::Contexts),
    body: BodyLimit::Default,
};
const USER: RouteSpec = RouteSpec::new(AuthzPolicy::user(), ContextKind::User, LIMITS);

#[derive(Debug, Clone, Copy, Default)]
struct DemoExtension;

impl Extension for DemoExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "demo-extension",
            name: "Demo Extension",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        c.schema(SchemaDefinition::new(
            "demo_extension_state",
            "CREATE TABLE IF NOT EXISTS demo_extension_state (id TEXT PRIMARY KEY)",
        ))
        .routes(
            RouteSet::at::<DemoState>(
                BasePath::Api(ApiBase::Extension("/api/demo")),
                RouteGroup::Extensions,
            )
            .route(Method::GET, "/status", status, USER)
            .build(),
        )
        .job(DemoJob)
        .command(CommandDecl::group::<DemoCommands>());
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct DemoState;

impl FromState for DemoState {
    fn from_state(_r: &Resolved<'_>) -> Result<Self, MissingDependency> {
        Ok(Self)
    }
}

async fn status() -> &'static str {
    "ok"
}

#[derive(Debug, Clone, Copy)]
struct DemoJob;

#[async_trait]
impl Job for DemoJob {
    fn name(&self) -> &'static str {
        "demo_heartbeat"
    }

    fn schedule(&self) -> &'static str {
        "0 0 * * * *"
    }

    async fn execute(&self, _ctx: &JobContext) -> ProviderResult<JobResult> {
        Ok(JobResult::success())
    }
}

#[derive(Debug, Clone, clap::Subcommand)]
enum DemoCommands {
    Status,
}

impl DescribeCommand for DemoCommands {
    fn descriptor(&self) -> CommandDescriptor {
        CommandDescriptor::NONE
    }
}

impl CliCommandGroup for DemoCommands {
    const NAME: &'static str = "demo";
    const ABOUT: &'static str = "Demo extension commands";

    fn run(
        self,
        _host: &dyn CommandHost,
    ) -> impl Future<Output = Result<CommandOutput, CommandError>> + Send {
        std::future::ready(match self {
            Self::Status => Ok(CommandOutput::text("demo extension is installed")),
        })
    }
}

fn main() -> Result<ExitCode, RunError> {
    reference().with(DemoExtension).run()
}
