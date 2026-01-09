📋 Final Master Directive: Council v2 (Rust Implementation)
Role: You are a Senior Rust Systems Engineer. Context: We are upgrading the codex-council crate to use a new Tag-Delimited Output Schema (XML-like tags, but tolerant parsing). Objective: Implement robust parsing for LLM outputs and wire up the new prompts without breaking existing v1 functionality.

🛠️ Engineering Standards (Strict Compliance)
No Panics: Do not use .unwrap() or .expect() on parsing logic. Use Result or Option.

Tolerant Parsing: LLMs output preamble text. Your parser must ignore junk outside the tags and find the specific blocks we need.

Type Safety: Map string severities ("P0", "P1") to a strict Rust enum Severity immediately.

No Placeholders: All functions must be fully implemented. Do not leave TODO, unimplemented!(), or empty returns.

Localization: All logic changes must live in codex-council or codex-core.

📅 Phase 1: Robust Infrastructure (The Parser)
[x] Task 1: Create codex-council/src/parsing.rs

Goal: Create a standalone module for robust text extraction.

Detail: Implement the module using Resource 1 (below).

Critical: Ensure extract_patch robustly handles <![CDATA[ wrappers.

Critical: Ensure Severity is an enum, not a string.

[x] Task 2: Expose and Test

Action: Add pub mod parsing; to lib.rs.

Action: Create tests/fixtures/v2_response.xml containing messy preamble + tags + CDATA.

Verification: Write a unit test that loads this fixture and asserts that extract_patch returns clean code and extract_findings returns the correct Enum variants.

📝 Phase 2: Asset Management (The Prompts)
[x] Task 3: Install v2 Prompts

Action: Create assets/prompts/v2/.

Action: Create constitution.txt, critic.txt, chair.txt, implementer.txt using Resource 2.

⚙️ Phase 3: Logic Wiring
[x] Task 4: Update Configuration

Action: In codex-core/src/config.rs, add prompt_version: String (default "v1").

[x] Task 5: Implement Prompt Loader

Logic: If version == "v2", load from v2/ and prepend constitution.txt to the System Prompt of every role.

Fallback: If v2 assets are missing, return a clear Result::Err (do not silently fallback to v1).

🔗 Phase 4: Integration
[x] Task 6: Connect the Critic

Action: Call parsing::extract_findings.

Logic: If findings exist, prevent auto-approval.

[x] Task 7: Connect the Chair

Action: Call parsing::extract_plan.

Logic: If None is returned, treat as a "Parsing Error" and prompt the user to retry.

[x] Task 8: Connect the Implementer

Action: Call parsing::extract_patch.

Safety: If None, return "Parsing Error." Do not attempt to apply an empty patch.

🧹 Phase 5: Refinement
[x] Task 9: Preserve Whitespace in Patch
    - Update `unwrap_cdata` to preserve internal whitespace.
    - Do not trim the result unless it's the wrapper itself.

[x] Task 10: Enforce Patch Markers in Prompt
    - Update `implementer.txt` to explicitly require `*** Begin Patch` and `*** End Patch`.
    - Remove `<APPLY_PATCH_TOOL_INSTRUCTIONS>` from the output template to prevent hallucination.

[x] Task 11: Validate Patch Before Apply
    - Implement `looks_like_apply_patch` in `parsing.rs`.
    - Check for markers and absolute paths.
    - Integrate into `run_fix`.

[x] Task 12: Robust Attribute Parsing
    - Replace whitespace splitting in `parse_attrs` with a proper state machine scanner.
