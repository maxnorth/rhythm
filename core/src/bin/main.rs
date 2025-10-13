/// Currant Global CLI
///
/// This binary provides administrative commands for Currant without requiring
/// a language runtime. It's useful for DevOps, debugging, and running baseline benchmarks.

use currant_core::cli;

#[tokio::main]
async fn main() {
    if let Err(e) = cli::run_cli().await {
        eprintln!("Error: {}", e);

        // Print error chain for better debugging
        let mut source = e.source();
        while let Some(err) = source {
            eprintln!("\nCaused by: {}", err);
            source = err.source();
        }

        std::process::exit(1);
    }
}
