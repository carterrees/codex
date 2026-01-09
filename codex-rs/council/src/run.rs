use crate::runner::CouncilRunner;
use crate::types::CouncilConfig;
use crate::types::CouncilEvent;
use crate::types::CouncilMode;
use anyhow::Result;
use std::path::PathBuf;
use tokio::fs;
use tokio_util::sync::CancellationToken;
use tracing::info;

pub async fn run_review(config: CouncilConfig, target: PathBuf) -> Result<()> {
    run_internal(config, target, CouncilMode::Review).await
}

pub async fn run_fix(config: CouncilConfig, target: PathBuf) -> Result<()> {
    run_internal(config, target, CouncilMode::Fix).await
}

async fn run_internal(config: CouncilConfig, target: PathBuf, mode: CouncilMode) -> Result<()> {
    let (tx, mut rx) = tokio::sync::mpsc::channel(100);
    let cancel_token = CancellationToken::new();

    let run_id = format!(
        "run-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs()
    );
    let job_dir = config.repo_root.join(".council").join("runs").join(&run_id);
    fs::create_dir_all(&job_dir).await?;

    let runner = CouncilRunner::new(config, tx, cancel_token, job_dir);

    let runner_fut = runner.run(target.clone(), mode);
    let target_for_print = target.clone();
    let run_id_print = run_id.clone();
    let job_dir_print = job_dir.clone();
    let printer_fut = async {
        while let Some(event) = rx.recv().await {
            match event {
                CouncilEvent::JobStarted { job_id, mode, .. } => {
                    info!("Job {} started in {:?} mode", job_id, mode);
                }
                CouncilEvent::PhaseStarted {
                    phase,
                    step_current,
                    step_total,
                    detail,
                } => {
                    info!("[{}/{}] {}: {}", step_current, step_total, phase, detail);
                }
                CouncilEvent::PhaseNote { phase, message } => {
                    info!("  ({}): {}", phase, message);
                }
                CouncilEvent::ArtifactWritten { kind, path } => {
                    info!("  Saved {} to {:?}", kind, path);
                }
                CouncilEvent::Warning { message } => {
                    tracing::warn!("Warning: {}", message);
                }
                CouncilEvent::Error { phase, message } => {
                    tracing::error!("Error in {}: {}", phase, message);
                }
                CouncilEvent::JobFinished {
                    outcome,
                    summary_line,
                } => {
                    info!("Job Finished: {:?} - {}", outcome, summary_line);
                    if mode == CouncilMode::Review && outcome == JobOutcome::Success {
                         let plan_path = job_dir_print.join("plan.md");
                         if let Ok(plan) = fs::read_to_string(&plan_path).await {
                             println!("\n─────────────── 📋 Review Findings ───────────────\n");
                             println!("{}", plan.trim());
                             println!("\n──────────────────────────────────────────────────");
                         }
                         println!("\n👉 Next Step: thinthread fix {} (Review ID: {})", target_for_print.display(), run_id_print);
                    }
                    break;
                }
                _ => {}
            }
        }
    };

    let (res, _) = tokio::join!(runner_fut, printer_fut);
    res
}
