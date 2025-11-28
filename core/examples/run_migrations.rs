use rhythm_core::application::InitBuilder;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Running database migrations...");

    InitBuilder::new()
        .auto_migrate(true)
        .init()
        .await?;

    println!("✓ Migrations completed successfully!");
    Ok(())
}
