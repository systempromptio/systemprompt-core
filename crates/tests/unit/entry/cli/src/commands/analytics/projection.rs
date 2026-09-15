use clap::Parser;
use systemprompt_cli::analytics::AnalyticsCommands;
use systemprompt_cli::analytics::projection::ProjectionCommands;

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    command: AnalyticsCommands,
}

#[test]
fn projection_commands_parse_status_rebuild_and_bounded_sync() {
    for (arguments, expected) in [
        (vec!["analytics", "projection", "status"], "status"),
        (vec!["analytics", "projection", "rebuild"], "rebuild"),
        (vec!["analytics", "projection", "sync"], "sync-default"),
        (
            vec!["analytics", "projection", "sync", "--limit", "17"],
            "sync-custom",
        ),
    ] {
        let parsed = Harness::try_parse_from(arguments).unwrap();
        match (parsed.command, expected) {
            (AnalyticsCommands::Projection(ProjectionCommands::Status), "status")
            | (AnalyticsCommands::Projection(ProjectionCommands::Rebuild), "rebuild") => {},
            (AnalyticsCommands::Projection(ProjectionCommands::Sync { limit }), "sync-default") => {
                assert_eq!(limit, 10_000)
            },
            (AnalyticsCommands::Projection(ProjectionCommands::Sync { limit }), "sync-custom") => {
                assert_eq!(limit, 17)
            },
            (unexpected, expected) => panic!("expected {expected}, got {unexpected:?}"),
        }
    }
    for invalid in ["-1", "not-a-number"] {
        assert!(
            Harness::try_parse_from(["analytics", "projection", "sync", "--limit", invalid])
                .is_err()
        );
    }
}
