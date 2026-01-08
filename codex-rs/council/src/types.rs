use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

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
