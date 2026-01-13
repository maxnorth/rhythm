//! Rhythm Language Server
//!
//! A Language Server Protocol implementation for the Rhythm workflow language.
//!
//! # Usage
//!
//! ```bash
//! rhythm-lsp --stdio
//! ```
//!
//! The server communicates over stdin/stdout using the LSP protocol.

use tower_lsp::{LspService, Server};
use tracing_subscriber::EnvFilter;

mod backend;
mod completions;
mod hover;
mod parser;
mod validation;

#[cfg(test)]
mod tests;

use backend::RhythmBackend;

#[tokio::main]
async fn main() {
    // Set up logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    let use_stdio = args.iter().any(|a| a == "--stdio");

    if !use_stdio {
        eprintln!("Rhythm Language Server");
        eprintln!("Usage: rhythm-lsp --stdio");
        eprintln!();
        eprintln!("Options:");
        eprintln!("  --stdio    Use stdin/stdout for communication (required)");
        std::process::exit(1);
    }

    // Create the LSP service
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(RhythmBackend::new);

    Server::new(stdin, stdout, socket).serve(service).await;
}
