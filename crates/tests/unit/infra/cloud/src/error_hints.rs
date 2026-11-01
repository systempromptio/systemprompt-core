use systemprompt_cloud::CloudError;
use systemprompt_cloud::deploy::provider::ValuesFileError;
use systemprompt_identifiers::{Email, TenantId};
use systemprompt_manifest::HostingProvider;
use systemprompt_models::net::OutboundUrlError;

fn tenant() -> TenantId {
    TenantId::try_new("tenant-hints").unwrap()
}

#[test]
fn tenant_store_conflicts_name_their_own_remedies() {
    let exists = CloudError::TenantAlreadyExists {
        tenant_id: tenant(),
    };
    assert_eq!(exists.user_message(), "Tenant already exists");
    assert!(exists.recovery_hint().starts_with("Choose another id"));
    assert!(exists.to_string().contains("tenant-hints"));
    assert!(!exists.requires_login());

    let managed = CloudError::TenantNotSelfManaged {
        tenant_id: tenant(),
    };
    assert_eq!(
        managed.user_message(),
        "Tenant is managed by systemprompt.io"
    );
    assert!(managed.recovery_hint().contains("cloud tenant delete"));
}

#[test]
fn deploy_family_errors_map_to_deploy_messages() {
    let deploy = CloudError::deploy("push failed");
    assert_eq!(deploy.user_message(), "Cloud deploy failed");
    assert_eq!(
        deploy.recovery_hint(),
        "Inspect the error message and try again"
    );
    assert_eq!(deploy.to_string(), "push failed");

    let dockerfile = CloudError::dockerfile("no FROM");
    assert_eq!(dockerfile.user_message(), "Dockerfile validation failed");
    assert!(dockerfile.recovery_hint().contains("cloud profile create"));

    let docker = CloudError::docker_with("spawn", std::io::Error::other("gone"));
    assert_eq!(docker.user_message(), "Docker command failed");
    assert!(docker.recovery_hint().contains("Docker is installed"));
    assert!(std::error::Error::source(&docker).is_some());
    assert!(std::error::Error::source(&CloudError::docker("x")).is_none());
}

#[test]
fn provider_errors_explain_the_pipeline() {
    let not_deployable = CloudError::ProviderNotDeployable {
        provider: HostingProvider::DockerLocal,
    };
    assert_eq!(
        not_deployable.user_message(),
        "This provider has no deploy pipeline"
    );
    assert!(
        not_deployable
            .recovery_hint()
            .contains("infra services start")
    );

    let unconfigured = CloudError::DeployNotConfigured {
        provider: HostingProvider::Kubernetes,
    };
    assert_eq!(
        unconfigured.user_message(),
        "Deploy target is not configured"
    );
    assert!(unconfigured.recovery_hint().contains("cloud.deploy"));
}

#[test]
fn gitops_and_values_errors_point_at_the_ops_repository() {
    let gitops = CloudError::gitops_with("commit", std::io::Error::other("lock"));
    assert_eq!(gitops.user_message(), "GitOps update failed");
    let values = CloudError::ValuesFile {
        path: "values.yaml".to_owned(),
        source: ValuesFileError::EmptyPath,
    };
    assert_eq!(values.user_message(), "Values file could not be updated");
    assert_eq!(gitops.recovery_hint(), values.recovery_hint());
    assert!(values.recovery_hint().contains("ops repository"));
    assert_eq!(CloudError::gitops("x").to_string(), "x");
}

#[test]
fn an_unready_tenant_reports_the_wait() {
    let unready = CloudError::TenantUnready {
        url: "https://t.example".to_owned(),
        waited_secs: 90,
        last: "503".to_owned(),
    };
    assert_eq!(unready.user_message(), "Tenant did not become ready");
    assert!(unready.recovery_hint().contains("cloud status"));
    assert!(unready.to_string().contains("90s"));
}

#[test]
fn every_operator_error_shares_the_operator_login_hint() {
    let errors = [
        CloudError::operator_auth("denied"),
        CloudError::OperatorEndpoint {
            endpoint: "http://idp.example".to_owned(),
            source: OutboundUrlError::NonLoopbackHttp,
        },
        CloudError::OperatorEmail {
            source: Email::try_new("not-an-email").unwrap_err(),
        },
    ];
    for error in &errors {
        assert_eq!(error.user_message(), "Operator sign-in failed");
        assert!(error.recovery_hint().contains("--issuer"));
        assert!(!error.requires_login());
    }
    let signed_out = CloudError::OperatorNotAuthenticated;
    assert_eq!(signed_out.user_message(), "Not signed in as an operator");
    assert_eq!(signed_out.recovery_hint(), errors[0].recovery_hint());
}
