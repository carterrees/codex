use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct CouncilConfig {
    pub repo_root: PathBuf,
    pub prompt_version: String,
    pub chair_model: String,
    pub critic_gpt_model: String,
    pub critic_gemini_model: String,
    pub implementer_model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextBundle {
    pub target_files: Vec<FileSnapshot>,
    pub related_files: Vec<FileSnapshot>, // Imports, etc.
    pub reverse_deps: HashMap<PathBuf, Vec<Snippet>>, // Files that import the target
    pub test_files: Vec<FileSnapshot>,
    pub truncation_info: TruncationInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSnapshot {
    pub path: PathBuf,
    pub content: String,
    pub is_truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snippet {
    pub line_start: usize,
    pub line_end: usize,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TruncationInfo {
    pub omitted_files: Vec<PathBuf>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum CouncilEvent {
    JobStarted {
        job_id: String,
        mode: CouncilMode,
        target: PathBuf,
        head_sha: String,
        repo_dirty: bool,
    },
    PhaseStarted {
        phase: String,
        step_current: usize,
        step_total: usize,
        detail: String,
    },
    PhaseNote {
        phase: String,
        message: String,
    },
    ArtifactWritten {
        kind: String,
        path: PathBuf,
    },
    CommandStarted {
        cmd_display: String,
    },
    CommandFinished {
        cmd_display: String,
        status: String,
        duration_ms: u64,
        truncated: bool,
    },
    Warning {
        message: String,
    },
    Error {
        phase: String,
        message: String,
    },
    JobFinished {
        outcome: JobOutcome,
        summary_line: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CouncilMode {
    Review, // Worktree at HEAD
    Fix,    // Worktree at HEAD
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobOutcome {
    Success,
    Failure,
    Cancelled,
}
