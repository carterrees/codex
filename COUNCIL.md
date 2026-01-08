# Codex Council

**Codex Council** is a multi-agent orchestration layer for Codex that uses a committee of AI models (GPT-5, Gemini 3) to review and fix code with high reliability.

## Quick Start

1.  **Build**:
    ```bash
    cargo install --path codex-rs/cli --bin codex --root ~/.cargo --force
    mv ~/.cargo/bin/codex ~/.cargo/bin/codex-council
    ```

2.  **Environment**:
    ```bash
    export OPENAI_API_KEY="..."
    export GEMINI_API_KEY="..."
    ```

3.  **Run**:
    ```bash
    # Fix a file autonomously
    codex-council council fix src/main.rs
    ```

For full documentation, see [codex-rs/council/README.md](codex-rs/council/README.md).
