use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use dashmap::DashMap;

use anyhow::{Result, bail};
use autoschematic_core::{
    config::AutoschematicConfig,
    config_rbac::AutoschematicRbacConfig,
    connector::{DocIdent, FilterResponse},
    connector_cache::{ConnectorCache, TopResponse},
    manifest::ConnectorManifest,
    template::{self},
    util::{RON, split_prefix_addr},
    workflow::{
        filter::filter,
        get::get,
        get_docstring::{get_docstring, get_system_docstring},
        rename,
    },
};
use lsp_types::*;
use path_at::ident_at;
use serde::de::DeserializeOwned;
use tokio::{sync::RwLock, task::JoinSet};
use tower_lsp_server::{Client, LanguageServer, LspService, Server, jsonrpc::Error as LspError, lsp_types};
use tracing_subscriber::filter::LevelFilter;
use util::{diag_to_lsp, lsp_error, lsp_param_to_path};

use crate::{
    path_at::Component,
    reindent::reindent,
    util::{lsp_param_to_rename_path, map_lsp_error},
};

pub mod parse;
pub mod path_at;
pub mod reindent;
pub mod util;

struct Backend {
    client: Client,
    docs: DashMap<Uri, String>,
    autoschematic_config: RwLock<Option<AutoschematicConfig>>,
    connector_cache: Arc<ConnectorCache>,
}

impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult, LspError> {
        // TODO Don't auto-load ourselves. Wait for the vscode extension to do it so it can display progress.
        let _ = self.try_reload_config().await;

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                document_formatting_provider: Some(OneOf::Left(true)),

                hover_provider: Some(HoverProviderCapability::Simple(true)),
                text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![".".to_string(), ":".to_string()]),
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client.log_message(MessageType::INFO, "Autoschematic LSP ready").await;
    }

    async fn shutdown(&self) -> Result<(), LspError> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.docs
            .insert(params.text_document.uri.clone(), params.text_document.text.clone());

        self.validate(&params.text_document.uri, &params.text_document.text).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        // we declared FULL sync, so there is exactly one change
        let text = &params.content_changes[0].text;

        self.docs.insert(params.text_document.uri.clone(), text.clone());

        self.validate(&params.text_document.uri, &text.clone()).await;
    }

    async fn hover(&self, params: HoverParams) -> tower_lsp_server::jsonrpc::Result<Option<Hover>> {
        let Ok(file_contents) = self
            .load_file_uri(&params.text_document_position_params.text_document.uri)
            .await
        else {
            return Ok(None);
        };

        let line = params.text_document_position_params.position.line + 1;
        let col = params.text_document_position_params.position.character + 1;

        // let path = path_at(&file_contents, line as usize, col as usize);
        let Ok(ident) = ident_at(&file_contents, line as usize, col as usize) else {
            return Ok(None);
        };

        let Ok(file_path) = self.uri_to_local_path(&params.text_document_position_params.text_document.uri) else {
            return Ok(None);
        };

        if let Some(ident) = ident {
            let Some(ref autoschematic_config) = *self.autoschematic_config.read().await else {
                return Ok(None);
            };

            let Some((prefix, addr)) = split_prefix_addr(autoschematic_config, &file_path) else {
                if let Ok(Some(res)) = get_system_docstring(&file_path, ident) {
                    return Ok(Some(Hover {
                        contents: HoverContents::Markup(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: res.markdown.to_string(),
                        }),
                        range: None,
                    }));
                } else {
                    return Ok(None);
                }
            };

            if let Ok(Some(res)) = get_docstring(autoschematic_config, &self.connector_cache, None, &prefix, &addr, ident).await
            {
                return Ok(Some(Hover {
                    contents: HoverContents::Markup(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: res.markdown.to_string(),
                    }),
                    range: None,
                }));
            }
        }

        Ok(None)
    }

    async fn completion(&self, params: CompletionParams) -> tower_lsp_server::jsonrpc::Result<Option<CompletionResponse>> {
        let Ok(file_contents) = self.load_file_uri(&params.text_document_position.text_document.uri).await else {
            return Ok(None);
        };

        let line = params.text_document_position.position.line + 1;
        let col = params.text_document_position.position.character + 1;

        let Ok(Some(path)) = path_at::path_at(&file_contents, line as usize, col as usize) else {
            return Ok(None);
        };

        if path.len() < 2 {
            return Ok(None);
        }

        let Some(Component::Name(parent)) = path.get(path.len() - 2) else {
            return Ok(None);
        };

        let Some(Component::Name(_name)) = path.last() else {
            return Ok(None);
        };

        let ident = DocIdent::Struct {
            name: parent.to_string(),
        };

        let Some(ref autoschematic_config) = *self.autoschematic_config.read().await else {
            return Ok(None);
        };

        let Ok(file_path) = self.uri_to_local_path(&params.text_document_position.text_document.uri) else {
            return Ok(None);
        };

        // If it's not under a prefix, try it against the system docstring lookup...
        let Some((prefix, addr)) = split_prefix_addr(autoschematic_config, &file_path) else {
            if let Ok(Some(res)) = get_system_docstring(&file_path, ident) {
                return Ok(Some(CompletionResponse::Array(
                    res.fields
                        .iter()
                        .map(|f| {
                            let ident = DocIdent::Field {
                                parent: parent.to_string(),
                                name: f.to_owned(),
                            };

                            if let Ok(Some(field_res)) = get_system_docstring(&file_path, ident) {
                                CompletionItem {
                                    label: f.to_string(),
                                    label_details: Some(CompletionItemLabelDetails {
                                        detail: Some(field_res.r#type.clone()),
                                        ..Default::default()
                                    }),
                                    kind: Some(CompletionItemKind::FIELD),
                                    documentation: Some(lsp_types::Documentation::MarkupContent(MarkupContent {
                                        kind: MarkupKind::Markdown,
                                        value: field_res.markdown.clone(),
                                    })),
                                    ..Default::default()
                                }
                            } else {
                                CompletionItem {
                                    label: f.to_string(),
                                    // label_details: Some(CompletionItemLabelDetails {
                                    //     detail: Some(res.r#type.clone()),
                                    //     ..Default::default()
                                    // }),
                                    kind: Some(CompletionItemKind::FIELD),
                                    // documentation: Some(lsp_types::Documentation::MarkupContent(MarkupContent {
                                    //     kind: MarkupKind::Markdown,
                                    //     value: res.markdown.clone(),
                                    // })),
                                    ..Default::default()
                                }
                            }
                        })
                        .collect(),
                )));
            } else {
                return Ok(None);
            }
        };

        // ...otherwise, just get a regular resource docstring in that prefix.
        if let Ok(Some(res)) = get_docstring(autoschematic_config, &self.connector_cache, None, &prefix, &addr, ident).await {
            let mut items = Vec::new();
            if res.fields.is_empty() {
                return Ok(None);
            }
            for field in res.fields {
                let field_doc = if let Ok(Some(field_doc)) = get_docstring(
                    autoschematic_config,
                    &self.connector_cache,
                    None,
                    &prefix,
                    &addr,
                    DocIdent::Field {
                        parent: parent.to_string(),
                        name: field.clone(),
                    },
                )
                .await
                {
                    Some(field_doc)
                } else {
                    None
                };

                eprintln!("{:?}", field_doc);

                let detail = field_doc.as_ref().map(|f| f.r#type.clone());
                let field_doc = field_doc.map(|f| {
                    lsp_types::Documentation::MarkupContent(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: f.markdown.clone(),
                    })
                });

                items.push(CompletionItem {
                    label: field,
                    kind: Some(CompletionItemKind::FIELD),
                    detail,
                    documentation: field_doc,
                    ..Default::default()
                });
            }
            return Ok(Some(CompletionResponse::Array(items)));
        }

        Ok(None)
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> tower_lsp_server::jsonrpc::Result<Option<Vec<TextEdit>>> {
        if !params.text_document.uri.as_str().ends_with(".ron") {
            return Ok(None);
        }

        let Some(file_contents) = self.docs.get(&params.text_document.uri) else {
            return Ok(None);
        };

        match reindent(&file_contents) {
            Ok(new_contents) => Ok(Some(vec![TextEdit {
                range: Range {
                    start: Position { line: 0, character: 0 },
                    end: Position {
                        line: u32::MAX,
                        character: u32::MAX,
                    },
                },
                new_text: new_contents,
            }])),
            Err(e) => Err(lsp_error(e)),
        }
    }

    async fn execute_command(&self, params: ExecuteCommandParams) -> tower_lsp_server::jsonrpc::Result<Option<LSPAny>> {
        match self.try_load_config().await {
            Ok(_) => {}
            Err(e) => {
                eprintln!("{e}");
                return Err(lsp_error(e));
            }
        }

        let keystore = None;

        match params.command.as_str() {
            "relaunch" => {
                *self.autoschematic_config.write().await = None;
                self.connector_cache.clear().await;
                match self.try_reload_config().await {
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("{e}");
                        return Err(lsp_error(e));
                    }
                }
            }
            "rename" => {
                let Some((old_path, new_path)) = lsp_param_to_rename_path(params) else {
                    return Ok(None);
                };

                let Some(ref autoschematic_config) = *self.autoschematic_config.read().await else {
                    return Ok(None);
                };

                match rename::rename(autoschematic_config, &self.connector_cache, keystore, &old_path, &new_path).await {
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("{e}");
                        return Err(lsp_error(e));
                    }
                }
            }
            "get" => {
                let Some(path) = lsp_param_to_path(params) else {
                    return Ok(None);
                };

                let Some(ref autoschematic_config) = *self.autoschematic_config.read().await else {
                    return Ok(None);
                };

                let Some((prefix, addr)) = split_prefix_addr(autoschematic_config, &path) else {
                    return Ok(None);
                };

                match get(autoschematic_config, &self.connector_cache, keystore, &prefix, &addr).await {
                    Ok(Some(res)) => match String::from_utf8(res) {
                        Ok(s) => {
                            let Ok(s) = serde_json::to_value(s) else {
                                return Ok(None);
                            };
                            return Ok(Some(s));
                        }
                        Err(e) => {
                            return Err(lsp_error(e.into()));
                        }
                    },
                    Ok(None) => return Ok(None),
                    Err(e) => {
                        return Err(lsp_error(e.into()));
                    }
                }
            }
            "get_untemplate" => {
                let Some(path) = lsp_param_to_path(params) else {
                    return Ok(None);
                };

                let Some(ref autoschematic_config) = *self.autoschematic_config.read().await else {
                    return Ok(None);
                };

                let Some((prefix, addr)) = split_prefix_addr(autoschematic_config, &path) else {
                    return Ok(None);
                };

                let remote_content = get(autoschematic_config, &self.connector_cache, keystore, &prefix, &addr).await;
                let remote_content = match map_lsp_error(remote_content)? {
                    Some(res) => map_lsp_error(String::from_utf8(res))?,
                    None => return Ok(None),
                };

                let local_content = tokio::fs::read_to_string(prefix.join(addr)).await;
                let local_content = map_lsp_error(local_content)?;

                let comments = template::extract_comments(&local_content);
                let reverse_templated = template::reverse_template_config(&prefix, &local_content, &remote_content, 8);
                let reverse_templated = map_lsp_error(reverse_templated)?;

                let result = template::apply_comments(reverse_templated, comments);

                let Ok(value) = serde_json::to_value(result) else {
                    return Ok(None);
                };
                return Ok(Some(value));
            }
            "filter" => {
                let Some(path) = lsp_param_to_path(params) else {
                    return Ok(None);
                };

                let Some(ref autoschematic_config) = *self.autoschematic_config.read().await else {
                    let Ok(value) = serde_json::to_value(false) else {
                        return Ok(None);
                    };
                    return Ok(Some(value));
                };

                let Some((prefix, addr)) = split_prefix_addr(autoschematic_config, &path) else {
                    let Ok(value) = serde_json::to_value(false) else {
                        return Ok(None);
                    };
                    return Ok(Some(value));
                };

                let Ok(filter_res) = filter(autoschematic_config, &self.connector_cache, keystore, None, &prefix, &addr).await
                else {
                    let Ok(value) = serde_json::to_value(false) else {
                        return Ok(None);
                    };
                    return Ok(Some(value));
                };
                let mut res = Vec::<String>::new();

                if filter_res.intersects(FilterResponse::Config) {
                    res.push(String::from("Config"));
                }
                if filter_res.intersects(FilterResponse::Resource) {
                    res.push(String::from("Resource"));
                }
                if filter_res.intersects(FilterResponse::Bundle) {
                    res.push(String::from("Bundle"));
                }
                if filter_res.intersects(FilterResponse::Task) {
                    res.push(String::from("Task"));
                }
                if filter_res.intersects(FilterResponse::Metric) {
                    res.push(String::from("Metric"));
                }
                let Ok(value) = serde_json::to_value(res) else {
                    return Ok(None);
                };
                return Ok(Some(value));
            }
            "top" => {
                let top_res = self.connector_cache.top().await;

                let mut res: HashMap<PathBuf, HashMap<String, TopResponse>> = HashMap::new();

                for (key, value) in top_res {
                    res.entry(key.prefix).or_default().insert(key.shortname, value);
                }

                let Ok(value) = serde_json::to_value(res) else {
                    return Ok(None);
                };
                return Ok(Some(value));
            }
            _ => {}
        }
        Ok(None)
    }
}

// fn word_at<'a>(text: &'a str, pos: Position) -> Option<(&'a str, Range)> {
//     let idx = lsp_offsets::offset(text, pos)?;      // convert LSP line/col to byte index
//     let bytes = text.as_bytes();

//     // Expand left and right while char::is_alphanumeric() || '_' || ':'.
//     let mut start = idx;
//     while start > 0 && is_ident_char(bytes[start-1]) { start -= 1; }
//     let mut end = idx;
//     while end < bytes.len() && is_ident_char(bytes[end]) { end += 1; }

//     let word = std::str::from_utf8(&bytes[start..end]).ok()?;
//     Some((
//         word,
//         Range::new(offset_to_pos(text, start)?, offset_to_pos(text, end)?),
//     ))
// }

// fn is_ident_char(b: u8) -> bool {
//     b == b'_' || b == b':' || (b as char).is_alphanumeric()
// }

impl Backend {
    async fn try_load_config(&self) -> anyhow::Result<()> {
        let need_reload = self.autoschematic_config.read().await.is_none();

        if need_reload {
            self.try_reload_config().await?;
        }

        Ok(())
    }
    async fn try_reload_config(&self) -> anyhow::Result<()> {
        let config: Option<AutoschematicConfig> = if PathBuf::from("autoschematic.ron").is_file() {
            match tokio::fs::read_to_string("autoschematic.ron").await {
                Ok(config_body) => match RON.from_str(&config_body) {
                    Ok(new_config) => Some(new_config),
                    Err(e) => {
                        eprintln!("Failed to parse autoschematic.ron: {e}");
                        self.client
                            .log_message(MessageType::ERROR, format!("Failed to parse autoschematic.ron: {e}"))
                            .await;
                        None
                    }
                },
                Err(e) => {
                    eprintln!("Failed to read autoschematic.ron: {e}");
                    self.client
                        .log_message(MessageType::ERROR, format!("Failed to read autoschematic.ron: {e}"))
                        .await;
                    None
                }
            }
        } else {
            None
        };

        *self.autoschematic_config.write().await = config;
        self.load_connectors().await?;

        Ok(())
    }

    async fn validate(&self, uri: &Uri, text: &str) {
        match self.try_deserialize(uri, text).await {
            Ok(diag) => {
                self.client.publish_diagnostics(uri.clone(), diag, None).await;
            }
            Err(e) => {
                eprintln!("{e}");
            }
        };
    }

    async fn diag_file(&self, path: &Path, body: &str) -> Result<Vec<Diagnostic>> {
        let mut res = Vec::new();

        self.try_load_config().await?;

        let Some(ref autoschematic_config) = *self.autoschematic_config.read().await else {
            return Ok(res);
        };

        for (prefix_name, prefix_def) in &autoschematic_config.prefixes {
            let prefix = PathBuf::from(prefix_name);

            let Ok(addr) = path.strip_prefix(&prefix) else {
                continue;
            };

            for connector_def in &prefix_def.connectors {
                match self
                    .connector_cache
                    .filter_cached(&connector_def.shortname, &prefix, addr)
                    .await?
                {
                    // TODO If the user edits a connector's config file,
                    // we need to re-init the connector, and clear the filter cache for that connector!
                    // autoschematic_core::connector::FilterResponse::Config => {
                    // autoschematic_core::connector::FilterResponse::Task => {}
                    res if res == autoschematic_core::connector::FilterResponse::none() => {}
                    _ => {
                        if let Some((connector, _inbox)) =
                            self.connector_cache.get_connector(&connector_def.shortname, &prefix).await
                        {
                            // eprintln!("{} filter: {:?} = true", connector_def.name, addr);
                            if let Some(diag) = connector.diag(addr, body.as_bytes()).await? {
                                res.append(&mut diag_to_lsp(diag));
                            }
                        }
                    }
                }
            }
        }

        Ok(res)
    }

    async fn load_connectors(&self) -> anyhow::Result<()> {
        let Some(ref autoschematic_config) = *self.autoschematic_config.read().await else {
            eprintln!("load_connectors: config none!");
            return Ok(());
        };

        let autoschematic_config = Arc::new(autoschematic_config.clone());
        // let mut handles = Vec::new();
        let mut joinset: JoinSet<anyhow::Result<()>> = JoinSet::new();

        let autoschematic_config = autoschematic_config.clone();
        for (prefix_name, prefix_def) in &autoschematic_config.prefixes {
            let autoschematic_config = autoschematic_config.clone();
            let prefix_def = prefix_def.clone();

            for connector_def in prefix_def.connectors {
                let connector_cache = self.connector_cache.clone();

                let autoschematic_config = autoschematic_config.clone();

                let prefix_name = prefix_name.clone();

                joinset.spawn(async move {
                    let (_connector, mut inbox) = connector_cache
                        .get_or_spawn_connector(&autoschematic_config, &prefix_name, &connector_def, None, false)
                        .await?;

                    // let sender_trace_handle = trace_handle.clone();
                    let _reader_handle = tokio::spawn(async move {
                        loop {
                            match inbox.recv().await {
                                Ok(Some(stdout)) => {
                                    // dbg!(&stdout);
                                    eprintln!("stdout: {stdout}");
                                    // self.client.log_message(MessageType::INFO, format!("{}", stdout)).await;
                                    // let res = append_run_log(&sender_trace_handle, stdout).await;
                                    // match res {
                                    //     Ok(_) => {}
                                    //     Err(_) => {}
                                    // }
                                }
                                Ok(None) => {}
                                Err(_) => break,
                            }
                        }
                    });

                    Ok(())
                });
            }
        }
        joinset.join_all().await;
        Ok(())
    }

    // Generic helper that runs serde and converts the error
    async fn check<T: DeserializeOwned>(&self, text: &str) -> Result<Option<Diagnostic>, anyhow::Error> {
        let res = ron::Deserializer::from_str_with_options(text, &RON);
        match res {
            Ok(mut deserializer) => {
                let result: Result<T, _> = serde_path_to_error::deserialize(&mut deserializer);
                match result {
                    Ok(_) => {}
                    Err(e) => {
                        let inner_error = deserializer.span_error(e.inner().clone());
                        return Ok(Some(Diagnostic {
                            range: Range::new(
                                Position::new(inner_error.span.start.line as u32 - 1, inner_error.span.start.col as u32 - 1),
                                Position::new(inner_error.span.end.line as u32 - 1, inner_error.span.end.col as u32),
                            ),
                            severity: Some(DiagnosticSeverity::ERROR),
                            // message: format!("{} at {}", e, path),
                            message: format!("{} at {}", inner_error.code, e.path()),
                            ..Default::default()
                        }));
                    }
                }
            }
            Err(e) => {
                return Ok(Some(Diagnostic {
                    range: Range::new(
                        Position::new(e.span.start.line as u32 - 1, e.span.start.col as u32 - 1),
                        Position::new(e.span.end.line as u32 - 1, e.span.end.col as u32),
                    ),
                    severity: Some(DiagnosticSeverity::ERROR),
                    // message: format!("{} at {}", e, path),
                    message: format!("{}", e.code),
                    ..Default::default()
                }));
            }
        };

        Ok(None)
    }

    fn uri_to_local_path(&self, uri: &Uri) -> anyhow::Result<PathBuf> {
        let Some(scheme) = uri.scheme() else { bail!("No uri scheme") };

        if !scheme.eq_lowercase("file") {
            bail!("Unknown uri scheme {}", scheme)
        }

        let file_path = PathBuf::from(uri.path().as_str());

        let Ok(cwd) = std::env::current_dir() else {
            bail!("Failed to get current directory");
        };

        let Ok(file_path) = file_path.strip_prefix(cwd) else {
            bail!("Outside of working dir");
        };

        Ok(file_path.into())
    }

    async fn load_file_uri(&self, uri: &Uri) -> anyhow::Result<String> {
        let Some(scheme) = uri.scheme() else { bail!("No uri scheme") };

        if !scheme.eq_lowercase("file") {
            bail!("Unknown uri scheme {}", scheme)
        }

        let path = PathBuf::from(uri.path().as_str());

        let res = tokio::fs::read_to_string(path).await?;

        Ok(res)
    }

    async fn try_deserialize(&self, uri: &Uri, text: &str) -> Result<Vec<Diagnostic>, anyhow::Error> {
        let mut res = Vec::new();

        // self.client.log_message(MessageType::WARNING, format!("{:?}", uri)).await;

        let Some(scheme) = uri.scheme() else { return Ok(res) };

        if !scheme.eq_lowercase("file") {
            return Ok(res);
        }

        let path = PathBuf::from(uri.path().as_str());

        let Ok(path) = path.strip_prefix(std::env::current_dir()?) else {
            return Ok(res);
        };

        // let Some(path) = path.stri

        // if !uri.as_str().ends_with(".ron") {
        //     return Ok(());
        // }

        let Some(path_str) = path.to_str() else {
            return Ok(res);
        };

        match path_str {
            "autoschematic.ron" => {
                self.try_reload_config().await?;

                if let Some(diag) = self.check::<AutoschematicConfig>(text).await? {
                    res.push(diag);
                }

                // if let Err(e) = self.load_connectors().await {
                //     res.push(Diagnostic {
                //         range: Range::new(Position::new(0, 0), Position::new(0, 0)),
                //         severity: Some(DiagnosticSeverity::ERROR),
                //         message: format!("{}", e),
                //         ..Default::default()
                //     });
                // }
            }
            "autoschematic.rbac.ron" => {
                if let Some(diag) = self.check::<AutoschematicRbacConfig>(text).await? {
                    res.push(diag);
                }
            }
            // "autoschematic.lock.ron" => {
            //     if let Some(diag) = self.check::<AutoschematicLockfile>(text).await? {
            //         res.push(diag);
            //     }
            // }
            "autoschematic.connector.ron" => {
                if let Some(diag) = self.check::<ConnectorManifest>(text).await? {
                    res.push(diag);
                }
            }
            s if s.ends_with(".ron") => {
                if let Some(diag) = self.check::<ron::Value>(text).await? {
                    res.push(diag);
                }
                let mut diag = self.diag_file(path, text).await?;
                res.append(&mut diag);
            }
            _ => {
                let mut diag = self.diag_file(path, text).await?;
                res.append(&mut diag);
            }
        }
        Ok(res)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_thread_ids(false)
        .with_ansi(false)
        .with_writer(std::io::stderr)
        .with_max_level(LevelFilter::WARN)
        .compact()
        .init();

    let connector_cache = ConnectorCache::default();

    let (stdin, stdout) = (tokio::io::stdin(), tokio::io::stdout());
    let (service, socket) = LspService::new(|client| Backend {
        client,
        docs: DashMap::new(),
        autoschematic_config: RwLock::new(None),
        connector_cache: Arc::new(connector_cache),
    });
    Server::new(stdin, stdout, socket).serve(service).await;
    Ok(())
}
