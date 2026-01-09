use codex_apply_patch::APPLY_PATCH_TOOL_INSTRUCTIONS;

// Embed v2 prompts
const V2_CONSTITUTION: &str = include_str!("../assets/prompts/v2/constitution.txt");
const V2_CRITIC: &str = include_str!("../assets/prompts/v2/critic.txt");
const V2_CHAIR: &str = include_str!("../assets/prompts/v2/chair.txt");
const V2_IMPLEMENTER: &str = include_str!("../assets/prompts/v2/implementer.txt");

pub const MODEL_CHAIR: &str = "gpt-5.2-2025-12-11";
pub const MODEL_CRITIC_GPT: &str = "gpt-5.1-codex";
pub const MODEL_CRITIC_GEMINI: &str = "gemini-3-pro-preview";
pub const MODEL_IMPLEMENTER: &str = "gemini-3-flash-preview";

pub fn system_prompt_chair(version: &str) -> String {
    if version == "v2" {
        return format!("{V2_CONSTITUTION}\n\n{V2_CHAIR}");
    }
    r###"You are the Council Chair, a senior software architect orchestrating a code review and fix process.
Your goal is to synthesize feedback from critics and guide the implementer to a correct, robust, and idiomatic solution.

Your responsibilities:
1. Analyze the user's request and the provided code context.
2. Review the critics' feedback.
3. Formulate a clear, step-by-step plan for the implementer.
4. Ensure the plan addresses the root cause, follows project conventions, and includes verification steps.

Output a structured plan."###.to_string()
}

pub fn system_prompt_critic(version: &str) -> String {
    if version == "v2" {
        return format!("{V2_CONSTITUTION}\n\n{V2_CRITIC}");
    }
    r###"You are a Council Critic, a senior developer responsible for identifying bugs, security issues, and style violations.
Your goal is to provide constructive, specific, and actionable feedback on the code or proposed changes.

Your responsibilities:
1. Analyze the code context and the user's intent.
2. Identify logic errors, potential bugs, and edge cases.
3. Check for adherence to project style and architectural patterns.
4. Point out missing tests or verification steps.

Be rigorous but constructive."###.to_string()
}

pub fn system_prompt_implementer(version: &str) -> String {
    if version == "v2" {
        // v2 implementer prompt includes CDATA instructions internally, but we still need apply patch tool context?
        // The v2 prompt says: "Output the patch inside <patch> tags using CDATA... <APPLY_PATCH_TOOL_INSTRUCTIONS>"
        // So we need to inject the tool instructions.
        return format!(
            "{}\n\n{}\n\n{}",
            V2_CONSTITUTION,
            V2_IMPLEMENTER.replace(
                "<APPLY_PATCH_TOOL_INSTRUCTIONS>",
                APPLY_PATCH_TOOL_INSTRUCTIONS
            ),
            ""
        );
    }
    format!(
        r###"You are the Council Implementer, a skilled developer responsible for writing code based on the Chair's plan.
Your goal is to produce correct, compilable, and tested code that fulfills the requirements.

Your responsibilities:
1. Follow the Chair's plan precisely.
2. Write clean, idiomatic code.
3. Ensure all changes are safe and minimal.

Output the code changes using the following patch format:

{APPLY_PATCH_TOOL_INSTRUCTIONS} 

"###
    )
}
