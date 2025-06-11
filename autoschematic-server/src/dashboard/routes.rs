use std::{collections::HashSet, path::PathBuf};

use actix_session::Session;
use actix_web::{HttpRequest, HttpResponse, web};
use actix_ws::{AggregatedMessage, Closed};
use futures::StreamExt;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    TASK_REGISTRY, TRACESTORE,
    error::{self, AutoschematicServerError, AutoschematicServerErrorType},
    task::{
        message::TaskRegistryMessage, registry::TaskRegistryKey, state::TaskState, subscribe_task_state,
        try_send_task_registry_message,
    },
    tracestore::{RepoKey, RunKey},
};

use super::{
    TEMPLATES,
    api_util::{get_config_for_repo, get_installations_for_user, get_pr_info, has_valid_session, is_repo_collaborator},
};

pub async fn dashboard(session: Session, param: web::Path<(String, String, u64)>) -> Result<HttpResponse, actix_web::Error> {
    if let Some((access_token, github_username)) = has_valid_session(&session).await? {
        let (owner, repo, pr) = param.into_inner();

        if is_repo_collaborator(&access_token, &github_username, &owner, &repo).await? {
            let Some(trace_store) = TRACESTORE.get() else {
                return Err(AutoschematicServerError {
                    kind: AutoschematicServerErrorType::ConfigurationError {
                        name: "TRACESTORE".to_string(),
                        message: "No tracestore configured".to_string(),
                    },
                }
                .into());
            };

            let mut context = tera::Context::new();

            context.insert("owner", &owner);
            context.insert("repo", &repo);
            context.insert("pr", &pr);

            let repo_key = RepoKey {
                owner: owner.clone(),
                repo: repo.clone(),
            };
            // let run_key = RunKey{ pr: pr, run_id: Uuid::new_v4()};

            let run_keys = trace_store
                .list_runs_for_pr(&repo_key, pr)
                .await
                .map_err(|e| error::AutoschematicServerError {
                    kind: AutoschematicServerErrorType::InternalError(e),
                })?;

            let mut runs = IndexMap::new();

            for run_key in run_keys {
                let run = trace_store
                    .get_run(&repo_key, &run_key)
                    .await
                    .map_err(|e| error::AutoschematicServerError {
                        kind: AutoschematicServerErrorType::InternalError(e),
                    })?;
                runs.insert(run_key.run_id, run);
            }

            context.insert("runs", &runs);

            // TEMPLATES.lock().unwrap().full_reload().unwrap();

            let Some(templates) = TEMPLATES.get() else {
                return Err(AutoschematicServerError {
                    kind: AutoschematicServerErrorType::ConfigurationError {
                        name: "TEMPLATES".to_string(),
                        message: "No templates found".to_string(),
                    },
                }
                .into());
            };
            let rendered = templates
                .render("dashboard.html", &context)
                .map_err(|e| error::AutoschematicServerError {
                    kind: AutoschematicServerErrorType::InternalError(e.into()),
                })?;
            Ok(HttpResponse::Ok().content_type("text/html").body(rendered))
        } else {
            Ok(HttpResponse::NotFound().finish())
        }
    } else {
        Ok(HttpResponse::Unauthorized().finish())
        // // User not authenticated, redirect to login
        // Ok(HttpResponse::Found()
        //     .append_header(("Location", "/api/login"))
        //     .finish())
    }
}

pub async fn install_list(session: Session) -> Result<HttpResponse, actix_web::Error> {
    let Some((access_token, github_username)) = has_valid_session(&session).await? else {
        return Ok(HttpResponse::Unauthorized().finish());
        // return Ok(HttpResponse::Found()
        //     .append_header(("Location", "/api/login"))
        //     .finish());
    };
    // User is authenticated, render dashboard
    // let mut context = tera::Context::new();

    // let jwt = create_jwt().map_err(|e| {
    //     tracing::error!("Failed to create jwt: {}", e);
    //     AutoschematicError::from(e)
    // })?;

    let installs = get_installations_for_user(&access_token).await.map_err(|e| {
        tracing::error!("Failed to get installations: {}", e);
        AutoschematicServerError::from(e)
    })?;

    // let mut allowed_installs = Vec::new();

    // for install in installs {
    //     if is_repo_collaborator(
    //         &access_token,
    //         &github_username,
    //         &install.owner,
    //         &install.repo,
    //     )
    //     .await?
    //     {
    //         allowed_installs.push(install);
    //     }
    // }

    // context.insert("installs", &installs);

    // let Some(templates) = TEMPLATES.get() else {
    //     return Err(AutoschematicError {
    //         kind: AutoschematicErrorType::ConfigurationError {
    //             name: "TEMPLATES".to_string(),
    //             message: "No templates found".to_string(),
    //         },
    //     }
    //     .into());
    // };
    // let rendered = templates
    //     .render("dashboard_list.html", &context)
    //     .map_err(|e| error::AutoschematicError {
    //         kind: AutoschematicErrorType::InternalError(e.into()),
    //     })?;
    Ok(HttpResponse::Ok().content_type("application/json").json(installs))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskListing {
    pub name: String,
    pub state: TaskState,
}
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct PrefixListing {
    name: String,
    tasks: Vec<TaskListing>,
}

pub async fn repo_view(session: Session, param: web::Path<(String, String, u64)>) -> Result<HttpResponse, actix_web::Error> {
    let (owner, repo, installation_id) = param.into_inner();

    if let Some((access_token, github_username)) = has_valid_session(&session).await? {
        let mut prefix_listings = Vec::new();

        // Uses the user's access token to fetch autoschematic.ron from the tip of the repo.
        if let Ok(Some(config)) = get_config_for_repo(&owner, &repo, &access_token).await {
            // context.insert("config_content", &config);
            for (prefix_name, prefix) in config.prefixes {
                let mut prefix_listing = PrefixListing {
                    name: prefix_name.clone(),
                    tasks: Vec::new(),
                };

                for task in prefix.tasks {
                    let registry_key = TaskRegistryKey {
                        owner: owner.clone(),
                        repo: repo.clone(),
                        prefix: PathBuf::from(prefix_name.clone()),
                        task_name: task.name.clone(),
                    };

                    let Some(registry) = TASK_REGISTRY.get() else {
                        continue;
                    };

                    let registry = registry.entries.read().await;

                    if let Some(registry_entry) = registry.get(&registry_key) {
                        prefix_listing.tasks.push(TaskListing {
                            name: task.name,
                            state: registry_entry.state.clone(),
                        });
                    } else {
                        prefix_listing.tasks.push(TaskListing {
                            name: task.name,
                            state: TaskState::Stopped,
                        });
                    }
                }

                prefix_listings.push(prefix_listing);
            }
        } ;
        Ok(HttpResponse::Ok().content_type("application/json").json(prefix_listings))
    } else {
        Ok(HttpResponse::Unauthorized().finish())
    }
}

pub async fn dashboard_list(session: Session) -> Result<HttpResponse, actix_web::Error> {
    if let Some((access_token, github_username)) = has_valid_session(&session).await? {
        let Some(trace_store) = TRACESTORE.get() else {
            return Err(AutoschematicServerError {
                kind: AutoschematicServerErrorType::ConfigurationError {
                    name: "TRACESTORE".to_string(),
                    message: "No tracestore configured".to_string(),
                },
            }
            .into());
        };
        // User is authenticated, render dashboard
        let mut context = tera::Context::new();

        let repos = trace_store.list_repos().await.map_err(|e| error::AutoschematicServerError {
            kind: AutoschematicServerErrorType::InternalError(e),
        })?;

        let mut allowed_repos = Vec::new();
        for repo in repos {
            if is_repo_collaborator(&access_token, &github_username, &repo.owner, &repo.repo).await? {
                let mut pr_ids: HashSet<u64> = HashSet::new();
                let mut prs = Vec::new();
                let runs = trace_store
                    .list_runs(&repo)
                    .await
                    .map_err(|e| error::AutoschematicServerError {
                        kind: AutoschematicServerErrorType::InternalError(e),
                    })?;

                for run in runs {
                    if !pr_ids.contains(&run.pr) {
                        let pr_info = get_pr_info(&access_token, &repo.owner, &repo.repo, run.pr).await?;
                        prs.push((run.pr, pr_info));
                        pr_ids.insert(run.pr);
                    }
                }
                allowed_repos.push((repo, prs));
            }
        }

        context.insert("allowed_repos", &allowed_repos);

        // TEMPLATES.lock().unwrap().full_reload().unwrap();

        let Some(templates) = TEMPLATES.get() else {
            return Err(AutoschematicServerError {
                kind: AutoschematicServerErrorType::ConfigurationError {
                    name: "TEMPLATES".to_string(),
                    message: "No templates found".to_string(),
                },
            }
            .into());
        };
        let rendered = templates
            .render("dashboard_list.html", &context)
            .map_err(|e| error::AutoschematicServerError {
                kind: AutoschematicServerErrorType::InternalError(e.into()),
            })?;
        Ok(HttpResponse::Ok().content_type("text/html").body(rendered))
    } else {
        Ok(HttpResponse::Unauthorized().finish())
        // User not authenticated, redirect to login
        // Ok(HttpResponse::Found()
        //     .append_header(("Location", "/api/login"))
        //     .finish())
    }
}

async fn repo_runs_subscribe(req: HttpRequest, stream: web::Payload) -> Result<HttpResponse, actix_web::Error> {
    let (res, session, stream) = actix_ws::handle(&req, stream)?;
    let stream = stream
        .aggregate_continuations()
        // aggregate continuation frames up to 1MiB
        .max_continuation_size(2_usize.pow(20));

    // start task but don't wait for it
    actix_web::rt::spawn(async move {});

    Ok(res)
}

pub async fn log_subscribe(
    req: HttpRequest,
    session: Session,
    stream: web::Payload,
    param: web::Path<(String, String, u64, Uuid)>,
) -> Result<HttpResponse, actix_web::Error> {
    let (res, mut ws_session, stream) = actix_ws::handle(&req, stream)?;

    if let Some((access_token, github_username)) = has_valid_session(&session).await? {
        let (owner, repo, pr, run_id) = param.into_inner();

        tracing::warn!("is_repo_collaborator...");
        if is_repo_collaborator(&access_token, &github_username, &owner, &repo).await? {
            tracing::warn!("is_repo_collaborator!");
            let Some(trace_store) = TRACESTORE.get() else {
                return Err(AutoschematicServerError {
                    kind: AutoschematicServerErrorType::ConfigurationError {
                        name: "TRACESTORE".to_string(),
                        message: "No tracestore configured".to_string(),
                    },
                }
                .into());
            };

            let repo_key = RepoKey { owner, repo };

            let run_key = RunKey { pr, run_id };

            let logs = {
                let run = trace_store
                    .get_run(&repo_key, &run_key)
                    .await
                    .map_err(|e| error::AutoschematicServerError {
                        kind: AutoschematicServerErrorType::InternalError(e),
                    })?;
                run.logs.clone()
            };

            let log_receiver =
                trace_store
                    .subscribe_run_logs(&repo_key, &run_key)
                    .await
                    .map_err(|e| error::AutoschematicServerError {
                        kind: AutoschematicServerErrorType::InternalError(e),
                    })?;

            let mut stream = stream
                .aggregate_continuations()
                // aggregate continuation frames up to 1MiB
                .max_continuation_size(2_usize.pow(20));

            // start task but don't wait for it
            actix_web::rt::spawn(async move {
                //
                for log in logs {
                    if let Err(Closed) = ws_session.binary(log).await {
                        return;
                    }
                }

                if let Some(mut log_receiver) = log_receiver {
                    loop {
                        let res = log_receiver.recv().await;
                        match res {
                            Ok(log) => {
                                if let Err(Closed) = ws_session.binary(log).await {
                                    return;
                                }
                            }
                            Err(e) => {
                                tracing::error!("log_receiver: {}", e);
                            }
                        }
                    }
                } else {
                    tracing::info!("log_receiver: None");
                }

                // receive messages from websocket
                while let Some(msg) = stream.next().await {
                    match msg {
                        Ok(AggregatedMessage::Text(text)) => {
                            // echo text message
                            // ws_session.text(text).await.unwrap();
                        }

                        Ok(AggregatedMessage::Binary(bin)) => {
                            // echo binary message
                            // ws_session.binary(bin).await.unwrap();
                        }

                        Ok(AggregatedMessage::Ping(msg)) => {
                            // respond to PING frame with PONG frame
                            if let Err(Closed) = ws_session.pong(&msg).await {
                                return;
                            }
                        }

                        _ => {}
                    }
                }
            });

            Ok(res)
        } else {
            Ok(HttpResponse::NotFound().finish())
        }
    } else {
        Ok(HttpResponse::Unauthorized().finish())
        // User not authenticated, redirect to login
        // Ok(HttpResponse::Found()
        //     .append_header(("Location", "/api/login"))
        //     .finish())
    }
}

pub async fn spawn_task(
    req: HttpRequest,
    session: Session,
    param: web::Path<(String, String, u64, String, String)>,
    arg: web::Json<serde_json::Value>,
) -> Result<HttpResponse, actix_web::Error> {
    let Some((access_token, github_username)) = has_valid_session(&session).await? else {
        return Ok(HttpResponse::Unauthorized().finish());
        // return Ok(HttpResponse::Found()
        //     .append_header(("Location", "/api/login"))
        //     .finish());
    };

    let (owner, repo, installation_id, prefix, name) = param.into_inner();

    crate::task::spawn_task(&owner, &repo, &PathBuf::from(prefix), &name, installation_id, arg.0)
        .await
        .map_err(|e| error::AutoschematicServerError {
            kind: AutoschematicServerErrorType::InternalError(e),
        })?;

    Ok(HttpResponse::Created().finish())
}

pub async fn send_task_message(
    req: HttpRequest,
    session: Session,
    param: web::Path<(String, String, u64, String, String)>,
    msg: web::Json<TaskRegistryMessage>,
) -> Result<HttpResponse, actix_web::Error> {
    let Some((access_token, github_username)) = has_valid_session(&session).await? else {
        return Ok(HttpResponse::Unauthorized().finish());
    };

    let (owner, repo, installation_id, prefix, task_name) = param.into_inner();

    let registry_key = TaskRegistryKey {
        owner,
        repo,
        prefix: PathBuf::from(prefix),
        task_name,
    };

    match try_send_task_registry_message(&registry_key, msg.into_inner()).await {
        Ok(_) => Ok(HttpResponse::Created().finish()),
        Err(e) => Err(error::AutoschematicServerError {
            kind: AutoschematicServerErrorType::InternalError(e),
        }
        .into()),
    }
}

pub async fn task_state_subscribe(
    req: HttpRequest,
    session: Session,
    stream: web::Payload,
    param: web::Path<(String, String, u64, String, String)>,
) -> Result<HttpResponse, actix_web::Error> {
    let (res, mut ws_session, stream) = actix_ws::handle(&req, stream)?;

    let Some((access_token, github_username)) = has_valid_session(&session).await? else {
        return Ok(HttpResponse::Unauthorized().finish());
    };

    let (owner, repo, installation_id, prefix, task_name) = param.into_inner();

    tracing::warn!("is_repo_collaborator...");
    if is_repo_collaborator(&access_token, &github_username, &owner, &repo).await? {
        let registry_key = TaskRegistryKey {
            owner,
            repo,
            prefix: PathBuf::from(prefix),
            task_name,
        };

        let mut receiver = subscribe_task_state(&registry_key)
            .await
            .map_err(|e| error::AutoschematicServerError {
                kind: AutoschematicServerErrorType::InternalError(e),
            })?;

        let stream = stream
            .aggregate_continuations()
            // aggregate continuation frames up to 1MiB
            .max_continuation_size(2_usize.pow(20));

        actix_web::rt::spawn(async move {
            loop {
                let res = receiver.recv().await;
                match res {
                    Ok(msg) => match serde_json::to_string(&msg) {
                        Ok(payload) => {
                            ws_session.binary(payload).await.unwrap();
                        }
                        Err(e) => {
                            tracing::error!("failed to deserialize")
                        }
                    },
                    Err(e) => {
                        tracing::error!("log_receiver: {}", e);
                    }
                }
            }
        });
        Ok(res)
    } else {
        Ok(HttpResponse::Unauthorized().finish())
    }
}
