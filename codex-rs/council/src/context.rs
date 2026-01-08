use crate::types::{ContextBundle, FileSnapshot, Snippet, TruncationInfo};
use anyhow::Result;
use ignore::WalkBuilder;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::warn;
use lazy_static::lazy_static;

lazy_static! {
    // Basic Python import regex: import x, from x import y
    static ref PYTHON_IMPORT_RE: Regex = Regex::new(r"(?m)^(?:from|import)\s+([\w\.]+)").unwrap();
}

pub struct ContextBuilder {
    repo_root: PathBuf,
}

impl ContextBuilder {
    pub fn new(repo_root: PathBuf) -> Self {
        Self { repo_root }
    }

    pub async fn build(&self, targets: &[PathBuf]) -> Result<ContextBundle> {
        let repo_root = self.repo_root.clone();
        let targets = targets.to_vec();

        // Run heavy I/O in blocking thread
        tokio::task::spawn_blocking(move || {
            let mut bundle = ContextBundle {
                target_files: Vec::new(),
                related_files: Vec::new(),
                reverse_deps: HashMap::new(),
                test_files: Vec::new(),
                truncation_info: TruncationInfo::default(),
            };

            let mut target_modules = HashSet::new();

            // 1. Process Targets
            for target in &targets {
                if let Ok(content) = fs::read_to_string(target) {
                    bundle.target_files.push(FileSnapshot {
                        path: target.clone(),
                        content: content.clone(),
                        is_truncated: false,
                    });

                    // Identify module name for reverse dep search
                    if let Some(mod_name) = file_to_module(&repo_root, target) {
                        target_modules.insert(mod_name.clone());
                    }

                    // 2. Find Imports (Related Files)
                    // Only for Python for now
                    if target.extension().map_or(false, |e| e == "py") {
                        let imports = extract_imports(&content);
                        for imp in imports {
                            if let Some(path) = resolve_module(&repo_root, &imp) {
                                // Avoid duplicates
                                if !targets.contains(&path) && !bundle.related_files.iter().any(|f| f.path == path) {
                                     if let Ok(c) = fs::read_to_string(&path) {
                                        bundle.related_files.push(FileSnapshot {
                                            path,
                                            content: c, // TODO: Add truncation logic later
                                            is_truncated: false, 
                                        });
                                     }
                                }
                            }
                        }
                    }
                }
            }

            // 3. Reverse Deps
            if !target_modules.is_empty() {
                bundle.reverse_deps = find_reverse_deps(&repo_root, &target_modules);
            }

            // 4. Test Discovery
            bundle.test_files = find_tests(&repo_root, &targets);

            Ok(bundle)
        }).await?
    }
}

fn file_to_module(repo_root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(repo_root).ok()?;
    let stem = rel.file_stem()?.to_string_lossy();
    let parent = rel.parent()?;
    let mut components: Vec<String> = parent.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    components.push(stem.into_owned());
    // Remove empty components (e.g. if parent was empty)
    let components: Vec<String> = components.into_iter().filter(|s| !s.is_empty()).collect();
    Some(components.join("."))
}

fn extract_imports(content: &str) -> Vec<String> {
    PYTHON_IMPORT_RE.captures_iter(content)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
        .collect()
}

fn resolve_module(repo_root: &Path, module: &str) -> Option<PathBuf> {
    let parts: Vec<&str> = module.split('.').collect();
    let mut current = repo_root.to_path_buf();
    for part in &parts {
        current.push(part);
    }
    
    // Check for .py
    let py_path = current.with_extension("py");
    if py_path.exists() {
        return Some(py_path);
    }

    // Check for /__init__.py
    let init_path = current.join("__init__.py");
    if init_path.exists() {
        return Some(init_path);
    }

    None
}

fn find_reverse_deps(repo_root: &Path, modules: &HashSet<String>) -> HashMap<PathBuf, Vec<Snippet>> {
    let mut results = HashMap::new();
    let walker = WalkBuilder::new(repo_root)
        .hidden(true) // Skip hidden files
        .git_ignore(true)
        .build();
    
    for result in walker {
        match result {
            Ok(entry) => {
                if entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                    let path = entry.path();
                    // Naive extension filter
                     if let Some(ext) = path.extension() {
                         let ext_str = ext.to_string_lossy();
                         if ext_str != "py" && ext_str != "rs" && ext_str != "ts" && ext_str != "js" {
                             continue; 
                         }
                     } else {
                         continue;
                     }

                    if let Ok(content) = fs::read_to_string(path) {
                        for module in modules {
                            if content.contains(module) {
                                // Found a potential hit
                                let mut snippets = Vec::new();
                                for (i, line) in content.lines().enumerate() {
                                    if line.contains(module) {
                                        snippets.push(Snippet {
                                            line_start: i + 1,
                                            line_end: i + 1,
                                            content: line.trim().to_string(),
                                        });
                                        if snippets.len() >= 3 {
                                            break;
                                        }
                                    }
                                }
                                if !snippets.is_empty() {
                                    results.insert(path.to_path_buf(), snippets);
                                }
                            }
                        }
                    }
                }
            }
            Err(err) => warn!("Error walking repo: {}", err),
        }
    }
    results
}

fn find_tests(repo_root: &Path, targets: &[PathBuf]) -> Vec<FileSnapshot> {
    let mut tests = Vec::new();
    for target in targets {
        let stem = target.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        // Simple heuristics
        let candidates = vec![
            format!("test_{}.py", stem),
            format!("{}_test.py", stem),
            format!("test_{}.rs", stem), // Rust
        ];
        
        // 1. Sibling check
        if let Some(parent) = target.parent() {
            for candidate in &candidates {
                let sibling = parent.join(candidate);
                if sibling.exists() {
                     if let Ok(c) = fs::read_to_string(&sibling) {
                        tests.push(FileSnapshot { path: sibling, content: c, is_truncated: false });
                    }
                }
            }
        }
        
        // 2. 'tests' folder check (if structure is src/foo.py -> tests/test_foo.py)
        // Assume parallel structure or flat tests folder?
        // Simple check: repo_root/tests/test_foo.py
        let tests_dir = repo_root.join("tests");
        if tests_dir.exists() {
             for candidate in &candidates {
                let test_path = tests_dir.join(candidate);
                if test_path.exists() {
                     if let Ok(c) = fs::read_to_string(&test_path) {
                        tests.push(FileSnapshot { path: test_path, content: c, is_truncated: false });
                    }
                }
            }
        }
    }
    tests
}
