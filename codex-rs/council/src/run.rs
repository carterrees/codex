use crate::worktree::Worktree;
use crate::verify::Verifier;
use crate::context::ContextBuilder;
use anyhow::Result;
use std::path::PathBuf;
use tracing::info;
use tokio::fs;

pub struct CouncilConfig {
    pub repo_root: PathBuf,
}

pub async fn run_review(_config: CouncilConfig, target: PathBuf) -> Result<()> {
    info!("Starting council review for {:?}", target);
    println!("Council review not implemented yet.");
    Ok(())
}

pub async fn run_fix(config: CouncilConfig, target: PathBuf) -> Result<()> {
    info!("Starting council fix for {:?}", target);
    
    let run_id = format!("run-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs());
    
    // 1. Build Context (on main repo, before worktree or inside? Main repo is safer for finding deps)
    info!("Building context...");
    let builder = ContextBuilder::new(config.repo_root.clone());
    let bundle = builder.build(&[target.clone()]).await?;
    
    // Setup runs dir
    let run_dir = config.repo_root.join(".council").join("runs").join(&run_id);
    let context_dir = run_dir.join("context");
    let verify_dir = run_dir.join("verify");
    fs::create_dir_all(&context_dir).await?;
    fs::create_dir_all(&verify_dir).await?;

    // Persist Context Bundle
    fs::write(context_dir.join("bundle.json"), serde_json::to_string_pretty(&bundle)?).await?;
    info!("Context bundle saved with {} related files and {} reverse deps.", 
        bundle.related_files.len(), bundle.reverse_deps.len());

    // 2. Create worktree
    let worktree = Worktree::create(&config.repo_root, &run_id).await?;
    info!("Worktree created at {:?}", worktree.path);
    
    // 3. Verify (Baseline)
    // Currently runs on the fresh worktree (no patch).
    let results = Verifier::run_all(&worktree.path).await?;
    
    // Persist verify results
    for res in &results {
        let filename = if res.command.contains("ruff format") {
            "ruff_format.txt"
        } else if res.command.contains("ruff check") {
            "ruff_check.txt"
        } else if res.command.contains("pytest") {
            "pytest.txt"
        } else {
            "other.txt"
        };
        
        let content = format!("Command: {}\nSuccess: {}\nStdout:\n{}\nStderr:\n{}", 
            res.command, res.success, res.stdout, res.stderr);
        fs::write(verify_dir.join(filename), content).await?;
    }
    
    fs::write(verify_dir.join("exit_codes.json"), serde_json::to_string_pretty(&results)?).await?;
    
    info!("Verification complete. Results saved to {:?}", verify_dir);

    // 4. Cleanup (not implemented yet per plan)
    
    Ok(())
}
