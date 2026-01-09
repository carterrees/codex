# Implementation Plan: ThinThread Sub-App Integration

This plan follows the "Codex Council" style, mapping features to concrete file changes.

## Phase A: Council Library Refactor (`codex-rs/council`) - [COMPLETED]

**Status:** Completed.
**Implementation Details:**
- **Event System:** `CouncilEvent` and `CouncilMode` defined in `src/types.rs`.
- **Core Logic:** `CouncilRunner` implemented in `src/runner.rs`. It replaces the old monolithic `run_fix` with an event-driven loop using `tokio::sync::mpsc`.
- **Isolation Strategy:** Detached git worktree at `HEAD` (`Worktree` in `src/worktree.rs`) used for both `Review` and `Fix`.
- **Hygiene:** `cleanup_old_jobs` implemented in `src/cleanup.rs` to prune artifacts (>20 runs or >24h old) and remove stale worktrees.
- **CLI Compatibility:** `run_fix` in `src/run.rs` was rewritten to wrap `CouncilRunner` and print events to stdout, ensuring the CLI tool works exactly as before.

## Phase B: TUI Integration (`codex-rs/tui2`) - [COMPLETED]

**Status:** Completed.
**Implementation Details:**
- **Slash Command:** Added `SlashCommand::ThinThread` in `src/slash_command.rs`. Argument parsing (e.g., `fix`, `review`, `apply`) is handled in `src/chatwidget.rs`.
- **Singleton Manager:** `CouncilJobManager` in `src/council_job.rs` enforces single-job concurrency and bridges `CouncilEvent`s to the main TUI event loop.
- **Visuals:** `CouncilProgressCell` in `src/council_progress_cell.rs` renders the live status (Isolation -> Context -> Critique...) using `Arc<Mutex<State>>` for thread-safe updates.
- **Wiring:** `App` in `src/app.rs` now owns `council_job_manager` and handles `AppEvent::CouncilJobEvent`, dispatching updates to the specific history cell.

## Phase C: Apply Gate & Final Polish - [COMPLETED]

**Status:** Completed.
**Implementation Details:**
- **Apply Gate:** `CouncilJobManager::apply_job` (in `src/council_job.rs`) implements the safety check:
    1.  Validates the patch artifact exists and looks like a patch.
    2.  Rejects unsafe paths (`..`, absolute paths, drive prefixes) in patch headers.
    3.  Executes `codex_apply_patch::apply_patch_in_dir(repo_root, ...)` (no process-global `set_current_dir`).
- **Debug Artifacts:** `CouncilRunner::write_debug_log` (in `codex-rs/council/src/runner.rs`) saves raw LLM responses (plans, critiques) to `debug_raw.log` *only* if `THINTHREAD_DEBUG` env var is set.
- **Startup Cleanup:** `App::run` spawns a background task to call `cleanup_old_jobs` on TUI startup, ensuring no stale locks or disk bloat.

## Verified Behavior
- **Tests:** `cargo test -p codex-apply-patch --offline` and `cargo test -p codex-council --offline` pass.
- **Safety:** Patch application is guarded by header path validation and applied relative to an explicit root (no process-global cwd mutation).
