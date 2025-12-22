use anyhow::Result;
use clap::{Parser, Subcommand};
use sqlx::postgres::PgPoolOptions;

#[derive(Parser)]
#[command(name = "rhythm")]
#[command(about = "Rhythm workflow engine CLI", long_about = None)]
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
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Migrate => {
            migrate().await?;
        }
    }

    Ok(())
}

async fn migrate() -> Result<()> {
    let database_url = std::env::var("RHYTHM_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .expect("RHYTHM_DATABASE_URL or DATABASE_URL must be set");

    println!("Running migrations against: {}", database_url);

    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    println!("Migrations completed successfully");

    Ok(())
}
