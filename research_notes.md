# ThinThread (Council) Research Notes & Future Improvements

This document tracks observations, limitations, and feature ideas arising from user testing of the ThinThread (Council) workflow.

## 1. Job State Persistence & Resumption
**Observation:** If a user runs `/thinthread review` or `/thinthread fix` and then exits the TUI (or crashes), the interactive "Job Progress Card" is lost. While artifacts persist in `.council/runs/`, the user cannot "resume" the UI to see the checklist or easily find the ID to apply.

**Idea: `/thinthread resume <JOB_ID>`**
- **Goal:** Allow re-attaching the TUI to an existing (running or finished) job.
- **Implementation:**
    - `CouncilJobManager` needs to scan `.council/runs` on startup (already partially implemented in `recover_crashed_jobs` but needs UI wiring).
    - Hydrate a `CouncilProgressCell` from the `job_metadata.json` and `plan.md` on disk.
    - If the job is still running (background process), re-attach the event channel (harder).
    - If the job is finished, just render the final state.

## 2. Review -> Fix Handover
**Observation:** Users intuitively expect `/thinthread review <file>` to "set the stage" for `/thinthread fix`. Currently, `fix` starts a fresh job and re-runs the criticism phase.

**Idea: Explicit Handover**
- **Command:** `/thinthread fix --from-review <JOB_ID>` or implicit "Fix the last review".
- **Benefit:** Saves tokens and time by reusing the "Criticism" phase artifacts from the review run.
- **Implementation:**
    - `CouncilRunner` needs a mode to skip `Phase::Criticism` and ingest an existing `plan.md` or `findings.json`.
    - Ensure the file hasn't changed since the review (hash check).

## 3. UI Polish & "Ghost" States
**Observation:**
- **Ghost Messages:** When a new job starts, sometimes summary messages from the *previous* job flash on screen or persist in the history in a confusing way.
- **Status Bar:** The TUI status bar might need to be more "job aware" (e.g., "Job Running: 01JG...").

**Fixes:**
- Ensure `App` strictly scopes event handlers to the *active* job ID.
- Verify `CouncilProgressCell` rendering doesn't cache stale state across different cell instances.

## 4. "Apply" Workflow Clarity
**Observation:** Users were confused about whether `fix` automatically applies changes or requires a separate step. The ID was hard to find.

**Fix (Implemented in v0.80.1):**
- Added explicit "👉 NEXT STEP" prompt in the UI.
- Added Job ID to the header.

**Future Idea:**
- **Interactive "Apply" Button:** If TUI supports mouse/interactive elements better, a clickable `[Apply]` button in the chat would be ideal.
- **Auto-Apply Prompt:** Instead of just printing "Run apply...", the TUI could prompt: "Job finished. Apply patch now? (y/n)" immediately in the chat input.

## 5. Artifact Management
**Observation:** `.council/runs` can grow large.
- **Current:** `cleanup_old_jobs` runs on spawn.
- **Idea:** A visible `/thinthread clean` command to manually prune old runs/worktrees.

## 6. Docstring/Code Sync
**Observation:** The "Critic" naturally finds docstring inconsistencies *after* the code is fixed (Run 2), effectively pipelining "Correctness" then "Documentation".
- **Idea:** Embrace this. explicitly model a "Polish" phase after "Implementation" to check for docstring drift.

## 7. Council Member Visibility
**Observation:** The UI shows generic phase names ("Criticism", "Planning") but doesn't show *which* Council member is active (e.g., "Reviewer A (Security)", "Reviewer B (Performance)", "Chairman"). Users want to see "who has the script."
- **Idea:** Update `CouncilRunner` to emit `PhaseStarted` events with the specific *persona name* in the `detail` field.
- **Benefit:** Increases user trust and engagement by showing the "multi-agent" nature of the system live.
