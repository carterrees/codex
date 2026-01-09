use anyhow::Context;
use anyhow::Result;
use std::path::Path;
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::process::Command;
use tracing::info;

pub struct Snapshot {
    pub path: PathBuf,
    pub _temp_dir: TempDir,
}

impl Snapshot {
    pub async fn create(repo_root: &Path, targets: &[PathBuf]) -> Result<Self> {
        let temp_dir = TempDir::new()?;
        let path = temp_dir.path().to_path_buf();

        info!("Creating snapshot at {:?}", path);

        for target in targets {
            let rel_path = target.strip_prefix(repo_root).with_context(|| {
                format!("Target {target:?} is not inside repo root {repo_root:?}")
            })?;

            let dest = path.join(rel_path);
            if let Some(parent) = dest.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }

            let rel_str = rel_path.to_string_lossy();
            // Git needs forward slashes even on Windows for revisions
            let git_path = if std::path::MAIN_SEPARATOR != '/' {
                rel_str.replace(std::path::MAIN_SEPARATOR, "/")
            } else {
                rel_str.into_owned()
            };

            let object = format!("HEAD:{git_path}");

            let output = Command::new("git")
                .arg("show")
                .arg(&object)
                .current_dir(repo_root)
                .output()
                .await?;

            if !output.status.success() {
                // If it fails (e.g. file new in worktree but not in HEAD), we just warn and skip?
                // Or we fail? The spec says "Review on HEAD". If not in HEAD, we can't review it.
                // We should probably fail or handle gracefully.
                // Let's fail for now to be explicit.
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("git show {object} failed: {stderr}");
            }

            tokio::fs::write(&dest, output.stdout).await?;
        }

        Ok(Self {
            path,
            _temp_dir: temp_dir,
        })
    }
}

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
        tokio::fs::create_dir_all(&worktrees_dir)
            .await
            .context("Failed to create .council/worktrees directory")?;

        info!("Creating worktree for run {} at {:?}", run_id, path);

        // git worktree add --detach <path> HEAD
        // We use --detach to avoid creating a branch name that conflicts if runs are frequent.
        // We can checkout a specific commit if needed later.
        let output = Command::new("git")
            .arg("worktree")
            .arg("add")
            .arg("--detach")
            .arg(&path)
            .arg("HEAD")
            .current_dir(repo_root)
            .output()
            .await
            .context("Failed to execute git worktree add command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("git worktree add failed with status: {}. Stderr: {}", output.status, stderr);
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
        let output = Command::new("git")
            .arg("worktree")
            .arg("remove")
            .arg("--force")
            .arg(&self.path)
            .output()
            .await
            .context("Failed to execute git worktree remove command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("git worktree remove failed with status: {}. Stderr: {}", output.status, stderr);
        }

        Ok(())
    }
}
