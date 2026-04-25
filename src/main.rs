use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use rmcp::{serve_server, transport::stdio};
use tracing::{info, warn};
use tracing_subscriber::{fmt, EnvFilter};

use mcp_server_git_rs::features::FeatureSet;
use mcp_server_git_rs::server::GitServer;

#[derive(Parser)]
#[command(
    name = "mcp-server-git-rs",
    about = "Fast Rust MCP server for git repository operations",
    version
)]
struct Cli {
    /// Restrict operations to one or more repositories (and their
    /// subdirectories and worktrees). Pass `-r` multiple times to allow
    /// several repos. Without this flag, the server is unscoped.
    #[arg(short = 'r', long = "repository", action = clap::ArgAction::Append)]
    repository: Vec<PathBuf>,

    /// Enable additional tool groups (comma-separated). Without this flag, only
    /// the core tools are exposed. Valid groups: inspection, tags, stash,
    /// remotes, history, branches-extended, worktrees, notes, all.
    #[arg(long = "features", value_delimiter = ',')]
    features: Vec<String>,

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

    let features = FeatureSet::from_cli(&cli.features).context("invalid --features value")?;

    for path in &cli.repository {
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

    let handler = GitServer::new(cli.repository, features);

    let running = serve_server(handler, stdio())
        .await
        .context("failed to start MCP server")?;
    running.waiting().await.context("server terminated")?;
    Ok(())
}
