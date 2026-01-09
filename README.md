# ThinThread (Codex Council)

**ThinThread** is the “Council” workflow inside the Codex CLI: a multi-agent, verification-first way to review and fix code.

You’ll see two names:
- **ThinThread**: the UX/product name (TUI header, `/thinthread ...` slash commands).
- **Codex Council**: the underlying implementation (crate names like `codex-council`, internal modules, and older docs).

**Note:** This is a private fork of the original Codex CLI, transformed into an autonomous engineering orchestration system.

<p align="center">
  <img src="./.github/codex-cli-splash.png" alt="ThinThread splash" width="80%" />
</p>
</br>

## What is it?

Standard coding agents often guess at fixes and break downstream code. ThinThread aims to make “AI codegen” behave more like an engineering team:

1. **Isolation:** create a temporary **git worktree** at `HEAD` and do all experimentation there.
2. **Adversarial review:** multiple models independently critique the code (different strengths, different failure modes).
3. **Governance:** a chair synthesizes critiques into a deterministic plan.
4. **Verification:** run project-appropriate checks/tests in the isolated worktree before proposing anything to your real working tree.

## 🚀 How to use it

### 0) Install

```bash
cd codex-rs
cargo install --path cli --bin codex --locked --force --root ~/.cargo
```

Optional alias if you prefer the “ThinThread” command name:

```bash
alias thinthread=codex
```

### 1) Interactive TUI (recommended)

Start the TUI:

```bash
codex
```

Inside the TUI, ThinThread is driven by slash commands:

- `/thinthread review path/to/file.rs` — run the “Criticism” phase and stop (read-only)
- `/thinthread fix path/to/file.rs` — run Criticism → Planning → Implementation → Verification (still read-only to your working tree)
- `/thinthread apply <run-id>` — apply a completed run’s patch to your real working tree

### 2) Council CLI (non-interactive)

The CLI entrypoint is the `council` subcommand:

```bash
codex council review path/to/file.rs
codex council fix path/to/file.rs
```

Today:
- `codex council review` and `codex council fix` are implemented.
- The CLI has placeholders for `apply/status/show`, but apply is currently best done via the TUI (`/thinthread apply <run-id>`) or by manually using the artifacts (next section).

## Where outputs go (artifacts)

Each run writes artifacts under the repo you ran it from:

- `.council/runs/<run-id>/`
  - `job_metadata.json` — mode, target, `HEAD` sha, dirty flag, timestamp
  - `context_bundle.json` — context used to prompt the models
  - `plan.md` and `plan_raw.md`
  - `implementation.patch` — the raw model output (v2 embeds `*** Begin Patch` / `*** End Patch` inside `<patch>`)
  - `verify_baseline.json` / `verify_final.json` — verification output

Run isolation uses git worktrees:
- `.council/worktrees/<run-id>/` — the detached worktree used for the run

## Safety model (high level)

- Model output is treated as untrusted text.
- Patch headers are validated (no absolute paths; no `..` traversal; includes `*** Move to:`).
- Patch application is done relative to an explicit root (no process-global `chdir`).
- Your real working tree is only modified when you explicitly apply (`/thinthread apply ...`).

## Models / configuration

Config lives in `~/.codex/config.toml`. The key one for the Council is `prompt_version`:

```toml
prompt_version = "v2"

[features]
tui2 = true
```

ThinThread/Council model selection is also configurable (names may appear top-level depending on config layout):

```toml
council_chair_model = "gpt-5.2-2025-12-11"
council_critic_gpt_model = "gpt-5.1-codex"
council_critic_gemini_model = "gemini-3-pro-preview"
council_implementer_model = "gemini-3-flash-preview"
```

## Troubleshooting

**Q: I don’t see the ThinThread ASCII header.**  
A: Ensure `tui2 = true` under `[features]` (this is the default in recent builds).

**Q: `thinthread` not found.**  
A: Run `codex` directly, or add `alias thinthread=codex` and ensure `~/.cargo/bin` is in your `$PATH`.

## Docs

- `COUNCIL.md` (high-level workflow)
- `codex-rs/council/README.md` (crate deep dive)
- `docs/getting-started.md` (general Codex CLI usage)

This repository is licensed under the [Apache-2.0 License](LICENSE).
