/// Rhythm Global CLI
///
/// This binary provides administrative commands for Rhythm without requiring
/// a language runtime. It's useful for DevOps, debugging, and running baseline benchmarks.

use rhythm_core::cli;

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
