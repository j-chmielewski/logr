use clap::Parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = logr::Args::parse();
    Ok(logr::run(args).await?)
}
