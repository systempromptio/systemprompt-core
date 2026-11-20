//! Profile-backed test for `web sitemap show`: it resolves the content
//! config from the bootstrapped profile's paths and renders its sitemap
//! routes.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::{CommandContext, EnvOverrides};
use systemprompt_config::paths::AppPaths;
use systemprompt_content_entry::cli::sitemap::{self, SitemapCommands};
use systemprompt_extension::cli::{CliConfig, OutputFormat};
use systemprompt_test_fixtures::ensure_test_bootstrap;

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    cmd: SitemapCommands,
}

fn parse(args: &[&str]) -> SitemapCommands {
    Harness::try_parse_from(std::iter::once("sitemap").chain(args.iter().copied()))
        .unwrap()
        .cmd
}

#[test]
fn sitemap_show_reads_the_profile_content_config() {
    let boot = ensure_test_bootstrap();
    std::fs::write(
        boot.services_path.join("content/config.yaml"),
        "content_sources:\n  blog:\n    path: content/blog\n    source_id: blog\n    category_id: \
         blog\n    enabled: true\n    sitemap:\n      enabled: true\n      url_pattern: \
         /blog/{slug}\n      priority: 0.7\n      changefreq: weekly\n",
    )
    .unwrap();
    let cli = CliConfig::new()
        .with_interactive(false)
        .with_output_format(OutputFormat::Json);
    let ctx = CommandContext::new(cli.clone(), EnvOverrides::default())
        .with_boot(Some(boot.boot.clone()));

    let profile = ctx.profile().unwrap();
    let paths = AppPaths::from_profile(&profile.paths, profile.path_resolution(), None).unwrap();
    sitemap::execute(parse(&["show"]), &cli, &paths).unwrap();
}
