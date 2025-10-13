/// Currant Global CLI
///
/// This binary provides administrative commands for Currant without requiring
/// a language runtime. It's useful for DevOps, debugging, and running baseline benchmarks.

use currant_core::cli;

#[tokio::main]
async fn main() {
    if let Err(e) = cli::run_cli().await {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
