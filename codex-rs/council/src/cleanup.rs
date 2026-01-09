use anyhow::Result;
use std::path::PathBuf;
use std::time::Duration;
use std::time::SystemTime;
use tokio::fs;
use tracing::info;
use tracing::warn;

pub async fn cleanup_old_jobs(repo_root: PathBuf) -> Result<()> {
    let runs_dir = repo_root.join(".council").join("runs");
    if !runs_dir.exists() {
        return Ok(());
    }

    let mut entries = Vec::new();
    let mut dir = fs::read_dir(&runs_dir).await?;

    while let Some(entry) = dir.next_entry().await? {
        if let Ok(meta) = entry.metadata().await
            && meta.is_dir()
            && let Ok(created) = meta.created().or_else(|_| meta.modified())
        {
            entries.push((entry.path(), created));
        }
    }

    // Sort by creation time (newest first)
    entries.sort_by(|a, b| b.1.cmp(&a.1));

    let now = SystemTime::now();
    let retention_limit = 20;
    let age_limit = Duration::from_secs(24 * 60 * 60); // 24 hours

    let mut to_remove = Vec::new();

    for (i, (path, created)) in entries.iter().enumerate() {
        let mut should_remove = false;

        // Rule 1: Count limit
        if i >= retention_limit {
            should_remove = true;
        }

        // Rule 2: Age limit
        if let Ok(age) = now.duration_since(*created)
            && age > age_limit
        {
            should_remove = true;
        }

        if should_remove {
            to_remove.push(path.clone());
        }
    }

    for run_path in to_remove {
        let run_id = run_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if run_id.is_empty() {
            continue;
        }

        info!("Cleaning up old council run: {}", run_id);

        // 1. Check for worktree
        let worktree_path = repo_root.join(".council").join("worktrees").join(run_id);
        if worktree_path.exists() {
            info!("Removing associated worktree: {:?}", worktree_path);
            // git worktree remove --force <path>
            // We use a simple command here.
            let status = tokio::process::Command::new("git")
                .arg("worktree")
                .arg("remove")
                .arg("--force")
                .arg(&worktree_path)
                .current_dir(&repo_root) // Run from repo root
                .status()
                .await;

            match status {
                Ok(s) => {
                    if !s.success() {
                        warn!(
                            "Failed to remove worktree {} (status {}). Attempting manual cleanup.",
                            run_id, s
                        );
                        // If git fails (e.g. index locked or already gone), we try to remove dir.
                        let _ = fs::remove_dir_all(&worktree_path).await;
                        // And verify git prune? "git worktree prune" might be needed later.
                    }
                }
                Err(e) => {
                    warn!("Failed to execute git worktree remove: {}", e);
                    let _ = fs::remove_dir_all(&worktree_path).await;
                }
            }
        }

        // 2. Remove run artifacts
        if let Err(e) = fs::remove_dir_all(&run_path).await {
            warn!("Failed to remove run directory {:?}: {}", run_path, e);
        }
    }

    Ok(())
}
