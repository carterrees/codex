# LLM Council on Codex: Build Spec (Worktree Default)

## 0) Goal and non-goals

### Goal
Add a first-class “Council” workflow to your Codex fork so you can:
- operate in any repo like normal Codex (`codex interactive` / `exec`)
- run a deterministic multi-model review + plan + patch cycle:
  - `codex council review` path
  - `codex council fix` path (review → plan → approval → patch → verify → optional apply)
- be **context-aware across files** (imports + dependents + tests)
- use **apply_patch** patches end-to-end (not raw git apply)
- verify with **ruff + pytest**
- run everything in a **git worktree by default**

### Non-goals (initial milestone)
- Full MCP server integration
- Fully general multi-language semantic graph
- Perfect call-graph accuracy (we’ll do deterministic heuristics first, then upgrade to tree-sitter)

## 1) UX and command surface

### 1.1 New commands
Add to `codex-rs/cli/src/main.rs`:
- `codex council review <PATH> [--scope auto|imports|impact|all] [--json]`
- `codex council fix <PATH> [--yes] [--redundant] [--scope ...] [--full-tests]`
- `codex council apply <RUN_ID> [--yes]`
- `codex council status <RUN_ID>`
- `codex council show <RUN_ID> [--plan|--patch|--verify]`

**Behavioral contracts**
- `review` produces: findings + plan (no code changes)
- `fix` produces: patch + verification results, then asks to apply (unless `--yes`)
- `apply` applies the patch from a prior run into the *main repo working tree* (after it already passed verification in worktree)

### 1.2 Compatibility with Codex workflows
Council should feel like a native subcommand (like review, apply, exec). It must:
- honor Codex config profiles and `-c key=value` overrides
- use the same CWD semantics (`-C`) and `.codex` discovery behavior where relevant
- store artifacts in a predictable repo-local directory

## 2) Artifact storage layout
Store everything under repo root: `.council/runs/<run_id>/`

Suggested structure:
- `meta.json` (timestamps, versions, config snapshot, models)
- `context/`
  - `bundle.json` (computed dependency graph + file list + truncation notes)
  - `files/<hash>__relative_path.ext` (raw content snapshots)
- `reviews/`
  - `perf.json`
  - `sec.json`
- `chair/`
  - `plan.json`
  - `plan.md` (pretty-printed for humans)
- `patch/`
  - `apply_patch.txt` (the only thing that modifies code)
- `verify/`
  - `ruff_format.txt`
  - `ruff_check.txt`
  - `pytest.txt`
  - `exit_codes.json`
- `final/`
  - `summary.md` (what happened, what changed)
  - `changed_files.json`

**Design rule:** a run directory must be sufficient to reproduce or reapply the change later.

## 3) Worktree strategy (default)

### 3.1 Default mode
- Create a worktree rooted at: `.council/worktrees/<run_id>/`
- Apply patch and run verification **in the worktree**
- If verification passes, prompt: “Apply to main working tree? (y/N)” unless `--yes`

### 3.2 Worktree lifecycle
- Keep worktree by default for debugging (or configurable retention):
  - `council.worktree.retain = true`
  - `council.worktree.prune_after_hours = 24`
- Provide `codex council cleanup` later, but not required initially.

### 3.3 Implementation detail
Use `git worktree add` and `git worktree remove`. Prefer using whatever git utilities exist in `utils/git` (workspace member `codex-git`) if they expose helpers; otherwise spawn git via `std::process::Command`.

## 4) Context awareness: how Council knows cross-file impact
This is the key piece you asked about: “a change in one script may impact another.”

### 4.1 Context bundle: deterministic and layered
Council builds a **ContextBundle** before any model calls. It contains:
1. **Target file(s)**
   - Full content for the file(s) directly requested.
2. **Import closure**
   - Parse imports in the target file(s).
   - Resolve local modules → file paths (Python: `import x.y`, `from x import y`)
   - Include:
     - full contents for “small” files
     - “API surface only” (signatures/docstrings) for large files
3. **Reverse dependency set**
   - Find files that import the target module or referenced symbols.
   - Include top call-sites as snippets (line ranges).
4. **Test closure**
   - Identify tests likely impacted:
     - filename heuristics (`foo.py` → `test_foo.py`)
     - grep for module import path
     - grep for primary symbols
   - Include full tests if small, else relevant snippets.
5. **Repo map (compressed global awareness)**
   - A short “map” of the repo: file list + per-file symbol summary
   - This is what gives a “Principal Engineer” the ability to recommend architectural changes without reading every file.

### 4.2 Context selection limits
Set strict limits to avoid “infinite context” creep:
- `max_files_total` (default 40)
- `max_bytes_per_file` (default 80KB)
- `max_total_bytes` (default ~1.5–2.5MB raw before compression)
- `max_snippets_per_reverse_dep` (default 3)

When limits are hit, record truncation in `bundle.json` so the chair can see what was omitted.

### 4.3 Implementation approach
MVP: deterministic heuristics
- Use `ignore` crate and `@.gitignore` rules to walk repo
- Use regex-based import parsing for Python initially
- Use ripgrep (or Rust `ignore` search) for reverse deps

V2: tree-sitter-backed extraction
- Since workspace already has tree-sitter, upgrade symbol extraction and import parsing for higher fidelity.

## 5) Patch format contract: apply_patch only
You already have `codex-rs/apply-patch` which implements:
- parsing `*** Begin Patch` / `*** Update File` hunks
- applying hunks to filesystem
- handling chunk context matching and tricky EOF/newline patterns

### 5.1 Council writer output format
The “writer” model must output **only**:
```
*** Begin Patch
*** Update File: relative/or/absolute/path.py
@codex-rs/tui2/src/snapshots/codex_tui__status_indicator_widget__tests__renders_with_queued_messages@macos.snap ...
*** End Patch
```
No prose. No markdown. No explanations.

### 5.2 Applying patches
Council should apply patches using the crate directly (preferred for council), not `git apply`.
- call `codex_apply_patch::apply_patch(patch, &mut stdout, &mut stderr)` inside worktree
- capture stdout/stderr into `verify/apply_patch_stdout.txt` and `verify/apply_patch_stderr.txt`

### 5.3 “Fuzzy apply” concern
The “git apply brittleness” problem largely disappears because `apply-patch` already uses context-based matching. That is precisely why Council should standardize on this patch format.

## 6) Verification pipeline (ruff + pytest)

### 6.1 Default commands
Run in the worktree:
1. `ruff format .`
2. `ruff check .`
3. `pytest -q`

Capture output per command and exit code in `verify/`.

### 6.2 Failure handling (bounded retries)
If any step fails:
- feed the **exact tool output** back to the writer as “repair context”
- require another `apply_patch`
- reapply to a fresh worktree state (or reset the worktree)
- **max_repair_iterations = 2** (default)

No infinite loops.

## 7) Council agent roles (default fast mode)
You asked about “two reviewers implement their way and chairman reaches consensus.” We are making that an *optional* flag, not default, because of merge complexity and latency.

Default is:

### 7.1 Default topology (fast + high quality)
- Parallel reviewers:
  - Reviewer A: performance / maintainability / idioms
  - Reviewer B: security / correctness / edge cases
- Chairman synthesizes a single plan
- Human approves plan (CLI)
- Single writer generates patch
- Both reviewers verify patch (“sign off”)
- Apply + verify tooling enforces truth

### 7.2 Optional redundant mode (`--redundant`)
- After plan approval:
  - Two independent writers generate patches
  - Chairman selects best patch (or merges)
- This is slower but can be valuable for risky refactors.

## 8) Configuration (TOML) and discovery

### 8.1 File `council.toml` at repo root (primary), plus optional global defaults later.

### 8.2 Example schema
```toml
[council]
worktree_default = true
runs_dir = ".council/runs"
worktrees_dir = ".council/worktrees"
max_repair_iterations = 2

[council.context]
scope_default = "auto"
max_files_total = 40
max_bytes_per_file = 80000
max_total_bytes = 2000000

[council.verify]
commands = ["ruff format .", "ruff check .", "pytest -q"]

[council.models]
chair = "gpt-5.2"
review_security = "gpt-5"
review_perf = "gemini-3"
writer_default = "gpt-5.2"
```

### 8.3 Override precedence
- CLI flags override TOML
- Codex `-c key=value` overrides should be allowed to override council values if you want everything under one mechanism (optional). Initially, council can just read its own TOML.

## 9) Code organization (Rust modules/crates)

### 9.1 New crate recommended: `council` (workspace member)
Create a new workspace crate `council` so `cli` stays thin.
- `codex-rs/council/`
  - `src/lib.rs`
  - `src/run.rs` (state machine)
  - `src/context.rs` (context bundle builder)
  - `src/worktree.rs` (worktree creation/cleanup)
  - `src/models.rs` (prompt templates + request/response parsing)
  - `src/verify.rs` (ruff/pytest)
  - `src/storage.rs` (run dir persistence)
  - `src/types.rs` (ContextBundle, Review, Plan, etc.)

Then `cli/src/council_cmd.rs` becomes a thin wrapper that calls `codex_council::run(...)`.

### 9.2 Reuse existing crates
- `apply-patch` for patch application
- `utils/git` for git operations if useful
- `ignore`/`walkdir` already in workspace deps for repo scanning
- `utils/cache` for memoizing repo map / context extraction (future performance)

## 10) State machine (deterministic)
Define explicit states persisted to `meta.json`:
- `DISCOVERING_CONTEXT`
- `REVIEW_RUNNING`
- `PLAN_READY`
- `AWAITING_APPROVAL`
- `PATCH_RUNNING`
- `PATCH_APPLIED_TO_WORKTREE`
- `VERIFY_RUNNING`
- `READY_TO_APPLY`
- `APPLIED_TO_MAIN`
- `FAILED` (with reason)

This makes resume straightforward later.

## 11) Implementation milestones (pragmatic sequence)

### Milestone 1 — Command scaffolding (1–2 PRs)
- Wire `codex council` into `codex-rs/cli/src/main.rs`
- Create `cli/src/council_cmd.rs` that calls into a stub `codex_council` library
- Create run directories, print status, no model calls yet

### Milestone 2 — Worktree + patch apply + verify (most leverage)
- Create worktree
- Load patch from a file (for now)
- Apply via `codex-apply-patch`
- Run ruff/pytest
- Persist outputs

This alone gives you an operational “verify/apply engine” usable even before the council is fully wired.

### Milestone 3 — Context builder (repo awareness MVP)
- Implement import closure + reverse deps + test closure
- Persist `bundle.json` + file snapshots

### Milestone 4 — Council model loop (review → plan → patch)
- Parallel reviewer calls (tokio join)
- Chairman plan synthesis
- HITL approval in CLI
- Writer patch generation
- Verifier sign-off prompts (optional; tooling is the ultimate check)

### Milestone 5 — Redundant mode and resilience
- `--redundant` dual patches + chair selection
- bounded repair loop on verification failures
- status/show/resume polish

## 12) The key design choice you just made (worktree default): what it buys you
- no risk of corrupting your active working tree
- deterministic “clean slate” for patch+tests
- reproducible run artifacts you can audit later
- makes “Council as judgment layer” safe to run frequently

If you want to proceed immediately, the single most valuable next step is Milestone 1 and 2: **worktree + apply_patch + ruff/pytest**, because it gives you the “closed-loop correctness gate” that everything else builds on. After that, Council is mostly orchestration and prompting.
