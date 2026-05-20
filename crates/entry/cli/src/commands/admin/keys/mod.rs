mod generate;

use anyhow::Result;
use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum KeysCommands {
    #[command(about = "Generate a fresh RSA-2048 signing keypair")]
    Generate(generate::GenerateArgs),
}

pub fn execute(cmd: KeysCommands) -> Result<()> {
    match cmd {
        KeysCommands::Generate(args) => generate::execute(args),
    }
}
