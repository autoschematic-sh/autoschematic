use askama::Template;
use autoschematic_core::unescape::try_unescape;
use clap::Parser;
use octocrab::{
    models::webhook_events::{
        WebhookEvent, WebhookEventPayload,
        payload::{IssueCommentWebhookEventAction, PullRequestWebhookEventAction},
    },
    params::checks::{CheckRunConclusion, CheckRunStatus},
};
use std::path::PathBuf;

use crate::{
    DOMAIN,
    changeset::{ChangeSet, util::create_comment_standalone},
    command::{AutoschematicCommand, HELP},
    error::{AutoschematicServerError, AutoschematicServerErrorType},
    template::{
        self, ApplyErrorTemplate, ApplyNoPlanTemplate, ApplySuccessTemplate, CommandParseFailure, GreetingTemplate,
        ImportErrorTemplate, ImportSuccessTemplate, PlanErrorTemplate, PlanNoChangesTemplate, PlanOverallErrorTemplate,
        PlanOverallSuccessTemplate, PlanSuccessTemplate, PrLockHeld, SkeletonImportErrorTemplate,
        SkeletonImportSuccessTemplate, random_failure_emoji,
    },
};
use crate::{TASK_REGISTRY, template::PullStateSuccessTemplate};
use crate::{
    task::util::message_from_github_webhook,
    template::{
        ApplyOverallSuccessTemplate, FilterMatchedNoFiles, PlanDeferralLoopTemplate, PlanOverallSuccessWithDeferralsTemplate,
        PullStateCleanTemplate, PullStateErrorTemplate, PullStateSuccessWithDeferralsTemplate,
    },
};

/// Dispatches incoming GitHub webhook events to appropriate handlers
///
/// This is the main entry point for webhook event processing. It determines the
/// event type and routes to specific handlers based on the payload content.
///
/// # Arguments
/// * `webhook_event` - The parsed webhook event from GitHub
///
/// # Returns
/// * `Result<(), AutoschematicError>` - Success or error from event processing
pub async fn dispatch(webhook_event: WebhookEvent) -> Result<(), AutoschematicServerError> {
    tracing::debug!("Dispatching webhook event: {:?}", webhook_event.specific);

    if let Ok(Some(task_message)) = message_from_github_webhook(&webhook_event) {
        if let Some(registry) = TASK_REGISTRY.get() {
            let entries = &*registry.entries.read().await;

            for (key, entry) in entries.into_iter() {
                tracing::error!("webhook sending message {:?}", &task_message);
                let _ = entry.outbox.send(task_message.clone()).await;
            }
        };
    }

    match webhook_event.specific {
        WebhookEventPayload::IssueComment(ref payload)
            if payload.action == IssueCommentWebhookEventAction::Created
                || payload.action == IssueCommentWebhookEventAction::Edited =>
        {
            let comment_username = payload.comment.user.login.clone();
            let comment_id = payload.comment.id;
            let Some(ref comment_body) = payload.comment.body else {
                return Ok(());
            };

            let comment_url = &payload.comment.html_url;

            let args: Vec<&str> = comment_body.split(" ").collect();
            let cmd = AutoschematicCommand::try_parse_from(&args);

            let Some(&arg0) = args.first() else {
                return Ok(());
            };

            if arg0 != "autoschematic" {
                return Ok(());
            }

            match cmd {
                Ok(cmd) => {
                    let changeset_res = ChangeSet::from_webhook(&webhook_event).await;

                    let changeset = match changeset_res {
                        Ok(changeset) => changeset,
                        Err(e) => {
                            return Err(AutoschematicServerError {
                                kind: AutoschematicServerErrorType::InternalError(e.into()),
                            });
                        }
                    };

                    let mut changeset = match changeset.try_lock() {
                        Ok(changeset) => changeset,
                        Err(e) => {
                            let error_template = PrLockHeld {
                                failure_emoji: template::random_failure_emoji(),
                            };
                            create_comment_standalone(&webhook_event, &error_template.render()?).await?;
                            return Err(e.into());
                        }
                    };

                    let _ = changeset
                        .add_reaction(comment_id, octocrab::models::reactions::ReactionContent::Rocket)
                        .await;

                    let check_run_url = match DOMAIN.get() {
                        Some(domain) => format!("https://{}", domain),
                        None => String::new(),
                    };

                    match cmd.command {
                        crate::command::AutoschematicSubcommand::Import {
                            prefix,
                            overwrite,
                            connector,
                            subpath,
                        } => {
                            let subpath = subpath.map(PathBuf::from);
                            let connector_check_run_id = changeset
                                .create_check_run(None, comment_body, &check_run_url, CheckRunStatus::InProgress, None)
                                .await?;

                            let res = changeset
                                .import_all(subpath, prefix, connector, &comment_username, comment_url.as_str(), overwrite)
                                .await;

                            match res {
                                Ok((imported_count, total_count)) => {
                                    changeset
                                        .create_check_run(
                                            Some(connector_check_run_id),
                                            comment_body,
                                            &check_run_url,
                                            CheckRunStatus::Completed,
                                            Some(CheckRunConclusion::Success),
                                        )
                                        .await?;

                                    let import_success_template = ImportSuccessTemplate {
                                        imported_count,
                                        total_count,
                                        paths: Vec::new(),
                                        success_emoji: template::random_success_emoji(),
                                    };

                                    changeset.create_comment(&import_success_template.render()?).await?;
                                }
                                Err(e) => {
                                    tracing::error!("{}", e);
                                    changeset
                                        .create_check_run(
                                            Some(connector_check_run_id),
                                            comment_body,
                                            &check_run_url,
                                            CheckRunStatus::Completed,
                                            Some(CheckRunConclusion::Failure),
                                        )
                                        .await?;

                                    let import_error_template = ImportErrorTemplate {
                                        error_message: try_unescape(&format!("{:#}", e)).to_string(),
                                        failure_emoji: template::random_failure_emoji(),
                                    };

                                    changeset.create_comment(&import_error_template.render()?).await?;
                                }
                            }
                        }
                        crate::command::AutoschematicSubcommand::Plan {
                            prefix,
                            connector,
                            subpath,
                        } => {
                            let subpath = subpath.map(PathBuf::from);
                            let plan_check_run_id = changeset
                                .create_check_run(None, comment_body, &check_run_url, CheckRunStatus::InProgress, None)
                                .await?;

                            let repo = changeset.clone_repo().await?;

                            let res = changeset
                                .plan(
                                    &repo,
                                    subpath.clone(),
                                    &prefix,
                                    &connector,
                                    &comment_username,
                                    comment_url.as_str(),
                                    false,
                                )
                                .await;
                            match res {
                                Ok(_) => {
                                    // Flag to set CheckRun success/failure at the end
                                    let mut overall_success = true;
                                    let mut all_plans_empty = true;

                                    if let Some(plan_report_set) = &changeset.last_plan {
                                        for plan_report in &plan_report_set.plan_reports {
                                            if let Some(error) = &plan_report.error {
                                                overall_success = false;

                                                let plan_error_template = PlanErrorTemplate {
                                                    prefix: plan_report.prefix.clone(),
                                                    error_message: try_unescape(&format!("{:#}", error)).to_string(),
                                                    failure_emoji: template::random_failure_emoji(),
                                                    filename: plan_report.virt_addr.to_string_lossy().to_string(),
                                                };

                                                changeset.create_comment(&plan_error_template.render()?).await?;
                                            } else {
                                                let mut op_reports = Vec::new();
                                                for op in &plan_report.connector_ops {
                                                    if let Some(msg) = &op.friendly_message {
                                                        op_reports.push((msg.to_string(), op.op_definition.to_string()));
                                                    } else {
                                                        op_reports
                                                            .push((String::from("Op Summary"), op.op_definition.to_string()));
                                                    }
                                                }
                                                // Skip empty plan reports :)
                                                if !op_reports.is_empty() {
                                                    all_plans_empty = false;
                                                    let plan_success_template = PlanSuccessTemplate {
                                                        success_emoji: template::random_success_emoji(),
                                                        filename: String::from(plan_report.virt_addr.to_string_lossy()),
                                                        op_reports,
                                                    };

                                                    changeset.create_comment(&plan_success_template.render()?).await?;
                                                }
                                            }
                                        }

                                        if overall_success {
                                            if all_plans_empty {
                                                if plan_report_set.deferred_count > 0 {
                                                    let msg = PlanDeferralLoopTemplate {
                                                        failure_emoji: template::random_failure_emoji(),
                                                        deferred_count: plan_report_set.deferred_count,
                                                        output_keys: plan_report_set
                                                            .deferred_pending_outputs
                                                            .iter()
                                                            .map(|o| o.to_string())
                                                            .collect(),
                                                    }
                                                    .render()?;

                                                    changeset.create_comment(&msg).await?;
                                                } else {
                                                    let msg = PlanNoChangesTemplate {
                                                        success_emoji: template::random_success_emoji(),
                                                    }
                                                    .render()?;

                                                    changeset.create_comment(&msg).await?;
                                                }
                                            } else {
                                                let apply_command = match (&subpath, &connector) {
                                                    (None, None) => String::from("autoschematic apply"),
                                                    (None, Some(connector)) => {
                                                        format!("autoschematic apply -c {}", &connector)
                                                    }
                                                    (Some(subpath), None) => {
                                                        format!("autoschematic apply -p {:?}", subpath)
                                                    }
                                                    (Some(subpath), Some(connector)) => {
                                                        format!("autoschematic apply -c {} -p {:?}", connector, subpath)
                                                    }
                                                };

                                                if plan_report_set.deferred_count > 0 {
                                                    let msg = PlanOverallSuccessWithDeferralsTemplate {
                                                        success_emoji: template::random_success_emoji(),
                                                        apply_command,
                                                        deferred_count: plan_report_set.deferred_count,
                                                        output_keys: plan_report_set
                                                            .deferred_pending_outputs
                                                            .iter()
                                                            .map(|o| o.to_string())
                                                            .collect(),
                                                    }
                                                    .render()?;

                                                    changeset.create_comment(&msg).await?;
                                                } else {
                                                    let plan_overall_success_template = PlanOverallSuccessTemplate {
                                                        success_emoji: template::random_success_emoji(),
                                                        apply_command,
                                                    };

                                                    changeset.create_comment(&plan_overall_success_template.render()?).await?;
                                                }
                                            }
                                            let _plan_check_run_id = changeset
                                                .create_check_run(
                                                    Some(plan_check_run_id),
                                                    comment_body,
                                                    &check_run_url,
                                                    CheckRunStatus::Completed,
                                                    Some(CheckRunConclusion::Success),
                                                )
                                                .await?;
                                        } else {
                                            let _plan_check_run_id = changeset
                                                .create_check_run(
                                                    Some(plan_check_run_id),
                                                    comment_body,
                                                    &check_run_url,
                                                    CheckRunStatus::Completed,
                                                    Some(CheckRunConclusion::Failure),
                                                )
                                                .await?;
                                        }
                                    }
                                }

                                Err(e) => {
                                    tracing::error!("{}", e);
                                    let plan_error_template = PlanOverallErrorTemplate {
                                        error_message: try_unescape(&format!("{:#}", e)).to_string(),
                                        failure_emoji: template::random_failure_emoji(),
                                    };

                                    changeset.create_comment(&plan_error_template.render()?).await?;

                                    let _plan_check_run_id = changeset
                                        .create_check_run(
                                            Some(plan_check_run_id),
                                            comment_body,
                                            &check_run_url,
                                            CheckRunStatus::Completed,
                                            Some(CheckRunConclusion::Failure),
                                        )
                                        .await?;
                                }
                            }
                        }
                        crate::command::AutoschematicSubcommand::Apply {
                            prefix,
                            connector,
                            subpath,
                        } => {
                            let subpath = subpath.map(PathBuf::from);

                            let repo = changeset.clone_repo().await?;

                            // let plan_res = changeset
                            //     .plan(&repo, subpath.clone(), connector.clone(), false)
                            //     .await;

                            let mut overall_success = true;
                            match &changeset.last_plan {
                                Some(_) => {
                                    let apply_report_set = changeset
                                        .apply(&repo, subpath, connector, &comment_username, comment_url.as_str())
                                        .await?;

                                    for apply_report in apply_report_set.apply_reports {
                                        if apply_report.error.is_none() {
                                            let mut op_output_descriptions = Vec::new();
                                            for op in apply_report.outputs {
                                                if let Some(friendly_message) = op.friendly_message {
                                                    op_output_descriptions.push(friendly_message);
                                                }
                                            }
                                            let apply_success_template = ApplySuccessTemplate {
                                                success_emoji: template::random_success_emoji(),
                                                filename: String::from(apply_report.virt_addr.to_string_lossy()),
                                                op_output_descriptions,
                                            };

                                            changeset.create_comment(&apply_success_template.render()?).await?;
                                        } else {
                                            let apply_error_template = if let Some(error) = apply_report.error {
                                                overall_success = false;
                                                ApplyErrorTemplate {
                                                    error_message: try_unescape(&format!("{:#}", error)).to_string(),
                                                    failure_emoji: template::random_failure_emoji(),
                                                    filename: String::from(apply_report.virt_addr.to_string_lossy()),
                                                }
                                            } else {
                                                ApplyErrorTemplate {
                                                    error_message: try_unescape("").to_string(),
                                                    failure_emoji: template::random_failure_emoji(),
                                                    filename: String::from(apply_report.virt_addr.to_string_lossy()),
                                                }
                                            };

                                            changeset.create_comment(&apply_error_template.render()?).await?;
                                        }
                                    }
                                    if overall_success {
                                        let msg = ApplyOverallSuccessTemplate {
                                            success_emoji: template::random_success_emoji(),
                                        }
                                        .render()?;

                                        changeset.create_comment(&msg).await?;
                                    }
                                }
                                None => {
                                    let apply_no_plan_template = ApplyNoPlanTemplate {};

                                    changeset.create_comment(&apply_no_plan_template.render()?).await?;
                                }
                            }
                        }
                        crate::command::AutoschematicSubcommand::PullState {
                            prefix,
                            connector,
                            subpath,
                            delete,
                        } => {
                            let subpath = subpath.map(PathBuf::from);
                            let connector_check_run_id = changeset
                                .create_check_run(None, comment_body, &check_run_url, CheckRunStatus::InProgress, None)
                                .await?;

                            let repo = changeset.clone_repo().await?;

                            let res = changeset
                                .pull_state(
                                    &repo,
                                    subpath,
                                    prefix,
                                    connector,
                                    &comment_username,
                                    comment_url.as_str(),
                                    delete,
                                )
                                .await;

                            match res {
                                Ok(pull_state_report) => {
                                    changeset
                                        .create_check_run(
                                            Some(connector_check_run_id),
                                            comment_body,
                                            &check_run_url,
                                            CheckRunStatus::Completed,
                                            Some(CheckRunConclusion::Success),
                                        )
                                        .await?;

                                    let msg = if pull_state_report.deferred_count > 0 {
                                        PullStateSuccessWithDeferralsTemplate {
                                            object_count: pull_state_report.object_count,
                                            import_count: pull_state_report.import_count,
                                            deferred_count: pull_state_report.deferred_count,
                                            success_emoji: template::random_success_emoji(),
                                            output_keys: pull_state_report
                                                .missing_outputs
                                                .iter()
                                                .map(|o| o.to_string())
                                                .collect(),
                                        }
                                        .render()?
                                    } else if pull_state_report.import_count > 0 {
                                        PullStateSuccessTemplate {
                                            object_count: pull_state_report.object_count,
                                            import_count: pull_state_report.import_count,
                                            success_emoji: template::random_success_emoji(),
                                        }
                                        .render()?
                                    } else if pull_state_report.object_count > 0 {
                                        PullStateCleanTemplate {
                                            success_emoji: template::random_success_emoji(),
                                        }
                                        .render()?
                                    } else {
                                        FilterMatchedNoFiles {
                                            command: comment_body.clone(),
                                            failure_emoji: template::random_failure_emoji(),
                                        }
                                        .render()?
                                    };

                                    changeset.create_comment(&msg).await?;
                                }
                                Err(e) => {
                                    tracing::error!("{}", e);
                                    changeset
                                        .create_check_run(
                                            Some(connector_check_run_id),
                                            comment_body,
                                            &check_run_url,
                                            CheckRunStatus::Completed,
                                            Some(CheckRunConclusion::Failure),
                                        )
                                        .await?;

                                    let msg = PullStateErrorTemplate {
                                        error_message: try_unescape(&format!("{:#}", e)).to_string(),
                                        failure_emoji: template::random_failure_emoji(),
                                    }
                                    .render()?;

                                    changeset.create_comment(&msg).await?;
                                }
                            }
                        }
                        crate::command::AutoschematicSubcommand::ImportSkeletons {
                            prefix,
                            connector,
                            subpath,
                        } => {
                            let subpath = subpath.map(PathBuf::from);

                            let connector_check_run_id = changeset
                                .create_check_run(None, comment_body, &check_run_url, CheckRunStatus::InProgress, None)
                                .await?;

                            let res = changeset
                                .import_skeletons(subpath, connector, &comment_username, comment_url.as_str())
                                .await;

                            match res {
                                Ok(imported_count) => {
                                    changeset
                                        .create_check_run(
                                            Some(connector_check_run_id),
                                            comment_body,
                                            &check_run_url,
                                            CheckRunStatus::Completed,
                                            Some(CheckRunConclusion::Success),
                                        )
                                        .await?;

                                    let import_success_template = SkeletonImportSuccessTemplate {
                                        imported_count,
                                        success_emoji: template::random_success_emoji(),
                                    };

                                    changeset.create_comment(&import_success_template.render()?).await?;
                                }
                                Err(e) => {
                                    tracing::error!("{}", e);
                                    changeset
                                        .create_check_run(
                                            Some(connector_check_run_id),
                                            comment_body,
                                            &check_run_url,
                                            CheckRunStatus::Completed,
                                            Some(CheckRunConclusion::Failure),
                                        )
                                        .await?;

                                    let import_error_template = SkeletonImportErrorTemplate {
                                        error_message: try_unescape(&format!("{:#}", e)).to_string(),
                                        failure_emoji: template::random_failure_emoji(),
                                    };

                                    changeset.create_comment(&import_error_template.render()?).await?;
                                }
                            }
                        }
                        crate::command::AutoschematicSubcommand::Safety { off: _ } => {
                            tracing::warn!("Safety command not yet implemented");
                            changeset.create_comment("⚠️ Safety command not yet implemented").await?;
                        }
                        crate::command::AutoschematicSubcommand::Help {} => {
                            changeset.create_comment(HELP).await?;
                        }
                    }
                }
                Err(e) => {
                    if arg0 == "autoschematic" {
                        let parse_failure = CommandParseFailure {
                            command: comment_body.clone(),
                            error_message: format!("{:#}", e),
                            failure_emoji: random_failure_emoji(),
                        };
                        create_comment_standalone(&webhook_event, &parse_failure.render()?).await?;
                    }
                    // Don't complain here. The comment may just be... a regular comment.
                }
            }
        }
        WebhookEventPayload::PullRequest(ref payload) => {
            tracing::info!("Got PullRequest.{:?} event", payload.action);
            if payload.action == PullRequestWebhookEventAction::Opened {
                let greeting_template = GreetingTemplate {};
                create_comment_standalone(&webhook_event, &greeting_template.render()?).await?;
            }
        }
        _ => (),
    };

    Ok(())
}
