use crate::worktree::Worktree;
use crate::verify::Verifier;
use crate::context::ContextBuilder;
use crate::client::CouncilClient;
use crate::prompts;
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
    
    // 1. Build Context
    info!("Building context...");
    let builder = ContextBuilder::new(config.repo_root.clone());
    let bundle = builder.build(&[target.clone()]).await?;
    let bundle_json = serde_json::to_string_pretty(&bundle)?;
    
    // Setup runs dir
    let run_dir = config.repo_root.join(".council").join("runs").join(&run_id);
    let context_dir = run_dir.join("context");
    let verify_dir = run_dir.join("verify");
    let discussion_dir = run_dir.join("discussion");
    fs::create_dir_all(&context_dir).await?;
    fs::create_dir_all(&verify_dir).await?;
    fs::create_dir_all(&discussion_dir).await?;

    // Persist Context Bundle
    fs::write(context_dir.join("bundle.json"), &bundle_json).await?;
    info!("Context bundle saved.");

    // 2. Create worktree
    let worktree = Worktree::create(&config.repo_root, &run_id).await?;
    info!("Worktree created at {:?}", worktree.path);
    
    // 3. Verify (Baseline)
    info!("Running baseline verification...");
    let results = Verifier::run_all(&worktree.path).await?;
    fs::write(verify_dir.join("baseline.json"), serde_json::to_string_pretty(&results)?).await?;

    // 4. Initialize Council
    info!("Convening the Council...");
    let chair = CouncilClient::new(prompts::MODEL_CHAIR).await?;
    let critic_gpt = CouncilClient::new(prompts::MODEL_CRITIC_GPT).await?;
    let critic_gemini = CouncilClient::new(prompts::MODEL_CRITIC_GEMINI).await?;
    let implementer = CouncilClient::new(prompts::MODEL_IMPLEMENTER).await?;

    // 5. Phase 1: Criticism
    info!("Phase 1: Criticism");
    let prompt_context = format!(
        "Target: {:?}\n\nContext Bundle:\n{}\n\nBaseline Verification Results:\n{}",
        target, bundle_json, serde_json::to_string_pretty(&results)?
    );

    let critics_fut = async {
        let gpt_fut = critic_gpt.send_message(
            prompts::system_prompt_critic(),
            format!("Please review this code context and identify bugs or issues.\n\n{}", prompt_context)
        );
        let gemini_fut = critic_gemini.send_message(
            prompts::system_prompt_critic(),
            format!("Please review this code context and identify bugs or issues.\n\n{}", prompt_context)
        );
        tokio::join!(gpt_fut, gemini_fut)
    };

    let (gpt_res, gemini_res) = critics_fut.await;
    
    let mut critiques = Vec::new();
    if let Ok(c) = gpt_res {
        fs::write(discussion_dir.join("critique_gpt.md"), &c).await?;
        critiques.push(format!("### GPT Critique\n\n{}", c));
    }
    if let Ok(c) = gemini_res {
        fs::write(discussion_dir.join("critique_gemini.md"), &c).await?;
        critiques.push(format!("### Gemini Critique\n\n{}", c));
    }

    let all_critiques = critiques.join("\n\n");

    // 6. Phase 2: Planning (Chair)
    info!("Phase 2: Planning (Chair)");
    let plan = chair.send_message(
        prompts::system_prompt_chair(),
        format!(
            "Review the following critiques and formulate a fix plan.\n\nContext:\n{}\n\nCritiques:\n{}",
            prompt_context, all_critiques
        )
    ).await?;
    fs::write(discussion_dir.join("plan.md"), &plan).await?;

    // 7. Phase 3: Implementation
    info!("Phase 3: Implementation");
    let implementation_prompt = format!(
        "Implement the following plan to fix the code.\n\nPlan:\n{}\n\nContext:\n{}",
        plan, prompt_context
    );
    
    let code_change = implementer.send_message(
        prompts::system_prompt_implementer(),
        implementation_prompt
    ).await?;
    fs::write(discussion_dir.join("implementation.patch"), &code_change).await?;

    // 8. Apply and Verify
    info!("Phase 4: Apply & Verify");
    
    // Change to worktree directory so relative paths in patch work
    let original_cwd = std::env::current_dir()?;
    std::env::set_current_dir(&worktree.path)?;
    
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    
    // Try to extract the patch from code block if present
    let patch_content = if code_change.contains("```") {
        code_change.split("```")
            .nth(1)
            .unwrap_or(&code_change)
            .trim()
            .to_string()
    } else {
        code_change.clone()
    };

    // Remove language identifier if present (e.g., ```diff)
    let patch_content = if patch_content.starts_with("diff") {
        patch_content.replacen("diff", "", 1)
    } else if patch_content.starts_with("patch") {
        patch_content.replacen("patch", "", 1)
    } else {
        patch_content
    };

    match codex_apply_patch::apply_patch(&patch_content, &mut stdout, &mut stderr) {
        Ok(_) => {
            info!("Patch applied successfully.");
            fs::write(discussion_dir.join("apply_stdout.txt"), &stdout).await?;
            fs::write(discussion_dir.join("apply_stderr.txt"), &stderr).await?;
        }
        Err(e) => {
            tracing::error!("Failed to apply patch: {}", e);
            fs::write(discussion_dir.join("apply_error.txt"), e.to_string()).await?;
            // Don't fail the run, just record it and maybe try verification anyway (though unlikely to pass if patch failed)
        }
    }
    
    // Restore CWD
    std::env::set_current_dir(original_cwd)?;

    // Run verification on patched worktree
    info!("Running final verification...");
    let final_results = Verifier::run_all(&worktree.path).await?;
    fs::write(verify_dir.join("final.json"), serde_json::to_string_pretty(&final_results)?).await?;

    // Compare results
    let baseline_failures = results.iter().filter(|r| !r.success).count();
    let final_failures = final_results.iter().filter(|r| !r.success).count();
    
    info!("Baseline failures: {}, Final failures: {}", baseline_failures, final_failures);
    
    if final_failures < baseline_failures {
        info!("SUCCESS: Verification improved.");
    } else if final_failures > baseline_failures {
        tracing::warn!("REGRESSION: Verification got worse.");
    } else {
        info!("NO CHANGE: Verification failures count remained the same.");
    }

    info!("Council run complete. See {:?} for artifacts.", run_dir);
    
    Ok(())
}
