# Codex (Private Fork)

**Note:** This is a private fork of the original Codex CLI. This repository is for personal, non-commercial use and experimentation.

<p align="center"><code>npm i -g @openai/codex</code><br />or <code>brew install --cask codex</code></p>
<p align="center"><strong>Codex CLI</strong> is a coding agent that runs locally on your computer.
<p align="center">
  <img src="./.github/codex-cli-splash.png" alt="Codex CLI splash" width="80%" />
</p>
</br>

## New Feature: Council

This fork introduces **Council**, a deterministic, multi-model review and repair workflow designed for high-assurance code changes.

### Key Features
- **Worktree Isolation**: All changes are verified in a temporary git worktree before touching your main working directory.
- **Context Awareness**: Automatically detects imports, reverse dependencies, and related tests to ensure changes don't break downstream code.
- **Verification Pipeline**: Enforces `ruff format`, `ruff check`, and `pytest` on every patch.
- **Roles**: Distinct "Reviewer", "Chair", and "Writer" roles (powered by different LLMs) to separate concerns.

### Usage
```bash
# Review a file without changing it
codex council review src/my_module.py

# Fix a file (creates worktree -> plans -> patches -> verifies -> prompts to apply)
codex council fix src/my_module.py
```

---

## Quickstart (Original)

### Installing and running Codex CLI

Install globally with your preferred package manager:

```shell
# Install using npm
npm install -g @openai/codex
```

```shell
# Install using Homebrew
brew install --cask codex
```

Then simply run `codex` to get started.

<details>
<summary>You can also go to the <a href="https://github.com/openai/codex/releases/latest">latest GitHub Release</a> and download the appropriate binary for your platform.</summary>

Each GitHub Release contains many executables, but in practice, you likely want one of these:

- macOS
  - Apple Silicon/arm64: `codex-aarch64-apple-darwin.tar.gz`
  - x86_64 (older Mac hardware): `codex-x86_64-apple-darwin.tar.gz`
- Linux
  - x86_64: `codex-x86_64-unknown-linux-musl.tar.gz`
  - arm64: `codex-aarch64-unknown-linux-musl.tar.gz`

Each archive contains a single entry with the platform baked into the name (e.g., `codex-x86_64-unknown-linux-musl`), so you likely want to rename it to `codex` after extracting it.

</details>

### Using Codex with your ChatGPT plan

Run `codex` and select **Sign in with ChatGPT**. We recommend signing into your ChatGPT account to use Codex as part of your Plus, Pro, Team, Edu, or Enterprise plan. [Learn more about what's included in your ChatGPT plan](https://help.openai.com/en/articles/11369540-codex-in-chatgpt).

You can also use Codex with an API key, but this requires [additional setup](https://developers.openai.com/codex/auth#sign-in-with-an-api-key).

## Docs

- [**Codex Documentation**](https://developers.openai.com/codex)
- [**Contributing**](./docs/contributing.md)
- [**Installing & building**](./docs/install.md)
- [**Open source fund**](./docs/open-source-fund.md)

This repository is licensed under the [Apache-2.0 License](LICENSE).