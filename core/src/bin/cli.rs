use clap::{Parser, Subcommand};
use workflows_core::db;

#[derive(Parser)]
#[command(name = "workflows")]
#[command(about = "Workflows CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run database migrations
    Migrate,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Migrate => {
            println!("Running migrations...");
            db::migrate().await?;
            println!("Migrations complete!");
        }
    }

    Ok(())
}
