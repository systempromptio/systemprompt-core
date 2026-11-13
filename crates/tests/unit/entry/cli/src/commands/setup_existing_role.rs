//! Setup against a role that already exists: it is never changed without
//! `--reset-role-password`, a password that does not log in stops setup, and
//! the reset flag is an irreversible, confirmed command.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::CliConfig;
use systemprompt_cli::admin::setup::postgres::setup_interactive;
use systemprompt_cli::shared::postgres_access::test_connection;
use systemprompt_cli::args::{Cli, Commands};
use systemprompt_cli::descriptor::{CommandDescriptor, DataImpact, DescribeCommand};
use systemprompt_loader::subprocess::ForegroundChildren;

use super::setup_postgres::{
    DbUrl, admin_pool, args, db_url, drop_scratch, runtime, scratch_name, scripted,
};

struct Scratch {
    db: DbUrl,
    role: String,
    database: String,
}

fn scratch_role(password: &str) -> Scratch {
    let db = db_url();
    let role = scratch_name("c1_role");
    let database = scratch_name("c1_db");
    runtime().block_on(async {
        let pool = admin_pool(&db).await;
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "CREATE ROLE \"{role}\" LOGIN PASSWORD '{password}'"
        )))
        .execute(&pool)
        .await
        .expect("create the pre-existing role");
        pool.close().await;
    });
    Scratch { db, role, database }
}

impl Scratch {
    fn run(
        &self,
        password: &str,
        reset: bool,
    ) -> anyhow::Result<systemprompt_cli::shared::postgres_access::PostgresConfig> {
        let mut setup_args = args();
        setup_args.db_password = Some(password.to_owned());
        setup_args.reset_role_password = reset;
        let port = self.db.port.to_string();
        let prompter = scripted(&[
            "0",
            self.db.host.as_str(),
            &port,
            &self.role,
            &self.database,
            "y",
            self.db.user.as_str(),
            self.db.password.as_str(),
        ]);
        runtime().block_on(setup_interactive(
            &setup_args,
            &prompter,
            "covtest",
            &CliConfig::default(),
            &ForegroundChildren::default(),
        ))
    }

    fn logs_in_with(&self, password: &str) -> bool {
        let options = sqlx::postgres::PgConnectOptions::new()
            .host(&self.db.host)
            .port(self.db.port)
            .username(&self.role)
            .password(password)
            .database("postgres");
        runtime().block_on(async {
            match sqlx::PgPool::connect_with(options).await {
                Ok(pool) => {
                    pool.close().await;
                    true
                },
                Err(_) => false,
            }
        })
    }

    fn drop(&self) {
        runtime().block_on(drop_scratch(&self.db, &self.role, &self.database));
    }
}

#[test]
fn a_wrong_password_for_an_existing_role_stops_and_leaves_the_role_alone() {
    let scratch = scratch_role("the_real_password");
    let outcome = scratch.run("mistyped", false);
    let old_still_works = scratch.logs_in_with("the_real_password");
    let mistyped_works = scratch.logs_in_with("mistyped");
    scratch.drop();

    let error = outcome.expect_err("a password that does not log in must stop setup");
    let message = format!("{error:#}");
    assert!(message.contains(&scratch.role), "{message}");
    assert!(message.contains("--reset-role-password"), "{message}");
    assert!(old_still_works, "the role's own password must be untouched");
    assert!(!mistyped_works);
}

#[test]
fn an_existing_role_with_its_own_password_is_verified_and_its_database_created() {
    let scratch = scratch_role("the_real_password");
    let outcome = scratch.run("the_real_password", false);
    let connects = outcome
        .as_ref()
        .is_ok_and(|config| runtime().block_on(test_connection(config)));
    scratch.drop();

    outcome.expect("the right password provisions the database");
    assert!(connects);
}

#[test]
fn the_reset_flag_changes_an_existing_roles_password() {
    let scratch = scratch_role("the_old_password");
    let outcome = scratch.run("new p@ss/w#rd", true);
    let new_works = scratch.logs_in_with("new p@ss/w#rd");
    let old_works = scratch.logs_in_with("the_old_password");
    scratch.drop();

    outcome.expect("an explicit reset provisions with the new password");
    assert!(new_works);
    assert!(!old_works);
}

#[test]
fn the_reset_flag_is_irreversible_and_plain_setup_is_not() {
    let describe = |args: &[&str]| {
        let cli = Cli::try_parse_from(std::iter::once("systemprompt").chain(args.iter().copied()))
            .unwrap();
        cli.command
            .as_ref()
            .map_or(CommandDescriptor::FULL, Commands::descriptor)
            .data_impact()
    };
    assert_eq!(
        describe(&["admin", "setup", "--reset-role-password"]),
        DataImpact::Irreversible
    );
    assert_eq!(describe(&["admin", "setup"]), DataImpact::Preserving);
}
