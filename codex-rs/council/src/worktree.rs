use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio::process::Command;
use tracing::info;

pub struct Worktree {
    pub path: PathBuf,
    pub id: String,
}

impl Worktree {
    /// Create a new worktree for a specific council run.
    /// Returns the Worktree struct containing the path.
    pub async fn create(repo_root: &Path, run_id: &str) -> Result<Self> {
        let worktrees_dir = repo_root.join(".council").join("worktrees");
        let path = worktrees_dir.join(run_id);
        
        // Ensure parent dir exists
        tokio::fs::create_dir_all(&worktrees_dir).await
            .context("Failed to create .council/worktrees directory")?;

        info!("Creating worktree for run {} at {:?}", run_id, path);

        // git worktree add --detach <path> HEAD
        // We use --detach to avoid creating a branch name that conflicts if runs are frequent.
        // We can checkout a specific commit if needed later.
        let status = Command::new("git")
            .arg("worktree")
            .arg("add")
            .arg("--detach")
            .arg(&path)
            .arg("HEAD")
            .current_dir(repo_root)
            .status()
            .await
            .context("Failed to execute git worktree add command")?;

        if !status.success() {
            anyhow::bail!("git worktree add failed with status: {}", status);
        }

        Ok(Self {
            path,
            id: run_id.to_string(),
        })
    }

    pub async fn remove(&self) -> Result<()> {
        info!("Removing worktree at {:?}", self.path);
        
        // git worktree remove --force <path>
        // --force is used because we might have modified files (applied patches) that we don't want to keep.
        let status = Command::new("git")
            .arg("worktree")
            .arg("remove")
            .arg("--force")
            .arg(&self.path)
            .status()
            .await
            .context("Failed to execute git worktree remove command")?;

        if !status.success() {
            anyhow::bail!("git worktree remove failed with status: {}", status);
        }
        
        Ok(())
    }
}
