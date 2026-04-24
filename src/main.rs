use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use rmcp::{serve_server, transport::stdio};
use tracing::{info, warn};
use tracing_subscriber::{fmt, EnvFilter};

use mcp_server_git_rs::server::GitServer;

#[derive(Parser)]
#[command(
    name = "mcp-server-git-rs",
    about = "Fast Rust MCP server for git repository operations",
    version
)]
struct Cli {
    /// Restrict all operations to this repository (and its subdirectories).
    #[arg(short = 'r', long = "repository")]
    repository: Option<PathBuf>,

    /// Increase log verbosity. Repeat for more (-v info, -vv debug).
    #[arg(short = 'v', long = "verbose", action = clap::ArgAction::Count)]
    verbose: u8,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let level = match cli.verbose {
        0 => "warn",
        1 => "info",
        _ => "debug",
    };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));
    fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();

    if let Some(ref path) = cli.repository {
        match git2::Repository::open(path) {
            Ok(_) => info!("using repository at {}", path.display()),
            Err(e) => {
                warn!("{} is not a valid git repository: {e}", path.display());
                return Err(anyhow::anyhow!(
                    "invalid repository path: {}",
                    path.display()
                ));
            }
        }
    }

    let handler = GitServer::new(cli.repository);

    let running = serve_server(handler, stdio())
        .await
        .context("failed to start MCP server")?;
    running.waiting().await.context("server terminated")?;
    Ok(())
}
