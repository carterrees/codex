# Codex Council

Codex Council is an advanced, automated code review and repair workflow built into Codex. It orchestrates a "council" of AI agents to rigorously analyze, critique, plan, and fix issues in your codebase.

## Architecture

The Council consists of specialized roles filled by different LLMs:

1.  **Critics**: Multiple models (e.g., `gpt-5.1-codex`, `gemini-3-pro-preview`) analyze the code in parallel to identify bugs, security flaws, and style violations.
2.  **Chair**: A high-reasoning model (e.g., `gpt-5.2-2025-12-11`) synthesizes the critiques and formulates a robust fix plan.
3.  **Implementer**: A fast, capable coding model (e.g., `gemini-3-flash-preview`) writes the actual patch based on the Chair's plan.

The process runs in an isolated **git worktree** to ensure safety and allow for "apply and verify" steps without messing up your working directory until the fix is proven.

## Prerequisites

You need API keys for the models used by the Council. Set them in your environment:

```bash
export OPENAI_API_KEY="sk-..."
export GEMINI_API_KEY="AIza..."
```

## Installation

To build and install the `codex-council` binary:

```bash
# Navigate to the repo root
cd codex-rs

# Install the binary (renaming it to avoid conflict with standard codex if desired)
cargo install --path cli --bin codex --root ~/.cargo --force
mv ~/.cargo/bin/codex ~/.cargo/bin/codex-council
```

## Usage

### 1. CLI Mode (Direct Fix)

To run the Council on a specific file directly from your terminal:

```bash
codex-council council fix path/to/your/file.rs
```

This will:
1.  Create a temporary worktree.
2.  Run baseline verification (tests/lints).
3.  Convene the Council to critique and plan a fix.
4.  Generate a patch.
5.  Apply the patch and run verification again.
6.  Report the results and location of artifacts (in `.council/runs/`).

### 2. Interactive Mode (TUI)

You can launch the Codex Council TUI to explore your codebase and run council commands interactively.

```bash
codex-council
```

Once inside the TUI, you can use the command prompt (usually `/`) to issue council commands if configured, or use the standard Codex chat interface to ask for a "council review" of a file (functionality integration dependent on slash commands).

*Note: The primary entry point for the autonomous loop is currently the CLI subcommand `council fix`.*

## Output & Artifacts

All run data is stored in `.council/runs/<run-id>/`. You can find:
- **Context**: `context/bundle.json` (the code snapshot used).
- **Discussion**: `discussion/` (critiques, plan, and generated patch).
- **Verification**: `verify/` (logs of test runs before and after).
