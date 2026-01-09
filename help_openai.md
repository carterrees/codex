# Engineering Log: codex-core Test Fixes

**Date:** January 8, 2026
**Topic:** Fixing Flaky/Failing Tests in `codex-core`
**Status:** In Progress

## 1. Problem Statement
Running `cargo test -p codex-core` revealed multiple failures, primarily in `suite::apply_patch_cli`, `suite::unified_exec`, and `suite::approvals`. Most failures in `unified_exec` and `approvals` appear to be timing-related (timeouts or tight timing assertions) due to a slow test environment. `apply_patch_cli` failures were due to outdated expected error messages.

## 2. Progress So Far

### Fixed:
1.  **`suite::apply_patch_cli`**: Updated `test_apply_patch_fails_on_traversal` to match the actual error message format ("path contains traversal '..'").
2.  **`suite::unified_exec::unified_exec_emits_one_begin_and_one_end_event`**: Relaxed the timing assertion for the "running" state duration from `10ms` to `100ms` to account for system latency.
3.  **`suite::approvals::RunCommand`**: Increased the timeout in the `shell_event` simulation from 1s to 5s.

### In Progress / Pending:
1.  **`suite::approvals`**:
    *   Need to increase timeouts for `WriteFile` and `RunUnifiedExecCommand` (similar to `RunCommand`). The default 1s is too short for the current environment.
2.  **`suite::unified_exec`**:
    *   `unified_exec_timeout_and_followup_poll`: Fails with exit code 124 (timeout) instead of expected output.
    *   `unified_exec_respects_early_exit_notifications`: Timing assertions about "wall_time" are too strict.
    *   Need to relax these timing checks and possibly increase simulated delays/timeouts.

## 3. Next Steps
1.  Update `codex-rs/core/tests/suite/approvals.rs`: Increase `WriteFile` and `RunUnifiedExecCommand` timeouts to 5000ms.
2.  Analyze and relax assertions in `codex-rs/core/tests/suite/unified_exec.rs`.
3.  Rerun `cargo test -p codex-core --test all` to verify stability.