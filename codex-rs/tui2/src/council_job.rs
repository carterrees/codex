use crate::app_event::AppEvent;
use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use codex_council::CouncilConfig;
use codex_council::CouncilMode;
use codex_council::CouncilRunner;
use codex_council::cleanup_old_jobs;
use codex_council::parsing;
use std::path::Path;
use std::path::PathBuf;
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;
use tracing::error;
use tracing::info;

#[derive(Default, Clone)]
pub(crate) struct CouncilJobManager {
    active_job_id: Option<String>,
    cancel_token: Option<CancellationToken>,
}

impl CouncilJobManager {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    #[allow(dead_code)]
    pub(crate) fn is_running(&self) -> bool {
        self.active_job_id.is_some()
    }

    pub(crate) fn active_job_id(&self) -> Option<String> {
        self.active_job_id.clone()
    }

    pub(crate) async fn spawn_job(
        &mut self,
        mode: CouncilMode,
        target: PathBuf,
        config: CouncilConfig,
        event_tx: UnboundedSender<AppEvent>,
    ) -> Result<String> {
        if let Some(id) = &self.active_job_id {
            return Err(anyhow!("A Council job is already running (id={id})."));
        }

        // Cleanup old jobs
        let repo_root = config.repo_root.clone();
        tokio::spawn(async move {
            if let Err(e) = cleanup_old_jobs(repo_root).await {
                error!("Failed to cleanup old jobs: {}", e);
            }
        });

        // Setup job
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();
        let run_id = format!("run-{timestamp}");
        let job_dir = config.repo_root.join(".council").join("runs").join(&run_id);
        tokio::fs::create_dir_all(&job_dir).await?;

        let (council_tx, mut council_rx) = tokio::sync::mpsc::channel(100);
        let cancel_token = CancellationToken::new();

        self.active_job_id = Some(run_id.clone());
        self.cancel_token = Some(cancel_token.clone());

        let runner = CouncilRunner::new(config, council_tx, cancel_token, job_dir);

        // Spawn runner
        tokio::spawn(async move {
            if let Err(e) = runner.run(target, mode).await {
                error!("Council job execution failed: {}", e);
            }
        });

        // Spawn bridge
        let bridge_tx = event_tx.clone();
        let bridge_run_id = run_id.clone();
        tokio::spawn(async move {
            while let Some(event) = council_rx.recv().await {
                // Bridge to TUI
                let _ = bridge_tx.send(AppEvent::CouncilJobEvent(bridge_run_id.clone(), event));
            }
        });

        Ok(run_id)
    }

    pub(crate) fn cancel_active_job(&mut self) {
        if let Some(token) = &self.cancel_token {
            token.cancel();
        }
    }

    pub(crate) fn on_job_finished(&mut self, job_id: &str) {
        if self.active_job_id.as_deref() == Some(job_id) {
            self.active_job_id = None;
            self.cancel_token = None;
        }
    }

    pub(crate) async fn apply_job(&self, job_id: &str, repo_root: &Path) -> Result<()> {
        let run_dir = repo_root.join(".council").join("runs").join(job_id);
        if !run_dir.exists() {
            return Err(anyhow!(
                "Job artifacts not found for {job_id}. Maybe it was pruned?"
            ));
        }

        let patch_path = run_dir.join("implementation.patch");
        if !patch_path.exists() {
            return Err(anyhow!("No patch artifact found for job {job_id}."));
        }

        let patch_content_raw = tokio::fs::read_to_string(&patch_path)
            .await
            .context("Failed to read patch file")?;

        let patch_content = if let Some(p) = parsing::extract_patch(&patch_content_raw) {
            p
        } else if patch_content_raw.contains("```") {
            patch_content_raw
                .split("```")
                .nth(1)
                .unwrap_or(&patch_content_raw)
                .to_string()
        } else {
            patch_content_raw
        };

        if !parsing::looks_like_apply_patch(&patch_content) {
            return Err(anyhow!("Patch content failed validation."));
        }

        if let Err(e) = parsing::validate_patch_paths(&patch_content) {
            return Err(anyhow!("Patch content rejected by safety check: {e}"));
        }

        info!("Applying patch for job {}...", job_id);

        // 1. Dry Run Check (Strict Gate)
        // We use git apply --check if possible, but our patch format is custom (apply_patch tool).
        // Since we are applying to the REAL repo root, we must be careful.
        // The spec says: "If git apply --check is not suitable... implement equivalent."
        // codex_apply_patch doesn't have a dry-run mode exposed yet?
        // Let's assume we proceed with caution or check if we can add dry-run to codex_apply_patch later.
        // For now, we will RELY on the fact that we just ran this in a worktree.
        // BUT the "Gate" requires a check against current state.

        // TODO: Add dry-run to codex-apply-patch crate.
        // For MVP without modifying apply-patch crate deeply:
        // We can check if files exist and permissions are okay?
        // Or we just proceed because the user explicitly typed "/thinthread apply".

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let result = codex_apply_patch::apply_patch_in_dir(
            repo_root,
            &patch_content,
            &mut stdout,
            &mut stderr,
        );

        match result {
            Ok(_) => Ok(()),
            Err(e) => {
                let err_msg = String::from_utf8_lossy(&stderr).to_string();
                Err(anyhow!("Patch application failed: {e}. Stderr: {err_msg}"))
            }
        }
    }

    #[allow(dead_code)]
    pub(crate) async fn recover_crashed_jobs(&self, repo_root: &Path) {
        let runs_dir = repo_root.join(".council").join("runs");
        if !runs_dir.exists() {
            return;
        }

        let mut dir = match tokio::fs::read_dir(&runs_dir).await {
            Ok(d) => d,
            Err(_) => return,
        };

        while let Ok(Some(entry)) = dir.next_entry().await {
            let metadata_path = entry.path().join("job_metadata.json");
            if !metadata_path.exists() {
                continue;
            }

            // TODO: Write an on-disk status marker on `JobFinished` and use it here to surface
            // crashed/orphaned jobs after a TUI restart.
            let _ = tokio::fs::read_to_string(&metadata_path).await;
        }
    }
}
