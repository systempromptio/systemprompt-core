#[tokio::main]
async fn main() -> anyhow::Result<()> {
    systemprompt_cli::run().await
}
