use anyhow::Result;
use clap::Args;
use clap::Subcommand;
use codex_council::CouncilConfig;
use codex_council::run_fix;
use codex_council::run_review;
use std::path::PathBuf;

#[derive(Debug, Args)]
pub struct CouncilCli {
    #[clap(subcommand)]
    pub command: CouncilCommand,
}

#[derive(Debug, Args)]
pub struct FixArgs {
    pub path: PathBuf,
    #[arg(long)]
    pub yes: bool,
    #[arg(long)]
    pub redundant: bool,
    #[arg(long, default_value = "auto")]
    pub scope: String,
    #[arg(long)]
    pub full_tests: bool,
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
    Fix(FixArgs),
    /// Apply a fix from a run.
    Apply {
        run_id: String,
        #[arg(long)]
        yes: bool,
    },
    /// Show status of a run.
    Status { run_id: String },
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

use codex_core::config::ConfigBuilder;

fn init_logging() {
    let default_level = "info";
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .or_else(|_| tracing_subscriber::EnvFilter::try_new(default_level))
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default_level)),
        )
        .with_writer(std::io::stderr)
        .try_init();
}

fn find_git_root() -> Result<PathBuf> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()?;
    if output.status.success() {
        let path = String::from_utf8(output.stdout)?.trim().to_string();
        Ok(PathBuf::from(path))
    } else {
        Ok(std::env::current_dir()?)
    }
}

pub async fn run_review_for_path(path: PathBuf) -> Result<()> {
    init_logging();
    let core_config = ConfigBuilder::default().build().await?;
    let repo_root = find_git_root()?;
    let abs_path = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()?.join(path)
    };
    
    let config = CouncilConfig {
        repo_root,
        prompt_version: core_config.prompt_version,
        chair_model: core_config.council_chair_model,
        critic_gpt_model: core_config.council_critic_gpt_model,
        critic_gemini_model: core_config.council_critic_gemini_model,
        implementer_model: core_config.council_implementer_model,
    };
    run_review(config, abs_path).await
}

pub async fn run_fix_args(args: FixArgs) -> Result<()> {
    init_logging();
    let core_config = ConfigBuilder::default().build().await?;
    let repo_root = find_git_root()?;
    let abs_path = if args.path.is_absolute() {
        args.path
    } else {
        std::env::current_dir()?.join(args.path)
    };
    
    let config = CouncilConfig {
        repo_root,
        prompt_version: core_config.prompt_version,
        chair_model: core_config.council_chair_model,
        critic_gpt_model: core_config.council_critic_gpt_model,
        critic_gemini_model: core_config.council_critic_gemini_model,
        implementer_model: core_config.council_implementer_model,
    };
    run_fix(config, abs_path).await
}

pub async fn run(cli: CouncilCli) -> Result<()> {
    init_logging();
    // Load config to get prompt_version
    let core_config = ConfigBuilder::default().build().await?;

    // Determine repo root. For now, assume current dir or find it.
    let repo_root = find_git_root()?;
    
    let config = CouncilConfig {
        repo_root,
        prompt_version: core_config.prompt_version,
        chair_model: core_config.council_chair_model,
        critic_gpt_model: core_config.council_critic_gpt_model,
        critic_gemini_model: core_config.council_critic_gemini_model,
        implementer_model: core_config.council_implementer_model,
    };

    match cli.command {
        CouncilCommand::Review { path, .. } => {
            let abs_path = if path.is_absolute() {
                path
            } else {
                std::env::current_dir()?.join(path)
            };
            run_review(config, abs_path).await?;
        }
        CouncilCommand::Fix(args) => {
            let abs_path = if args.path.is_absolute() {
                args.path
            } else {
                std::env::current_dir()?.join(args.path)
            };
            run_fix(config, abs_path).await?;
        }
        _ => {
            println!("Command not implemented yet.");
        }
    }
    Ok(())
}
