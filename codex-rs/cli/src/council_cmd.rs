use clap::{Args, Subcommand};
use std::path::PathBuf;
use codex_council::{run_fix, run_review, CouncilConfig};
use anyhow::Result;

#[derive(Debug, Args)]
pub struct CouncilCli {
    #[clap(subcommand)]
    pub command: CouncilCommand,
}

#[derive(Debug, Subcommand)]
pub enum CouncilCommand {
    /// Review a file or path.
    Review {
        path: PathBuf,
        #[arg(long, default_value = "auto")]
        scope: String,
        #[arg(long)]
        json: bool,
    },
    /// Fix a file or path.
    Fix {
        path: PathBuf,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        redundant: bool,
        #[arg(long, default_value = "auto")]
        scope: String,
        #[arg(long)]
        full_tests: bool,
    },
    /// Apply a fix from a run.
    Apply {
        run_id: String,
        #[arg(long)]
        yes: bool,
    },
    /// Show status of a run.
    Status {
        run_id: String,
    },
    /// Show artifacts of a run.
    Show {
        run_id: String,
        #[arg(long)]
        plan: bool,
        #[arg(long)]
        patch: bool,
        #[arg(long)]
        verify: bool,
    },
}

pub async fn run(cli: CouncilCli) -> Result<()> {
    // Determine repo root. For now, assume current dir or find it.
    let repo_root = std::env::current_dir()?;
    let config = CouncilConfig { repo_root };

    match cli.command {
        CouncilCommand::Review { path, .. } => {
            run_review(config, path).await?;
        }
        CouncilCommand::Fix { path, .. } => {
            run_fix(config, path).await?;
        }
        _ => {
            println!("Command not implemented yet.");
        }
    }
    Ok(())
}
