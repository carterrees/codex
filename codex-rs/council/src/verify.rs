use anyhow::Result;
use std::path::Path;
use tokio::process::Command;
use tracing::info;
use tracing::warn;

#[derive(Debug, serde::Serialize)]
pub struct VerifyResult {
    pub command: String,
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

pub struct Verifier;

impl Verifier {
    pub async fn run_all(worktree_path: &Path, target: Option<&Path>) -> Result<Vec<VerifyResult>> {
        let mut results = Vec::new();

        let cargo_manifest = target
            .and_then(|t| find_nearest_cargo_toml(worktree_path, t))
            .or_else(|| {
                let root_manifest = worktree_path.join("Cargo.toml");
                root_manifest.exists().then_some(root_manifest)
            });

        if let Some(manifest_path) = cargo_manifest {
            // Rust Project Detection

            let manifest_arg = manifest_path.to_string_lossy().to_string();

            // 1. Cargo Check
            results.push(
                Self::run_cmd(
                    worktree_path,
                    "cargo",
                    &[
                        "check",
                        "--offline",
                        "--manifest-path",
                        manifest_arg.as_str(),
                    ],
                )
                .await?,
            );

            // 2. Cargo Test
            results.push(
                Self::run_cmd(
                    worktree_path,
                    "cargo",
                    &[
                        "test",
                        "--offline",
                        "--manifest-path",
                        manifest_arg.as_str(),
                    ],
                )
                .await?,
            );
        } else {
            // Fallback (Python defaults)

            // 1. Ruff Format
            results.push(Self::run_cmd(worktree_path, "ruff", &["format", "."]).await?);

            // 2. Ruff Check
            results.push(Self::run_cmd(worktree_path, "ruff", &["check", "."]).await?);

            // 3. Pytest
            results.push(Self::run_cmd(worktree_path, "pytest", &["-q"]).await?);
        }

        Ok(results)
    }

    async fn run_cmd(cwd: &Path, program: &str, args: &[&str]) -> Result<VerifyResult> {
        info!("Running verification: {} {}", program, args.join(" "));

        // Check if program exists (optional, but good for error messages)
        // For now, let Command fail if not found.

        let output = Command::new(program)
            .args(args)
            .current_dir(cwd)
            .output()
            .await;

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                let success = out.status.success();

                if !success {
                    warn!(
                        "Verification failed: {} {}
Stdout: {}
Stderr: {}",
                        program,
                        args.join(" "),
                        stdout,
                        stderr
                    );
                }

                Ok(VerifyResult {
                    command: format!("{} {}", program, args.join(" ")),
                    success,
                    stdout,
                    stderr,
                })
            }
            Err(e) => {
                warn!("Failed to execute {}: {}", program, e);
                Ok(VerifyResult {
                    command: format!("{} {}", program, args.join(" ")),
                    success: false,
                    stdout: "".to_string(),
                    stderr: e.to_string(),
                })
            }
        }
    }
}

fn find_nearest_cargo_toml(worktree_root: &Path, target: &Path) -> Option<std::path::PathBuf> {
    let start_dir = if target.is_dir() {
        target
    } else {
        target.parent()?
    };

    for dir in start_dir.ancestors() {
        if !dir.starts_with(worktree_root) {
            break;
        }

        let candidate = dir.join("Cargo.toml");
        if candidate.exists() {
            return Some(candidate);
        }

        if dir == worktree_root {
            break;
        }
    }

    None
}
