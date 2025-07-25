use std::path::PathBuf;

use askama::Template;
use rand::seq::IndexedRandom;

const SUCCESS_EMOJI: &[&str] = &[
    "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩",
    "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩",
    "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩",
    "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🟩", "🌿", "🥬",
];

const FAILURE_EMOJI: &[&str] = &[
    "🟥", "🟥", "🟥", "🟥", "🟥", "🟥", "🟥", "🟥", "🟥", "🟥", "🟥", "🟥", "🟥", "🟥", "🟥", "🟥", "🟥", "🟥", "🟥", "🟥",
    "🟥", "🟥", "🟥", "🟥", "🟥", "🟥", "🟥", "🟥", "🟥", "🟥", "🟥", "🟥", "🟥", "🟥", "🟥", "🟥", "🟥", "🟥", "🟥", "🟥",
    "🟥", "🟥", "🟥", "🟥", "🟥", "🟥", "🟥", "🟥", "🟥", "🟥", "🟥", "🟥", "🟥", "🟥", "🟥", "🟥", "🟥", "🟥", "🟥",
    "🟥", "🟥", "🟥", "🟥", "💔",
];

pub fn random_success_emoji() -> &'static str {
    SUCCESS_EMOJI.choose(&mut rand::rng()).unwrap_or(&"🟩")
}

pub fn random_failure_emoji() -> &'static str {
    FAILURE_EMOJI.choose(&mut rand::rng()).unwrap_or(&"🟥")
}

#[derive(Template)]
#[template(path = "filter_matched_no_files.md")]
pub struct FilterMatchedNoFiles {
    pub failure_emoji: &'static str,
    pub command: String,
}

#[derive(Template)]
#[template(path = "greeting.md")]
pub struct GreetingTemplate {}

#[derive(Template)]
#[template(path = "plan_error.md")]
pub struct PlanErrorTemplate {
    pub prefix: PathBuf,
    pub filename: String,
    pub failure_emoji: &'static str,
    pub error_message: String,
}

#[derive(Template)]
#[template(path = "plan_overall_error.md")]
pub struct PlanOverallErrorTemplate {
    pub failure_emoji: &'static str,
    pub error_message: String,
}

#[derive(Template)]
#[template(path = "plan_deferral_loop.md")]
pub struct PlanDeferralLoopTemplate {
    pub failure_emoji: &'static str,
    pub deferred_count: usize,
    pub output_keys: Vec<String>,
}

#[derive(Template)]
#[template(path = "plan_success.md")]
pub struct PlanSuccessTemplate {
    pub filename: String,
    pub success_emoji: &'static str,
    pub op_reports: Vec<(String, String)>,
}

#[derive(Template)]
#[template(path = "plan_overall_success.md")]
pub struct PlanOverallSuccessTemplate {
    pub success_emoji: &'static str,
    pub apply_command: String,
}

#[derive(Template)]
#[template(path = "plan_overall_success_with_deferrals.md")]
pub struct PlanOverallSuccessWithDeferralsTemplate {
    pub success_emoji: &'static str,
    pub apply_command: String,
    pub deferred_count: usize,
    pub output_keys: Vec<String>,
}

#[derive(Template)]
#[template(path = "plan_no_changes.md")]
pub struct PlanNoChangesTemplate {
    pub success_emoji: &'static str,
}

#[derive(Template)]
#[template(path = "apply_error.md")]
pub struct ApplyErrorTemplate {
    pub filename: String,
    pub error_message: String,
    pub failure_emoji: &'static str,
}

#[derive(Template)]
#[template(path = "apply_success.md")]
pub struct ApplySuccessTemplate {
    pub filename: String,
    pub op_output_descriptions: Vec<String>,
    pub success_emoji: &'static str,
}

#[derive(Template)]
#[template(path = "apply_overall_success.md")]
pub struct ApplyOverallSuccessTemplate {
    pub success_emoji: &'static str,
}

#[derive(Template)]
#[template(path = "apply_no_plan.md")]
pub struct ApplyNoPlanTemplate {}

#[derive(Template)]
#[template(path = "import_error.md")]
pub struct ImportErrorTemplate {
    pub error_message: String,
    pub failure_emoji: &'static str,
}

#[derive(Template)]
#[template(path = "import_success.md")]
pub struct ImportSuccessTemplate {
    pub paths: Vec<String>,
    pub imported_count: usize,
    pub total_count: usize,
    pub success_emoji: &'static str,
}

#[derive(Template)]
#[template(path = "skeleton_import_error.md")]
pub struct SkeletonImportErrorTemplate {
    pub error_message: String,
    pub failure_emoji: &'static str,
}

#[derive(Template)]
#[template(path = "skeleton_import_success.md")]
pub struct SkeletonImportSuccessTemplate {
    pub imported_count: usize,
    pub success_emoji: &'static str,
}

#[derive(Template)]
#[template(path = "migration_explain_error.md")]
pub struct ExplainErrorTemplate {
    pub filename: String,
    pub statement: String,
    pub error_message: String,
}

#[derive(Template)]
#[template(path = "command_parse_failure.md")]
pub struct CommandParseFailure {
    pub command: String,
    pub error_message: String,
    pub failure_emoji: &'static str,
}

#[derive(Template)]
#[template(path = "connector_stdout.md")]
pub struct ConnectorStdout {
    pub prefix: String,
    pub connector_name: String,
    pub filename: String,
    pub stdout: String,
}

#[derive(Template)]
#[template(path = "pr_lock_held.md")]
pub struct PrLockHeld {
    pub failure_emoji: &'static str,
}

#[derive(Template)]
#[template(path = "misc_error.md")]
pub struct MiscError {
    pub error_message: String,
    pub failure_emoji: &'static str,
}

#[derive(Template)]
#[template(path = "pull_state_clean.md")]
pub struct PullStateCleanTemplate {
    pub success_emoji: &'static str,
}

#[derive(Template)]
#[template(path = "pull_state_error.md")]
pub struct PullStateErrorTemplate {
    pub failure_emoji: &'static str,
    pub error_message: String,
}

#[derive(Template)]
#[template(path = "pull_state_success_with_deferrals.md")]
pub struct PullStateSuccessWithDeferralsTemplate {
    pub object_count: usize,
    pub import_count: usize,
    pub deferred_count: usize,
    pub success_emoji: &'static str,
    pub output_keys: Vec<String>,
}

#[derive(Template)]
#[template(path = "pull_state_success.md")]
pub struct PullStateSuccessTemplate {
    pub object_count: usize,
    pub import_count: usize,
    pub success_emoji: &'static str,
}
