//! Identifier value parsers used as clap `value_parser`s.
//!
//! A malformed identifier argument must be a usage error raised while the
//! arguments are parsed, never a value that reaches a command body.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use clap::error::ErrorKind;
use systemprompt_cli::shared::{
    parse_agent_name, parse_ai_request_id, parse_campaign_id, parse_category_id, parse_job_name,
    parse_link_click_id, parse_link_id, parse_mcp_server_id, parse_mcp_tool_name, parse_plugin_id,
    parse_service_name, parse_tenant_id, parse_trace_id,
};
use systemprompt_identifiers::{JobName, LinkId};

#[derive(Debug, Parser)]
struct Harness {
    #[arg(value_parser = parse_link_id)]
    link_id: LinkId,
    #[arg(long, value_parser = parse_job_name)]
    job: Option<JobName>,
}

#[test]
fn every_id_parser_rejects_an_empty_value() {
    assert!(parse_agent_name("").is_err());
    assert!(parse_ai_request_id("").is_err());
    assert!(parse_campaign_id("").is_err());
    assert!(parse_category_id("").is_err());
    assert!(parse_job_name("").is_err());
    assert!(parse_link_click_id("").is_err());
    assert!(parse_link_id("").is_err());
    assert!(parse_mcp_server_id("").is_err());
    assert!(parse_mcp_tool_name("").is_err());
    assert!(parse_plugin_id("").is_err());
    assert!(parse_service_name("").is_err());
    assert!(parse_tenant_id("").is_err());
    assert!(parse_trace_id("").is_err());
}

#[test]
fn every_id_parser_rejects_a_blank_value() {
    assert!(parse_agent_name("   ").is_err());
    assert!(parse_job_name("   ").is_err());
    assert!(parse_link_id("   ").is_err());
    assert!(parse_mcp_server_id("   ").is_err());
    assert!(parse_mcp_tool_name("   ").is_err());
    assert!(parse_service_name("   ").is_err());
    assert!(parse_tenant_id("   ").is_err());
}

#[test]
fn agent_name_parser_rejects_sentinels() {
    assert!(parse_agent_name("unknown").is_err());
    assert!(parse_agent_name("unset").is_err());
}

#[test]
fn id_parsers_keep_a_well_formed_value_verbatim() {
    assert_eq!(parse_link_id("lnk_123").expect("valid").as_str(), "lnk_123");
    assert_eq!(
        parse_job_name("daily-cleanup").expect("valid").as_str(),
        "daily-cleanup"
    );
    assert_eq!(
        parse_agent_name("edward").expect("valid").as_str(),
        "edward"
    );
    assert_eq!(
        parse_mcp_server_id("systemprompt").expect("valid").as_str(),
        "systemprompt"
    );
    assert_eq!(
        parse_service_name("my-agent").expect("valid").as_str(),
        "my-agent"
    );
}

#[test]
fn malformed_id_argument_is_a_usage_error() {
    let error = Harness::try_parse_from(["cli", ""]).expect_err("empty id must not parse");
    assert_eq!(error.kind(), ErrorKind::ValueValidation);

    let error = Harness::try_parse_from(["cli", "lnk_1", "--job", " "])
        .expect_err("blank job name must not parse");
    assert_eq!(error.kind(), ErrorKind::ValueValidation);
}

#[test]
fn well_formed_id_arguments_parse_into_typed_ids() {
    let harness =
        Harness::try_parse_from(["cli", "lnk_1", "--job", "nightly"]).expect("valid arguments");
    assert_eq!(harness.link_id.as_str(), "lnk_1");
    assert_eq!(harness.job.expect("job set").as_str(), "nightly");
}
