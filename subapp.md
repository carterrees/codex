## Final Build Spec: `/thinthread` Council Job Runner Embedded in TUI

### Goal

Enable a user to run the full Council loop (Critique → Plan → Patch → Verify) **from inside the ThinThread TUI** via a native slash command, without leaving the app, without freezing the render loop, and without polluting stdout. The result must be **safe-by-default** (explicit apply gate), **deterministic**, and **operationally robust** (singleton concurrency + retention policy).

### Success Criteria

1. `/thinthread review <file>` produces a Critic report as a persisted artifact and posts a compact summary into the chat history.
2. `/thinthread fix <file>` runs the full loop in an isolated worktree, streams structured progress events, and persists artifacts including a patch file.
3. TUI remains responsive during Council jobs (no blocking render loop; cancellable).
4. If the repo is dirty, the UI clearly warns: **“Council runs on HEAD; uncommitted changes ignored.”** (Option C).
5. Only **one** Council job can run at a time (MVP singleton enforcement).
6. Job artifacts/worktrees are pruned automatically (retention policy).
7. Patch application is guarded: path confinement + patch validation + dry-run check, then explicit user apply.

---

## Architectural Overview

### Mental model

This is **not a sub-TUI**. It is a background **job runner** that emits a structured event stream and produces durable artifacts.

### System diagram (conceptual)

```text
[TUI Render Loop]
    |
    |  /thinthread fix src/foo.rs
    v
[Slash Command Router] --> [CouncilJobManager (singleton + retention)]
                               |
                               | spawn job (blocking-safe)
                               v
                         [CouncilRunner (library)]
                               |
                               | emits CouncilEvent via channel
                               v
                     [Event Bridge -> TUI Event Queue]
                               |
                               v
                    [CouncilProgressCell(job_id)]
                               |
                               v
                 [Artifacts on disk + explicit apply gate]
```

---

## Redline Requirements Incorporated

### A) Dirty State Paradox (State Consistency)

**Defined behavior (Option C, transparency):**

* Council operates on **HEAD**, not the working tree.
* On job start, detect dirty state (tracked changes and optionally untracked).
* If dirty, the progress cell must show:

**WARNING:** `Running on HEAD (uncommitted changes ignored).`

Additionally:

* Persist `job_metadata.json` including:

  * `head_sha_at_start`
  * `repo_dirty_at_start: true/false`
  * `dirty_files_at_start` (optional; at least count)

**New alias to address the real UX need:**

* `/thinthread review @dirty` targets the set of files modified vs HEAD (tracked files).

  * Implementation: `git diff --name-only HEAD` (tracked) and optionally `git ls-files --others --exclude-standard` (untracked) if you want.

### B) Concurrency & Rate Limits

**MVP constraint: singleton job**

* CouncilJobManager allows **one active job**.
* If a job is running and the user invokes another:

  * Respond with a deterministic rejection:

    * “A Council job is already running (job_id=...). Use `/thinthread cancel <job_id>` or wait.”
* No job queue in MVP (queues create confusing UX); add later if needed.

### C) Artifact Rot (Disk Hygiene)

**Retention policy required**

* Jobs are stored under a dedicated directory (not repo root clutter), e.g.:

  * macOS/Linux: `~/.cache/thinthread/council_runs/` (or OS-appropriate cache dir)
  * Windows: equivalent local app data cache
* Implement `cleanup_old_jobs()`:

  * Run at TUI startup and on each job completion.
  * Policy: keep last **N=20** jobs AND delete jobs older than **24 hours** (whichever is stricter).
* Worktrees associated with pruned jobs must be removed safely.

### D) Startup Hygiene (Crash Recovery)

**Stale Lock Detection**

* On TUI startup, scan for jobs marked `RUNNING`.
* Check if the process ID (PID) recorded in `job_metadata.json` is still alive and matches the Council process signature.
* If dead/mismatched: transition state to `CRASHED` to unblock the Singleton lock.

---

## Command Interface

### Slash commands

```text
/thinthread review <path| @ref>
/thinthread fix <path| @ref> [--yes]

/thinthread review @dirty

/thinthread list
/thinthread show <job_id> (critique|plan|patch|verify|summary)
/thinthread cancel <job_id>

/thinthread apply <job_id> [--yes]
```

### File resolution

* Accept `src/foo.rs` and ` @src/foo.rs`.
* Must resolve to a **relative path within repo root**.
* Deny absolute paths, `..` traversal, and symlink escapes.

---

## CouncilRunner Library Contract (Phase A)

### Core invariants

* **No stdout/progress bar rendering at the library boundary**
* **Structured events only**
* **Cancellable**
* **Artifacts persisted deterministically**
* **Command execution hardened**

### Execution Strategy

* **Fix Mode:** Uses full **Worktree Isolation**.
  * Creates a fresh git worktree to run tests/builds safely without locking the main repo index.
* **Review Mode:** Uses **Snapshot Strategy**.
  * No full worktree creation (too slow/heavy).
  * Extracts target files (and necessary context) at HEAD into a temp directory using `git show`.
  * Guarantees "Review on HEAD" semantics with minimal latency.

### CouncilEvent (structured stream)

Replace percentage progress with step counters.

```text
CouncilEvent
- JobStarted { job_id, mode, target, head_sha, repo_dirty }
- PhaseStarted { phase, step_current, step_total, detail }
- PhaseNote { phase, message }                       // coalesced; low volume
- ArtifactWritten { kind, path }
- CommandStarted { cmd_display }
- CommandFinished { cmd_display, status, duration_ms, truncated }
- Warning { message }
- Error { phase, message }
- JobFinished { outcome, summary_line }
```

Rules:

* No raw payloads/tokens or full file contents in events.
* Any logged output must be redacted + truncated before storage and before emission.

### Worktree hygiene

* Worktrees are created under the job directory, not the repo root.
* Each job has:

  * `job_dir/`

    * `worktree/` (only for fix mode)
    * `snapshot/` (for review mode)
    * `critique.xml`
    * `plan.xml`
    * `patch.apply_patch`
    * `verify.log` (redacted/truncated)
    * `summary.json`
    * `job_metadata.json`
    * `debug_raw.log` (OPTIONAL, see below)

### Debug Artifacts

* **Default:** `verify.log` and others are redacted/truncated.
* **Debug Mode:** If explicitly enabled (e.g., config or env var), write `debug_raw.log`.
  * **Guardrails:** File permission `0600`. Never displayed in UI. Local only.

### External command execution (security + stability)

Centralize in `CommandRunner`:

* argv-array execution only (no shell interpolation)
* cwd pinned to worktree/snapshot
* env allowlist (default minimal)
* timeouts (per command + global)
* output caps + truncation
* redaction (secrets/PII patterns + known env var values if present)

---

## TUI Integration Contract (Phase B)

### CouncilJobManager (singleton)

Responsibilities:

* enforce single active job
* spawn CouncilRunner in blocking-safe lane
* own cancellation tokens
* bridge events to TUI event queue
* maintain job registry for `/list` and `/show`
* run retention cleanup and **startup crash recovery**

### CouncilProgressCell

Minimum UX requirements:

* shows job header: mode, target, job_id
* shows repo state: `HEAD=<sha>` and dirty warning if applicable
* shows phase checklist with running/done status indicators
* collapsible detail: last few PhaseNote lines
* final state: success/failure/cancelled with short summary
* includes discoverable follow-ups:

  * `/thinthread show <job_id> patch`
  * `/thinthread apply <job_id>`

---

## Apply Gate (Phase C)

### PatchArtifact validation (must exist before apply)

Before apply is allowed:

1. Parse patch and compute diffstat (files + lines).
2. Validate all file paths are relative and within repo root (path traversal guard).
3. Flag destructive ops (Delete File) prominently.
4. Ensure patch format is valid apply_patch.

### Double-Check Apply Transaction

1. **UX Enablement Check (Pre-flight):**
   * Run `git apply --check` or equivalent dry-run to enable the "Apply" button/command.
   * If fail: Disable button, show "Repo changed".

2. **Atomic Execution Check (The Gate):**
   * Upon user confirmation (`--yes` or interactive click):
   * **IMMEDIATELY** re-run the check.
   * If check fails: Abort operation with "State changed during confirmation".
   * If check passes: Apply patch.

### Explicit confirmation

* `/thinthread apply <job_id>` prompts confirmation (unless `--yes`).
* Apply uses your safe patch applier (apply_patch library), not ad-hoc shell operations.

---

## Performance Optimizations (Speed Without Fragility)

1. **Toolchain probe cache**: Detect and cache commands (cargo/pnpm/pytest/etc.) with TTL. High ROI, safe.
2. **Context Build**: Defer complex hash-based invalidation for MVP. Use minimal file listing cache if beneficial.
3. **Coalesced UI updates**: coalesce PhaseNote events to avoid UI thrash.
4. **Bounded channels**: prevent runaway memory under spammy logging.
5. **spawn_blocking for external processes**: do not starve async runtime or UI.
