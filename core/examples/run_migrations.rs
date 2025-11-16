use rhythm_core::init::InitBuilder;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Running database migrations...");

    InitBuilder::new()
        .auto_migrate(true)
        .require_initialized(false)
        .init()
        .await?;

    println!("✓ Migrations completed successfully!");
    Ok(())
}
