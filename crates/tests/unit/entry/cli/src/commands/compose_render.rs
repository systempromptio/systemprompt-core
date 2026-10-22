//! One PostgreSQL compose renderer serves both the local setup database and
//! a tenant container: the host port is the only knob, and the container
//! port is always the default PostgreSQL port.

use systemprompt_cli::commands::admin::setup::docker_compose::{
    ComposeCredentials, PostgresCompose, postgres_compose,
};
use systemprompt_models::defaults::DEFAULT_POSTGRES_PORT;

#[test]
fn the_local_setup_shape_interpolates_credentials_and_names_the_container() {
    let rendered = postgres_compose(&PostgresCompose {
        host_port: 5544,
        credentials: ComposeCredentials::FromEnvironment,
        container_name: Some("cov_pg"),
        init_scripts: true,
        restart: None,
        header: None,
    });

    assert!(rendered.starts_with("services:\n  postgres:\n    image: postgres:18-alpine\n"));
    assert!(rendered.contains("    container_name: cov_pg\n"));
    assert!(rendered.contains("POSTGRES_USER: ${POSTGRES_USER:-systemprompt}"));
    assert!(rendered.contains("POSTGRES_PASSWORD: ${POSTGRES_PASSWORD}"));
    assert!(rendered.contains(&format!("      - \"5544:{DEFAULT_POSTGRES_PORT}\"\n")));
    assert!(rendered.contains("      - ./init-scripts:/docker-entrypoint-initdb.d\n"));
    assert!(rendered.contains("pg_isready -U ${POSTGRES_USER:-systemprompt}"));
    assert!(rendered.contains("networks:\n  cov_pg_network:\n    driver: bridge\n"));
    assert!(!rendered.contains("restart:"));
}

#[test]
fn the_tenant_shape_uses_literal_credentials_and_a_restart_policy() {
    let rendered = postgres_compose(&PostgresCompose {
        host_port: 6543,
        credentials: ComposeCredentials::Literal {
            user: "tenant",
            password: "s3cret",
            database: "tenant_db",
        },
        container_name: None,
        init_scripts: false,
        restart: Some("unless-stopped"),
        header: Some("# header line\n"),
    });

    assert!(rendered.starts_with("# header line\n\nservices:\n"));
    assert!(rendered.contains("    restart: unless-stopped\n"));
    assert!(rendered.contains("POSTGRES_USER: tenant\n"));
    assert!(rendered.contains("POSTGRES_PASSWORD: s3cret\n"));
    assert!(rendered.contains("POSTGRES_DB: tenant_db\n"));
    assert!(rendered.contains(&format!("      - \"6543:{DEFAULT_POSTGRES_PORT}\"\n")));
    assert!(rendered.contains("pg_isready -U tenant -d tenant_db"));
    assert!(rendered.contains("volumes:\n  postgres_data: {}\n"));
    assert!(!rendered.contains("container_name"));
    assert!(!rendered.contains("init-scripts"));
    assert!(!rendered.contains("networks"));
}

#[test]
fn the_rendered_file_is_valid_yaml_in_both_shapes() {
    for opts in [
        PostgresCompose {
            host_port: 5432,
            credentials: ComposeCredentials::FromEnvironment,
            container_name: Some("local"),
            init_scripts: true,
            restart: None,
            header: None,
        },
        PostgresCompose {
            host_port: 15432,
            credentials: ComposeCredentials::Literal {
                user: "u",
                password: "p",
                database: "d",
            },
            container_name: None,
            init_scripts: false,
            restart: Some("always"),
            header: Some("# generated\n"),
        },
    ] {
        let doc: serde_yaml::Value =
            serde_yaml::from_str(&postgres_compose(&opts)).expect("compose renders as YAML");
        assert!(doc["services"]["postgres"]["ports"].is_sequence());
    }
}
