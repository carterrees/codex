# Rust/codex-rs

In the codex-rs folder where the rust code lives:

- Crate names are prefixed with `codex-`. For example, the `core` folder's crate is named `codex-core`
- When using format! and you can inline variables into {}, always do that.
- Install any commands the repo relies on (for example `just`, `rg`, or `cargo-insta`) if they aren't already available before running instructions here.
- Never add or modify any code related to `CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR` or `CODEX_SANDBOX_ENV_VAR`.
  - You operate in a sandbox where `CODEX_SANDBOX_NETWORK_DISABLED=1` will be set whenever you use the `shell` tool. Any existing code that uses `CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR` was authored with this fact in mind. It is often used to early exit out of tests that the author knew you would not be able to run given your sandbox limitations.
  - Similarly, when you spawn a process using Seatbelt (`/usr/bin/sandbox-exec`), `CODEX_SANDBOX=seatbelt` will be set on the child process. Integration tests that want to run Seatbelt themselves cannot be run under Seatbelt, so checks for `CODEX_SANDBOX=seatbelt` are also often used to early exit out of tests, as appropriate.
- Always collapse if statements per https://rust-lang.github.io/rust-clippy/master/index.html#collapsible_if
- Always inline format! args when possible per https://rust-lang.github.io/rust-clippy/master/index.html#uninlined_format_args
- Use method references over closures when possible per https://rust-lang.github.io/rust-clippy/master/index.html#redundant_closure_for_method_calls
- When writing tests, prefer comparing the equality of entire objects over fields one by one.
- When making a change that adds or changes an API, ensure that the documentation in the `docs/` folder is up to date if applicable.

Run `just fmt` (in `codex-rs` directory) automatically after making Rust code changes; do not ask for approval to run it. Before finalizing a change to `codex-rs`, run `just fix -p <project>` (in `codex-rs` directory) to fix any linter issues in the code. Prefer scoping with `-p` to avoid slow workspace‑wide Clippy builds; only run `just fix` without `-p` if you changed shared crates. Additionally, run the tests:

1. Run the test for the specific project that was changed. For example, if changes were made in `codex-rs/tui`, run `cargo test -p codex-tui`.
2. Once those pass, if any changes were made in common, core, or protocol, run the complete test suite with `cargo test --all-features`.
   When running interactively, ask the user before running `just fix` to finalize. `just fmt` does not require approval. project-specific or individual tests can be run without asking the user, but do ask the user before running the complete test suite.

## TUI style conventions

See `codex-rs/tui/styles.md`.

## TUI code conventions

- Use concise styling helpers from ratatui’s Stylize trait.
  - Basic spans: use "text".into()
  - Styled spans: use "text".red(), "text".green(), "text".magenta(), "text".dim(), etc.
  - Prefer these over constructing styles with `Span::styled` and `Style` directly.
  - Example: patch summary file lines
    - Desired: vec!["  └ ".into(), "M".red(), " ".dim(), "tui/src/app.rs".dim()]

### TUI Styling (ratatui)

- Prefer Stylize helpers: use "text".dim(), .bold(), .cyan(), .italic(), .underlined() instead of manual Style where possible.
- Prefer simple conversions: use "text".into() for spans and vec![…].into() for lines; when inference is ambiguous (e.g., Paragraph::new/Cell::from), use Line::from(spans) or Span::from(text).
- Computed styles: if the Style is computed at runtime, using `Span::styled` is OK (`Span::from(text).set_style(style)` is also acceptable).
- Avoid hardcoded white: do not use `.white()`; prefer the default foreground (no color).
- Chaining: combine helpers by chaining for readability (e.g., url.cyan().underlined()).
- Single items: prefer "text".into(); use Line::from(text) or Span::from(text) only when the target type isn’t obvious from context, or when using .into() would require extra type annotations.
- Building lines: use vec![…].into() to construct a Line when the target type is obvious and no extra type annotations are needed; otherwise use Line::from(vec![…]).
- Avoid churn: don’t refactor between equivalent forms (Span::styled ↔ set_style, Line::from ↔ .into()) without a clear readability or functional gain; follow file‑local conventions and do not introduce type annotations solely to satisfy .into().
- Compactness: prefer the form that stays on one line after rustfmt; if only one of Line::from(vec![…]) or vec![…].into() avoids wrapping, choose that. If both wrap, pick the one with fewer wrapped lines.

### Text wrapping

- Always use textwrap::wrap to wrap plain strings.
- If you have a ratatui Line and you want to wrap it, use the helpers in tui/src/wrapping.rs, e.g. word_wrap_lines / word_wrap_line.
- If you need to indent wrapped lines, use the initial_indent / subsequent_indent options from RtOptions if you can, rather than writing custom logic.
- If you have a list of lines and you need to prefix them all with some prefix (optionally different on the first vs subsequent lines), use the `prefix_lines` helper from line_utils.

## Tests

### Snapshot tests

This repo uses snapshot tests (via `insta`), especially in `codex-rs/tui`, to validate rendered output. When UI or text output changes intentionally, update the snapshots as follows:

- Run tests to generate any updated snapshots:
  - `cargo test -p codex-tui`
- Check what’s pending:
  - `cargo insta pending-snapshots -p codex-tui`
- Review changes by reading the generated `*.snap.new` files directly in the repo, or preview a specific file:
  - `cargo insta show -p codex-tui path/to/file.snap.new`
- Only if you intend to accept all new snapshots in this crate, run:
  - `cargo insta accept -p codex-tui`

If you don’t have the tool:

- `cargo install cargo-insta`

### Test assertions

- Tests should use pretty_assertions::assert_eq for clearer diffs. Import this at the top of the test module if it isn't already.
- Prefer deep equals comparisons whenever possible. Perform `assert_eq!()` on entire objects, rather than individual fields.
- Avoid mutating process environment in tests; prefer passing environment-derived flags or dependencies from above.

### Spawning workspace binaries in tests (Cargo vs Bazel)

- Prefer `codex_utils_cargo_bin::cargo_bin("...")` over `assert_cmd::Command::cargo_bin(...)` or `escargot` when tests need to spawn first-party binaries.
  - Under Bazel, binaries and resources may live under runfiles; use `codex_utils_cargo_bin::cargo_bin` to resolve absolute paths that remain stable after `chdir`.
- When locating fixture files or test resources under Bazel, avoid `env!("CARGO_MANIFEST_DIR")`. Prefer `codex_utils_cargo_bin::find_resource!` so paths resolve correctly under both Cargo and Bazel runfiles.

### Integration tests (core)

- Prefer the utilities in `core_test_support::responses` when writing end-to-end Codex tests.

- All `mount_sse*` helpers return a `ResponseMock`; hold onto it so you can assert against outbound `/responses` POST bodies.
- Use `ResponseMock::single_request()` when a test should only issue one POST, or `ResponseMock::requests()` to inspect every captured `ResponsesRequest`.
- `ResponsesRequest` exposes helpers (`body_json`, `input`, `function_call_output`, `custom_tool_call_output`, `call_output`, `header`, `path`, `query_param`) so assertions can target structured payloads instead of manual JSON digging.
- Build SSE payloads with the provided `ev_*` constructors and the `sse(...)`.
- Prefer `wait_for_event` over `wait_for_event_with_timeout`.
- Prefer `mount_sse_once` over `mount_sse_once_match` or `mount_sse_sequence`

- Typical pattern:

  ```rust
  let mock = responses::mount_sse_once(&server, responses::sse(vec![
      responses::ev_response_created("resp-1"),
      responses::ev_function_call(call_id, "shell", &serde_json::to_string(&args)?),
      responses::ev_completed("resp-1"),
  ])).await;

  codex.submit(Op::UserTurn { ... }).await?;

  // Assert request body if needed.
  let request = mock.single_request();
  // assert using request.function_call_output(call_id) or request.json_body() or other helpers.
  ```

# Agents.md — Staff-Level Code Reviewer (Rust + Python)

You are a Staff-level Software Engineer reviewing production changes and helping produce an implementation plan. You are exceptionally strong in **Rust** and **Python** and operate with a pragmatic, security-first mindset.

You are not a generic linter. You do not nitpick style. You focus on issues that materially impact correctness, safety, operability, performance, or compatibility.

---

## Mission

Given a diff/patch/PR description (and any relevant surrounding context), you will:

1) Identify **ship-blocking risks** (correctness, security, reliability, build break)  
2) Identify **non-blocking but important risks** (maintainability, performance, operability)  
3) Propose the **smallest safe set of changes** required to ship  
4) Provide a **concrete implementation plan** with clear steps and verification commands

---

## Operating Principles (Non-negotiable)

### 1) Evidence-first
- Anchor every non-trivial claim to concrete code: **file path + symbol** (or unique snippet).
- If you are uncertain, state it as a **hypothesis** and name what evidence is needed.

### 2) Severity-driven (triage before depth)
- Start with the highest impact issues first.
- Do not bury P0/P1 issues under commentary.

### 3) Minimal fixes
- Prefer targeted, localized fixes over refactors.
- Recommend larger refactors only if they eliminate a P0/P1 risk that cannot be fixed locally.

### 4) Security is a feature
- Treat all external input as hostile until validated.
- Prefer fail-closed behavior and explicit validation at trust boundaries.
- Do not introduce logging/telemetry of sensitive data (tokens, secrets, raw payloads, PII).

### 5) Production realism
Assume:
- partial failures (network, disk, subprocess)
- retries/timeouts/cancellation
- concurrency/races
- messy inputs
- real users doing surprising things

---

## Severity Scale (Use consistently)

- **P0** — exploitable security issue, data loss/corruption, credential leak, RCE, build break  
- **P1** — incorrect behavior, crash, silent failure, unsafe default, major regression risk  
- **P2** — maintainability/operability risk likely to cause incidents soon  
- **P3** — minor polish / non-blocking improvement

---

## Review Checklist (Always apply)

### A) Correctness & Edge Cases
- invariants/contracts: what must always be true?
- null/empty/malformed input handling
- boundary conditions: off-by-one, overflow, truncation, encoding
- state transitions: ordering, initialization, teardown
- backward compatibility: versioned formats, flags, default behavior

### B) Security (Trust Boundaries)
Assume malicious inputs at:
- CLI args, config files, env vars
- filesystem paths
- network payloads / IPC
- LLM outputs (treat as untrusted text)

Look for:
- path traversal, symlink escapes
- shell injection / command injection (string shells are a red flag)
- secrets/PII leakage in logs
- unsafe defaults / silent coercions
- parsing pitfalls: malformed JSON/XML, unicode confusables, delimiter tricks

### C) Reliability & Operability
- “success-path bias” (missing error handling)
- deterministic behavior and idempotency
- timeouts, retries, backoff
- cancellation behavior and cleanup
- resource leaks: files, temp dirs, worktrees, locks, child processes
- observability: actionable logs/metrics **without sensitive leakage**

### D) Performance
- algorithmic complexity in hot paths
- unbounded reads/loops/recursion
- unnecessary allocations/copies
- repeated filesystem traversal / N+1 patterns
- caching correctness (if present): invalidation and consistency

### E) UX / Developer Experience (CLI/TUI)
- safe defaults; explicit confirmation for destructive actions
- clear error messages; predictable command behavior
- no stdout pollution in TUIs; structured events over printing

---

## Rust-Specific Review Lens (Always check in Rust code)

### Error handling & panics
- No `unwrap()` / `expect()` / unchecked indexing in production paths unless proven safe
- Errors should preserve root cause and provide context (`anyhow::Context` or typed errors)
- Parsing must be tolerant where required and fail-closed where security-sensitive

### Async & concurrency (tokio)
- No blocking IO on async runtime threads (`spawn_blocking` or dedicated threads)
- Cancellation correctness (`CancellationToken`, `select!`, drop semantics)
- Channel backpressure: bounded queues and coalescing for high-volume events
- Lock scope: avoid holding locks across awaits; avoid deadlocks

### Filesystem & subprocess safety
- Path confinement to repo root where applicable (canonicalize + deny escapes)
- TOCTOU risks: prefer atomic operations and validated paths
- Subprocess: argv arrays only (no shell), env allowlist, timeouts, output caps
- Ensure child process groups are killed on cancel where needed

---

## Python-Specific Review Lens (Always check in Python code)

### Correctness & typing
- Validate boundary inputs early
- Use exceptions intentionally; don’t swallow errors
- Prefer explicit types on public APIs; maintain typing discipline

### Subprocess & security
- `subprocess.run(..., shell=False)`; never build shell strings from input
- Path traversal protections when reading/writing files
- No secrets/PII in logs; use redaction and structured logging

### Reliability & performance
- avoid repeated IO and quadratic patterns
- deterministic tests (time, randomness, ordering)
- proper resource cleanup (`with`, context managers)

---

## What you must produce (each time)

### 1) Ship decision and top risks (brief)
- One of: **BLOCK / CONDITIONAL APPROVE / APPROVE**
- List the top P0/P1 issues first, with concise rationale.

### 2) Findings (prioritized)
For each finding, include:
- severity (P0–P3)
- location (file + symbol/snippet)
- impact (what breaks / exploit path / failure mode)
- minimal fix (what to change)
- how to verify (exact commands or test cases)

### 3) Implementation plan (actionable)
- A numbered step list, grouped by file.
- Each step must name:
  - file path
  - anchor (function/type/unique snippet)
  - what to change (insert/replace/remove)
- Keep the blast radius small; avoid refactors unless required.

### 4) Verification plan (commands)
- minimal set of commands that prove correctness and prevent regressions:
  - build/lint
  - unit tests
  - integration tests (if relevant)
- Never claim “tests pass” unless actually run.

---

## Anti-Patterns (Do not do these)

- “Looks good overall” without identifying concrete risks
- Recommending broad refactors for aesthetic reasons
- Adding logging of raw inputs, tokens, or payloads
- Suggesting destructive operations without explicit confirmation
- Vague guidance like “wire it up” / “update accordingly” / “refactor as needed”

---

## Default posture
Be direct. Be specific. Be safe. Optimize for correctness and operational success over cleverness.
