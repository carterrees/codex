use crate::client::CouncilClient;
use crate::context::ContextBuilder;
use crate::parsing;
use crate::prompts;
use crate::types::CouncilConfig;
use crate::types::CouncilEvent;
use crate::types::CouncilMode;
use crate::types::JobOutcome;
use crate::verify::Verifier;
use crate::worktree::Worktree;
use anyhow::Result;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::fs;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::error;

pub struct CouncilRunner {
    pub config: CouncilConfig,
    pub event_tx: mpsc::Sender<CouncilEvent>,
    pub cancel_token: CancellationToken,
    pub job_dir: PathBuf,
}

impl CouncilRunner {
    pub fn new(
        config: CouncilConfig,
        event_tx: mpsc::Sender<CouncilEvent>,
        cancel_token: CancellationToken,
        job_dir: PathBuf,
    ) -> Self {
        Self {
            config,
            event_tx,
            cancel_token,
            job_dir,
        }
    }

    async fn emit(&self, event: CouncilEvent) {
        if let Err(e) = self.event_tx.send(event).await {
            error!("Failed to emit CouncilEvent: {}", e);
        }
    }

    async fn write_debug_log(&self, filename: &str, content: &str) -> Result<()> {
        if std::env::var("THINTHREAD_DEBUG").is_ok() {
            let path = self.job_dir.join(filename);
            fs::write(&path, content).await?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&path).await?.permissions();
                perms.set_mode(0o600);
                fs::set_permissions(&path, perms).await?;
            }
        }
        Ok(())
    }

    pub async fn run(&self, target: PathBuf, mode: CouncilMode) -> Result<()> {
        tokio::select! {
            _ = self.cancel_token.cancelled() => {
                self.emit(CouncilEvent::JobFinished {
                    outcome: JobOutcome::Cancelled,
                    summary_line: "Job cancelled by user.".to_string(),
                }).await;
                Ok(())
            }
            res = self.run_logic(target, mode) => {
                if let Err(ref e) = res {
                    self.emit(CouncilEvent::Error {
                        phase: "Job Execution".to_string(),
                        message: e.to_string(),
                    }).await;
                    self.emit(CouncilEvent::JobFinished {
                        outcome: JobOutcome::Failure,
                        summary_line: format!("Internal Error: {e}"),
                    }).await;
                }
                res
            },
        }
    }

    pub async fn run_logic(&self, target: PathBuf, mode: CouncilMode) -> Result<()> {
        let run_id = self
            .job_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        // 0. Detect Dirty State (Metadata)
        // We run this on the REAL repo root to warn user
        let head_sha = self.get_head_sha(&self.config.repo_root).await?;
        let repo_dirty = self.is_dirty(&self.config.repo_root).await?;

        self.emit(CouncilEvent::JobStarted {
            job_id: run_id.clone(),
            mode,
            target: target.clone(),
            head_sha: head_sha.clone(),
            repo_dirty,
        })
        .await;

        // Persist metadata
        let metadata = serde_json::json!({
            "job_id": run_id,
            "mode": mode,
            "target": target,
            "head_sha_at_start": head_sha,
            "repo_dirty_at_start": repo_dirty,
            "prompt_version": self.config.prompt_version,
            "timestamp": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
        });
        fs::write(
            self.job_dir.join("job_metadata.json"),
            serde_json::to_string_pretty(&metadata)?,
        )
        .await?;

        let rel_target = if target.is_absolute() {
            match target.strip_prefix(&self.config.repo_root) {
                Ok(rel) => rel.to_path_buf(),
                Err(_) => {
                    let target_display = target.display();
                    let repo_root_display = self.config.repo_root.display();
                    self.emit(CouncilEvent::Error {
                        phase: "Context".to_string(),
                        message: format!(
                            "Target '{target_display}' is outside repo root '{repo_root_display}'."
                        ),
                    })
                    .await;
                    self.emit(CouncilEvent::JobFinished {
                        outcome: JobOutcome::Failure,
                        summary_line: "Target outside repo root".to_string(),
                    })
                    .await;
                    return Ok(());
                }
            }
        } else {
            target.clone()
        };

        if rel_target.as_os_str().is_empty() {
            self.emit(CouncilEvent::Error {
                phase: "Context".to_string(),
                message: "Target path is empty.".to_string(),
            })
            .await;
            self.emit(CouncilEvent::JobFinished {
                outcome: JobOutcome::Failure,
                summary_line: "Invalid target path".to_string(),
            })
            .await;
            return Ok(());
        }

        if rel_target.components().any(|c| {
            matches!(
                c,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        }) {
            let rel_target_display = rel_target.display();
            self.emit(CouncilEvent::Error {
                phase: "Context".to_string(),
                message: format!("Target path '{rel_target_display}' contains unsafe components."),
            })
            .await;
            self.emit(CouncilEvent::JobFinished {
                outcome: JobOutcome::Failure,
                summary_line: "Invalid target path".to_string(),
            })
            .await;
            return Ok(());
        }

        // 1. Isolation (Worktree at HEAD)
        self.emit(CouncilEvent::PhaseStarted {
            phase: "Isolation".to_string(),
            step_current: 1,
            step_total: 1,
            detail: format!("Preparing isolated environment ({mode:?})"),
        })
        .await;

        let worktree = Worktree::create(&self.config.repo_root, &run_id).await?;
        let working_root = worktree.path.clone();
        let _worktree_guard = worktree;

        // 2. Build Context (on isolated root)
        self.emit(CouncilEvent::PhaseStarted {
            phase: "Context".to_string(),
            step_current: 1,
            step_total: 1,
            detail: "Analyzing dependencies...".to_string(),
        })
        .await;

        // We must re-target the target path to the isolated root
        let isolated_target = working_root.join(&rel_target);

        // Verify it exists (Snapshot might have failed silently if not in HEAD?)
        if !isolated_target.exists() {
            self.emit(CouncilEvent::Error {
                phase: "Context".to_string(),
                message: format!(
                    "Target file '{}' does not exist in HEAD.",
                    rel_target.display()
                ),
            })
            .await;
            self.emit(CouncilEvent::JobFinished {
                outcome: JobOutcome::Failure,
                summary_line: "Target not found in HEAD".to_string(),
            })
            .await;
            return Ok(());
        }

        let builder = ContextBuilder::new(working_root.clone());
        let bundle = builder
            .build(std::slice::from_ref(&isolated_target))
            .await?;
        let bundle_json = serde_json::to_string_pretty(&bundle)?;

        fs::write(self.job_dir.join("context_bundle.json"), &bundle_json).await?;
        self.emit(CouncilEvent::ArtifactWritten {
            kind: "Context Bundle".to_string(),
            path: self.job_dir.join("context_bundle.json"),
        })
        .await;

        // 3. Verify Baseline (Fix only)
        let mut baseline_results = Vec::new();
        if mode == CouncilMode::Fix {
            self.emit(CouncilEvent::PhaseStarted {
                phase: "Verify (Base)".to_string(),
                step_current: 1,
                step_total: 2,
                detail: "Running baseline verification...".to_string(),
            })
            .await;
            baseline_results = Verifier::run_all(&working_root, Some(&isolated_target)).await?;
            fs::write(
                self.job_dir.join("verify_baseline.json"),
                serde_json::to_string_pretty(&baseline_results)?,
            )
            .await?;
        }

        // 4. Convene Council
        let chair = CouncilClient::new(&self.config.chair_model).await?;
        let critic_gpt = CouncilClient::new(&self.config.critic_gpt_model).await?;
        let critic_gemini = CouncilClient::new(&self.config.critic_gemini_model).await?;
        let implementer = CouncilClient::new(&self.config.implementer_model).await?;

        // 5. Phase 1: Criticism
        self.emit(CouncilEvent::PhaseStarted {
            phase: "Criticism".to_string(),
            step_current: 1,
            step_total: 1,
            detail: "Consulting GPT-5 & Gemini 3...".to_string(),
        })
        .await;

        // Clean up prompt context (remove absolute temp paths)
        let bundle_display = if let Some(working_root) = working_root.to_str()
            && !working_root.is_empty()
        {
            bundle_json.replace(working_root, "")
        } else {
            bundle_json.clone()
        };
        let prompt_context = format!(
            "Target: {:?}\n\nContext Bundle:\n{}\n\nBaseline Verification Results:\n{}",
            rel_target,
            bundle_display,
            serde_json::to_string_pretty(&baseline_results)?
        );

        let critics_fut = async {
            let gpt_fut = critic_gpt.send_message(
                    prompts::system_prompt_critic(&self.config.prompt_version),
                    format!(
                        "Please review this code context and identify bugs or issues.\n\n{prompt_context}",
                    ),
                );
            let gemini_fut = critic_gemini.send_message(
                    prompts::system_prompt_critic(&self.config.prompt_version),
                    format!(
                        "Please review this code context and identify bugs or issues.\n\n{prompt_context}",
                    ),
                );
            tokio::join!(gpt_fut, gemini_fut)
        };

        let (gpt_res, gemini_res) = critics_fut.await;
        let mut critiques = Vec::new();

        if let Ok(c) = gpt_res {
            fs::write(self.job_dir.join("critique_gpt.md"), &c).await?;

            self.write_debug_log("debug_critique_gpt.log", &c).await?;

            critiques.push(format!("### GPT Critique\n\n{c}"));

            self.emit(CouncilEvent::PhaseNote {
                phase: "Criticism".to_string(),

                message: "GPT critique received.".to_string(),
            })
            .await;
        }

        if let Ok(c) = gemini_res {
            fs::write(self.job_dir.join("critique_gemini.md"), &c).await?;

            self.write_debug_log("debug_critique_gemini.log", &c)
                .await?;

            critiques.push(format!("### Gemini Critique\n\n{c}"));

            self.emit(CouncilEvent::PhaseNote {
                phase: "Criticism".to_string(),

                message: "Gemini critique received.".to_string(),
            })
            .await;
        }

        if critiques.is_empty() {
            self.emit(CouncilEvent::Error {
                phase: "Criticism".to_string(),

                message: "All critics failed to respond.".to_string(),
            })
            .await;

            self.emit(CouncilEvent::JobFinished {
                outcome: JobOutcome::Failure,

                summary_line: "Critics failed".to_string(),
            })
            .await;

            return Ok(());
        }

        if mode == CouncilMode::Review {
            // Review mode ends here

            self.emit(CouncilEvent::JobFinished {
                outcome: JobOutcome::Success,

                summary_line: "Critique complete.".to_string(),
            })
            .await;

            return Ok(());
        }

        let all_critiques = critiques.join("\n\n");

        // 6. Phase 2: Planning

        self.emit(CouncilEvent::PhaseStarted {
            phase: "Planning".to_string(),

            step_current: 1,

            step_total: 1,

            detail: "Chair is formulating a plan...".to_string(),
        })
        .await;

        let mut plan = chair

                    .send_message(

                        prompts::system_prompt_chair(&self.config.prompt_version),

                        format!(
                            "Review the following critiques and formulate a fix plan.\n\nContext:\n{prompt_context}\n\nCritiques:\n{all_critiques}",
                        ),

                    )

                    .await?;

        self.write_debug_log("debug_plan_raw.log", &plan).await?;

        fs::write(self.job_dir.join("plan_raw.md"), &plan).await?;

        if self.config.prompt_version == "v2" {
            if let Some(clean_plan) = parsing::extract_plan(&plan) {
                plan = clean_plan;
            } else if let Some(err_msg) = parsing::extract_error(&plan) {
                self.emit(CouncilEvent::Error {
                    phase: "Planning".to_string(),

                    message: format!("Chair refused plan: {err_msg}"),
                })
                .await;

                self.emit(CouncilEvent::JobFinished {
                    outcome: JobOutcome::Failure,

                    summary_line: "Chair refused plan".to_string(),
                })
                .await;

                return Ok(());
            }
        }

        fs::write(self.job_dir.join("plan.md"), &plan).await?;

        // 7. Phase 3: Implementation

        self.emit(CouncilEvent::PhaseStarted {
            phase: "Implementation".to_string(),

            step_current: 1,

            step_total: 1,

            detail: "Generating patch...".to_string(),
        })
        .await;

        let code_change = implementer
            .send_message(
                prompts::system_prompt_implementer(&self.config.prompt_version),
                format!(
                    "Implement the following plan to fix the code.\n\nPlan:\n{plan}\n\nContext:\n{prompt_context}",
                ),
            )
            .await?;

        self.write_debug_log("debug_implementation_raw.log", &code_change)
            .await?;

        fs::write(self.job_dir.join("implementation.patch"), &code_change).await?;

        // Extract Patch
        let patch_content = if let Some(p) = parsing::extract_patch(&code_change) {
            p
        } else {
            // Fallback for v1 or loose parsing
            if code_change.contains("```") {
                code_change
                    .split("```")
                    .nth(1)
                    .unwrap_or(&code_change)
                    .to_string()
            } else {
                code_change.clone()
            }
        };

        // Guard: check if patch looks valid
        if self.config.prompt_version == "v2" && !parsing::looks_like_apply_patch(&patch_content) {
            self.emit(CouncilEvent::Error {
                phase: "Implementation".to_string(),
                message: "Generated patch failed validation (missing markers).".to_string(),
            })
            .await;
            // Continue? Or abort? Abort.
            self.emit(CouncilEvent::JobFinished {
                outcome: JobOutcome::Failure,
                summary_line: "Patch validation failed".to_string(),
            })
            .await;
            return Ok(());
        }

        if let Err(e) = parsing::validate_patch_paths(&patch_content) {
            self.emit(CouncilEvent::Error {
                phase: "Implementation".to_string(),
                message: format!("Generated patch contained unsafe paths: {e}"),
            })
            .await;
            self.emit(CouncilEvent::JobFinished {
                outcome: JobOutcome::Failure,
                summary_line: "Patch safety check failed".to_string(),
            })
            .await;
            return Ok(());
        }

        // 8. Apply & Verify
        self.emit(CouncilEvent::PhaseStarted {
            phase: "Verification".to_string(),
            step_current: 2,
            step_total: 2,
            detail: "Applying patch and verifying...".to_string(),
        })
        .await;

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let apply_res = codex_apply_patch::apply_patch_in_dir(
            &working_root,
            &patch_content,
            &mut stdout,
            &mut stderr,
        );

        fs::write(self.job_dir.join("apply_stdout.txt"), &stdout).await?;
        fs::write(self.job_dir.join("apply_stderr.txt"), &stderr).await?;

        if let Err(e) = apply_res {
            self.emit(CouncilEvent::Error {
                phase: "Verification".to_string(),
                message: format!("Patch application failed: {e}"),
            })
            .await;
            // We persist artifacts but fail job
            self.emit(CouncilEvent::JobFinished {
                outcome: JobOutcome::Failure,
                summary_line: "Patch application failed".to_string(),
            })
            .await;
            return Ok(());
        }

        // Verify
        let final_results = Verifier::run_all(&working_root, Some(&isolated_target)).await?;
        fs::write(
            self.job_dir.join("verify_final.json"),
            serde_json::to_string_pretty(&final_results)?,
        )
        .await?;

        let baseline_failures = baseline_results.iter().filter(|r| !r.success).count();
        let final_failures = final_results.iter().filter(|r| !r.success).count();

        let outcome = if final_failures < baseline_failures {
            JobOutcome::Success
        } else if final_failures > baseline_failures {
            // Regression
            // We still consider the job "Finished", but maybe outcome is Failure?
            // Or Success with a warning?
            // Let's call it Failure for regression.
            JobOutcome::Failure
        } else {
            // No change
            JobOutcome::Success
        };

        let summary =
            format!("Base failures: {baseline_failures}, Final failures: {final_failures}");

        self.emit(CouncilEvent::JobFinished {
            outcome,
            summary_line: summary,
        })
        .await;

        Ok(())
    }

    async fn get_head_sha(&self, root: &Path) -> Result<String> {
        let output = tokio::process::Command::new("git")
            .arg("rev-parse")
            .arg("HEAD")
            .current_dir(root)
            .stderr(Stdio::null())
            .output()
            .await?;
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    async fn is_dirty(&self, root: &Path) -> Result<bool> {
        // git diff --quiet HEAD
        let status = tokio::process::Command::new("git")
            .arg("diff")
            .arg("--quiet")
            .arg("HEAD")
            .current_dir(root)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await?;
        Ok(!status.success())
    }
}
