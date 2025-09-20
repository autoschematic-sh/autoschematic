#![deny(unused_must_use)]

mod aux_task;
mod changeset;
mod changeset_cache;
mod chwd;
mod command;
mod credentials;
mod dashboard;
mod error;
mod event_handlers;
mod github_cred_store;
mod github_util;
mod object;
mod repolock;
mod secret;
mod template;
mod tracestore;
mod url_builder;
mod util;

use actix_cors::Cors;
// use actix_files::NamedFile;
use actix_session::{Session, SessionMiddleware, storage::CookieSessionStore};
use anyhow::Context;
use autoschematic_core::{
    aux_task::registry::TaskRegistry,
    keystore::{KeyStore, keystore_init},
};
use dashboard::api_util::get_self;
use error::{AutoschematicServerError, AutoschematicServerErrorType};
use octocrab::{Octocrab, models::webhook_events::WebhookEvent};
use once_cell::{self, sync::OnceCell};
use ron_pfnsec_fork as ron;
use secrecy::ExposeSecret;
use serde::Deserialize;
use serde_json::json;
use std::{collections::HashMap, env, path::PathBuf, sync::Arc};
use tokio::sync::RwLock;
use tracestore::{InMemTraceStore, TraceStore};
use tracing_subscriber::EnvFilter;
use url_builder::URLBuilder;
use util::validate_github_hmac;

use actix_web::{
    App, Error, HttpRequest, HttpResponse, HttpServer, Responder,
    cookie::{Key, SameSite},
    dev::{ServiceRequest, ServiceResponse},
    middleware::Logger,
    web::{self},
};

use crate::{
    dashboard::api_util::has_valid_session,
    github_cred_store::{GITHUB_CRED_STORE, GithubCredStore, GithubCredStoreFile, get_github_cred_store},
};

static DOMAIN: OnceCell<String> = OnceCell::new();
// static REPOLOCKSTORE: OnceCell<Box<dyn RepoLockStore>> = OnceCell::new();
pub static TASK_REGISTRY: OnceCell<TaskRegistry> = OnceCell::new();
static TRACESTORE: OnceCell<Box<dyn TraceStore>> = OnceCell::new();

lazy_static::lazy_static! {
    pub static ref RON: ron::options::Options = ron::Options::default()
    .with_default_extension(ron::extensions::Extensions::UNWRAP_NEWTYPES)
    .with_default_extension(ron::extensions::Extensions::UNWRAP_VARIANT_NEWTYPES)
    .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME);

    pub static ref KEYSTORE: Arc<dyn KeyStore> = env::var("KEYSTORE")
        .context("Missing KEYSTORE environment variable")
        .map(|path| keystore_init(&path).expect("Failed to init keystore"))
        .unwrap();

    pub static ref GITHUB_MANIFEST_ENABLED: bool = match env::var("AUTOSCHEMATIC_GITHUB_MANIFEST_ENABLED") {
        Ok(s) if s == "false" => false,
        Ok(_) => true,
        Err(_) => false,
    };
}

pub fn main() {
    actix_web::rt::System::with_tokio_rt(|| tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap())
        .block_on(async_main())
        .unwrap();
}

async fn async_main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_target(true)
        .with_thread_ids(true)
        .with_line_number(true)
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // Install crypto provider for TLS
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    let webhook_domain = env::var("WEBHOOK_DOMAIN").context("Missing WEBHOOK_DOMAIN environment variable")?;

    DOMAIN.set(webhook_domain.clone()).unwrap();

    let session_key = env::var("SESSION_KEY")
        .context("Missing SESSION_KEY environment variable")
        .map(|key| base64::decode(key).expect("Invalid SESSION_KEY format"))?;

    TRACESTORE.set(Box::new(InMemTraceStore::default())).unwrap();

    TASK_REGISTRY
        .set(TaskRegistry {
            entries: RwLock::new(HashMap::new()),
        })
        .unwrap();

    if *GITHUB_MANIFEST_ENABLED {
        match GithubCredStoreFile::load().await? {
            Some(f) => {
                GITHUB_CRED_STORE.set(RwLock::new(GithubCredStore::from(f))).unwrap();
            }
            None => {
                GITHUB_CRED_STORE.set(RwLock::new(GithubCredStore::new().await?)).unwrap();
            }
        }
    } else {
        GITHUB_CRED_STORE.set(RwLock::new(GithubCredStore::new().await?)).unwrap();
    }

    tracing::info!("Service configured with webhook URL: https://{}", webhook_domain);
    tracing::info!("Visit https://{}/create-app to create a Github App", webhook_domain);
    // TODO Add the manifest install support here!
    // TODO does that mean we get the secret from github... and then we have to store it?
    // Or do we need to provide our secret value to github somehow after the manifest hook?

    Ok(HttpServer::new(move || {
        let cors = Cors::default()
            .allowed_origin("http://localhost:5173")
            .allowed_origin("https://autoschematic.sh")
            .allowed_methods(vec!["GET", "POST", "PUT", "DELETE", "OPTIONS"])
            .allowed_headers(vec![
                "Authorization",
                "Content-Type",
                "Accept",
                "Upgrade",
                "Connection",
                "Sec-WebSocket-Key",
                "Sec-WebSocket-Version",
                "Sec-WebSocket-Extensions",
                "ngrok-skip-browser-warning",
            ])
            .supports_credentials()
            .max_age(3600);

        App::new()
            .wrap(Logger::new("%r %s %b %D ms %a %{User-Agent}i"))
            .wrap(
                SessionMiddleware::builder(CookieSessionStore::default(), Key::from(&session_key))
                    .cookie_secure(true) // required with SameSite=None
                    .cookie_same_site(SameSite::None) // allow cross-site requests
                    .cookie_domain(DOMAIN.get().cloned())
                    .build(),
            )
            .wrap(cors)
            .route("/health", web::get().to(health_check))
            .route("/api/create-app", web::get().to(create_app))
            .route("/api/webhook", web::post().to(github_webhook))
            .route("/api/oauth", web::post().to(oauth))
            .route("/api/oauth", web::get().to(oauth))
            .route("/api/manifest", web::get().to(manifest))
            .route("/api/login", web::get().to(login))
            .route("/api/github_app_info", web::get().to(get_github_app_info))
            .route("/api/repo/", web::get().to(dashboard::routes::install_list))
            .route(
                "/api/repo/{owner}/{repo}/{installation_id}/view",
                web::get().to(dashboard::routes::repo_view),
            )
            .route(
                "/api/repo/{owner}/{repo}/{installation_id}/{prefix}/{task}/spawn",
                web::post().to(dashboard::routes::spawn_aux_task),
            )
            .route(
                "/api/repo/{owner}/{repo}/{installation_id}/{prefix}/{task}/send",
                web::post().to(dashboard::routes::send_task_message),
            )
            .route(
                "/api/repo/{owner}/{repo}/pr/{issue}/",
                web::get().to(dashboard::routes::dashboard),
            )
            .route(
                "/api/repo/{owner}/{repo}/pr/{issue}/{run}/logs",
                web::get().to(dashboard::routes::log_subscribe),
            )
            .route("/api/pubkeys", web::get().to(list_pubkeys))
            .route("/api/pubkey/{id}", web::get().to(get_pubkey))
        // .service(
        //     actix_files::Files::new("/", "./dashboard-react/dist/")
        //         .index_file("index.html")
        //         .default_handler(actix_web::dev::fn_service(|req: ServiceRequest| async {
        //             let (req, _) = req.into_parts();
        //             let file = NamedFile::open_async("./dashboard-react/dist/index.html").await?;
        //             let res = file.into_response(&req);
        //             Ok(ServiceResponse::new(req, res))
        //         })),
        // )
    })
    .bind("127.0.0.1:8086")?
    .run()
    .await?)
}

async fn health_check() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

async fn login() -> Result<HttpResponse, actix_web::Error> {
    let domain = DOMAIN.get().context("Missing WEBHOOK_DOMAIN environment variable").unwrap();
    let cred = get_github_cred_store().await?.read().await;

    let Some(client_id) = cred.client_id.as_ref().map(|s| s.expose_secret()) else {
        tracing::error!("GITHUB_CLIENT_ID not configured");
        return Err(AutoschematicServerError {
            kind: AutoschematicServerErrorType::ConfigurationError {
                name: "GITHUB_CLIENT_ID".to_string(),
                message: "OAuth client ID must be configured".to_string(),
            },
        }
        .into());
    };

    let redirect_uri = format!("https://{domain}/api/oauth");

    let authorize_url =
        format!("https://github.com/login/oauth/authorize?client_id={client_id}&redirect_uri={redirect_uri}&scope=repo");
    Ok(HttpResponse::Found().append_header(("Location", authorize_url)).finish())
}

async fn list_pubkeys() -> Result<HttpResponse, Error> {
    let Ok(pubkey_list) = KEYSTORE.list() else {
        return Err(error::AutoschematicServerError {
            kind: AutoschematicServerErrorType::InternalError(anyhow::anyhow!("Keystore list() failed")),
        }
        .into());
    };

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .body(serde_json::to_string(&pubkey_list)?))
}

async fn get_pubkey(param: web::Path<String>) -> Result<HttpResponse, Error> {
    let key_id = param.into_inner();

    let Ok(pubkey) = KEYSTORE.get_public_key(&key_id) else {
        return Err(error::AutoschematicServerError {
            kind: AutoschematicServerErrorType::InternalError(anyhow::anyhow!("Keystore list() failed")),
        }
        .into());
    };

    Ok(HttpResponse::Ok().content_type("text/plain").body(pubkey))
}

/// Handles incoming GitHub webhook events
///
/// Validates the webhook signature, parses the event payload,
/// and sends it on to event_handler::dispatch(...).
///
/// # Headers
/// - X-GitHub-Event: Event type (required)
/// - X-Hub-Signature-256: HMAC signature (required)
const DEFAULT_CONFIG_LIMIT: usize = 262_144; // 2^18 bytes (256KiB)
async fn github_webhook(req: HttpRequest, payload: web::Payload) -> Result<HttpResponse, AutoschematicServerError> {
    tracing::debug!("Received webhook request");

    let event_header = req.headers().get("X-GitHub-Event").ok_or_else(|| {
        tracing::warn!("Missing X-GitHub-Event header");
        AutoschematicServerError {
            kind: AutoschematicServerErrorType::MissingHeader("X-GitHub-Event".into()),
        }
    })?;

    let event_header = event_header.to_str().map_err(|e| {
        tracing::warn!("Invalid X-GitHub-Event header: {}", e);
        AutoschematicServerError::from(e)
    })?;

    let payload_signature = req.headers().get("X-Hub-Signature-256").ok_or_else(|| {
        tracing::warn!("Missing X-Hub-Signature-256 header");
        AutoschematicServerError {
            kind: AutoschematicServerErrorType::MissingHeader("X-Hub-Signature-256".into()),
        }
    })?;

    let body_bytes = payload.to_bytes_limited(DEFAULT_CONFIG_LIMIT).await??;

    validate_github_hmac(&body_bytes, payload_signature).await?;

    let webhook_event =
        WebhookEvent::try_from_header_and_body(event_header, &body_bytes).map_err(AutoschematicServerError::from)?;

    match event_handlers::dispatch(webhook_event).await {
        Ok(_) => {
            tracing::info!("Success!");
        }
        Err(e) => {
            tracing::error!("{:?}", e);
            return Err(e);
        }
    };

    Ok(HttpResponse::Created().finish())
}

#[derive(Deserialize)]
struct AuthRequest {
    code: String,
}

/// Handles OAuth callback from GitHub
/// Exchanges the temporary code for an access token and stores it in the session.
async fn oauth(query: web::Query<AuthRequest>, session: Session) -> Result<HttpResponse, Error> {
    let cred = get_github_cred_store().await?.read().await;

    let Some(client_id) = cred.client_id.as_ref().map(|s| s.expose_secret()) else {
        tracing::error!("GITHUB_CLIENT_ID not configured");
        return Err(AutoschematicServerError {
            kind: AutoschematicServerErrorType::ConfigurationError {
                name: "GITHUB_CLIENT_ID".to_string(),
                message: "OAuth client ID must be configured".to_string(),
            },
        }
        .into());
    };

    let Some(client_secret) = cred.client_secret.as_ref().map(|s| s.expose_secret()) else {
        tracing::error!("GITHUB_CLIENT_SECRET not configured");
        return Err(AutoschematicServerError {
            kind: AutoschematicServerErrorType::ConfigurationError {
                name: "GITHUB_CLIENT_SECRET".to_string(),
                message: "OAuth client secret must be configured".to_string(),
            },
        }
        .into());
    };

    let code = &query.code;

    // Exchange code for access token
    let client = reqwest::Client::new();
    let params = [
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("code", &code.clone()),
    ];

    let res = client
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .form(&params)
        .send()
        .await
        .map_err(|e| {
            tracing::error!("OAuth token exchange failed: {}", e);
            AutoschematicServerError::from(e)
        })?;

    let res_json: serde_json::Value = res.json().await.map_err(|e| {
        tracing::error!("Invalid OAuth response format: {}", e);
        AutoschematicServerError::from(e)
    })?;

    let access_token = res_json.get("access_token").and_then(|v| v.as_str()).ok_or_else(|| {
        tracing::error!("OAuth response missing access_token");
        AutoschematicServerError {
            kind: AutoschematicServerErrorType::ConfigurationError {
                name: "access_token".to_string(),
                message: "GitHub OAuth response missing access token".to_string(),
            },
        }
    })?;

    let res = get_self(access_token).await?;

    let user: serde_json::Value = res.json().await.map_err(|e| {
        tracing::error!("Couldn't parse user json {}", e);
        AutoschematicServerError::from(e)
    })?;

    let Some(username) = user.get("login") else {
        return Ok(HttpResponse::NotFound().finish());
    };

    let Some(username) = username.as_str() else {
        return Ok(HttpResponse::NotFound().finish());
    };

    // Store access token in session
    session.insert("access_token", access_token.to_string())?;
    session.insert("github_username", username.to_string())?;

    let domain = DOMAIN.get().context("Missing WEBHOOK_DOMAIN environment variable").unwrap();
    // Redirect to dashboard
    Ok(HttpResponse::TemporaryRedirect()
        .insert_header(("Location", format!("http://localhost:5173/clusters/{}", domain)))
        .finish())
}

#[derive(Deserialize)]
struct ManifestRequest {
    code: String,
    state: String,
}

/// Handles Manifest installation callback from GitHub
async fn manifest(query: web::Query<ManifestRequest>, session: Session) -> Result<HttpResponse, Error> {
    if !*GITHUB_MANIFEST_ENABLED {
        return Err(AutoschematicServerError {
            kind: AutoschematicServerErrorType::ConfigurationError {
                name: "AUTOSCHEMATIC_GITHUB_MANIFEST_ENABLED".to_string(),
                message: "Github App Manifest support is disabled.".to_string(),
            },
        }
        .into());
    }

    let code = &query.code;

    // Exchange code for access token
    let client = reqwest::Client::new();
    let url = format!("https://api.github.com/app-manifests/{code}/conversions");

    tracing::error!("{:#?}", url);

    let res = client
        .post(url)
        .header("Accept", "application/json")
        .header("User-Agent", "Autoschematic-Cluster")
        .send()
        .await
        .map_err(|e| {
            tracing::error!("Manifest conversion failed: {}", e);
            AutoschematicServerError::from(e)
        })?;

    let res_json: serde_json::Value = res.json().await.map_err(|e| {
        tracing::error!("Invalid manifest response format: {}", e);
        AutoschematicServerError::from(e)
    })?;

    let cred = get_github_cred_store().await?;

    cred.write()
        .await
        .from_manifest_result(&res_json)
        .map_err(|e| AutoschematicServerError::from(e))?;

    cred.read()
        .await
        .save()
        .await
        .map_err(|e| AutoschematicServerError::from(e))?;

    Ok(HttpResponse::TemporaryRedirect()
        .append_header((
            "Location",
            format!("https://autoschematic.sh/clusters/{}", DOMAIN.get().unwrap()),
        ))
        .finish())
}

async fn get_github_app_info(session: Session) -> Result<HttpResponse, Error> {
    let Some((_access_token, _github_username)) = has_valid_session(&session).await? else {
        return Ok(HttpResponse::Unauthorized().finish());
    };

    let cred = get_github_cred_store().await?;

    let Some(app_name) = &cred.read().await.app_name else {
        return Ok(HttpResponse::Unauthorized().finish());
    };

    let Some(app_slug) = &cred.read().await.app_slug else {
        return Ok(HttpResponse::Unauthorized().finish());
    };

    let Some(client_id) = &cred.read().await.client_id else {
        return Ok(HttpResponse::Unauthorized().finish());
    };

    Ok(HttpResponse::Ok().json(json!({
        "client_id": client_id.expose_secret(),
        "app_name": app_name,
        "app_slug": app_slug,
    })))
}

async fn create_app() -> Result<HttpResponse, AutoschematicServerError> {
    let domain = DOMAIN.get().context("Missing WEBHOOK_DOMAIN environment variable").unwrap();

    let webhook_url = format!("https://{domain}");

    let mut ub = URLBuilder::new();

    // Build the GitHub App creation URL
    ub.set_protocol("https")
        .set_host("github.com")
        .add_route("settings")
        .add_route("apps")
        .add_route("new")
        .add_param("name", "autoschematic")
        .add_param("public", "false")
        .add_param("pull_requests", "write")
        .add_param("pull_request_reviews", "write")
        .add_param("checks", "write")
        .add_param("contents", "write")
        .add_param("merge_queues", "write")
        .add_param("issues", "write")
        .add_param("url", &webhook_url)
        .add_param("callback_urls[]", &format!("{webhook_url}/api/oauth"))
        .add_param("webhook_url", &format!("{webhook_url}/api/webhook"))
        .add_param("webhook_active", "true")
        .add_param("events[]", "check_run")
        .add_param("events[]", "check_suite")
        .add_param("events[]", "pull_request")
        .add_param("events[]", "pull_request_review")
        .add_param("events[]", "pull_request_review_comment");

    // Redirect the user to the GitHub App creation URL
    Ok(HttpResponse::TemporaryRedirect()
        .append_header(("Location", ub.build()))
        .finish())
}
