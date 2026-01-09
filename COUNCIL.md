# ThinThread (formerly Codex Council)

**ThinThread** is a multi-agent orchestration layer for Codex that uses a committee of AI models (GPT-5, Gemini 3) to review and fix code with high reliability.

## Workflow

1. **Context Building:** ThinThread analyzes the file you want to fix (`target`) and gathers relevant context (imports, definitions, tests).
2. **Worktree Isolation:** It creates a temporary git worktree to run the fix safely.
3. **Phase 1: Criticism:** Two critics (GPT-5.1 & Gemini 3 Pro) review the code and identify bugs.
4. **Phase 2: Planning:** The Chair (GPT-5.2) synthesizes the critiques into a strict plan.
5. **Phase 3: Implementation:** The Implementer (Gemini 3 Flash) writes the code patch.
6. **Phase 4: Apply & Verify:** The patch is applied and verification commands (lints/tests) are run.

## Usage

### Interactive Chat (TUI)
Run `codex` (or `thinthread` if you alias it) to start the TUI. The header will show “ThinThread”.

### Council CLI (Fixer)
To fix a bug:
```bash
codex council fix src/lib.rs
```

## Configuration

In `~/.codex/config.toml`:

```toml
prompt_version = "v2"
council_chair_model = "gpt-5.2-2025-12-11"
council_implementer_model = "gemini-3-flash-preview"
```
