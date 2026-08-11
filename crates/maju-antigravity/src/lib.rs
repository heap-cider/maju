#![forbid(unsafe_code)]

mod agy;
mod catalog;
mod wire;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use agy::{PermissionMode, RunError, RunRequest};
use catalog::ModelCatalog;
use serde_json::{json, Value};
use tokio::io::BufReader;
use tokio::sync::{mpsc, watch, Mutex, OnceCell};
use uuid::Uuid;

struct App {
    agy_command: String,
    catalog: OnceCell<Arc<ModelCatalog>>,
    sessions: Mutex<HashMap<String, Session>>,
}

struct Session {
    cwd: PathBuf,
    system_prompt: Option<String>,
    model_id: String,
    effort: Option<String>,
    permission_mode: PermissionMode,
    conversation_id: Option<String>,
    cancel_tx: Option<watch::Sender<bool>>,
    busy: bool,
    turn_sequence: u64,
    accumulated_input_tokens: u64,
    accumulated_output_tokens: u64,
    accumulated_cached_input_tokens: u64,
    accumulated_total_tokens: u64,
}

struct PromptSnapshot {
    cwd: PathBuf,
    system_prompt: Option<String>,
    model_id: String,
    effort: Option<String>,
    permission_mode: PermissionMode,
    conversation_id: Option<String>,
    cancel_rx: watch::Receiver<bool>,
    turn_id: String,
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async_main());
    Ok(())
}

async fn async_main() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();
    let app = Arc::new(App {
        agy_command: resolve_agy_command(),
        catalog: OnceCell::new(),
        sessions: Mutex::new(HashMap::new()),
    });
    let (wire_tx, wire_rx) = mpsc::channel::<Value>(128);
    let writer = tokio::spawn(wire::writer(wire_rx));
    if let Err(error) = read_loop(app.clone(), wire_tx).await {
        tracing::error!("ACP input failed: {error}");
    }
    for session in app.sessions.lock().await.values_mut() {
        if let Some(cancel_tx) = session.cancel_tx.take() {
            let _ = cancel_tx.send(true);
        }
    }
    let _ = writer.await;
}

fn resolve_agy_command() -> String {
    if let Ok(command) = std::env::var("MAJU_ANTIGRAVITY_AGY_COMMAND") {
        if !command.trim().is_empty() {
            return command;
        }
    }
    let mut candidates = Vec::new();
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".local").join("bin").join("agy"));
    }
    #[cfg(windows)]
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        candidates.push(PathBuf::from(local).join("agy").join("bin").join("agy.exe"));
    }
    #[cfg(target_os = "macos")]
    candidates.push(PathBuf::from(
        "/Applications/Antigravity.app/Contents/Resources/app/bin/agy",
    ));
    candidates
        .into_iter()
        .find(|path| path.is_file())
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "agy".to_string())
}

async fn read_loop(app: Arc<App>, wire_tx: wire::Sender) -> std::io::Result<()> {
    let mut stdin = BufReader::new(tokio::io::stdin());
    while let Some(line) = wire::read_line(&mut stdin).await? {
        if line.trim().is_empty() {
            continue;
        }
        let message = match serde_json::from_str::<Value>(&line) {
            Ok(message) => message,
            Err(error) => {
                wire::send(
                    &wire_tx,
                    wire::error(Value::Null, -32700, format!("invalid JSON: {error}")),
                )
                .await;
                continue;
            }
        };
        dispatch(app.clone(), message, wire_tx.clone()).await;
    }
    Ok(())
}

async fn dispatch(app: Arc<App>, message: Value, wire_tx: wire::Sender) {
    match wire::classify(&message) {
        wire::Inbound::Request { id, method, params } => match method.as_str() {
            "initialize" => initialize(id, params, &wire_tx).await,
            "session/new" => {
                tokio::spawn(async move { session_new(app, id, params, wire_tx).await });
            }
            "session/set_config_option" => {
                set_config_option(&app, id, params, &wire_tx).await;
            }
            "session/set_model" => set_model(&app, id, params, &wire_tx).await,
            "session/prompt" => {
                tokio::spawn(async move { session_prompt(app, id, params, wire_tx).await });
            }
            "session/cancel" => {
                cancel_session(&app, &params).await;
                wire::send(&wire_tx, wire::ok(id, Value::Null)).await;
            }
            "session/close" => close_session(&app, id, params, &wire_tx).await,
            _ => {
                wire::send(
                    &wire_tx,
                    wire::error(
                        id,
                        wire::METHOD_NOT_FOUND,
                        format!("method not found: {method}"),
                    ),
                )
                .await;
            }
        },
        wire::Inbound::Notification { method, params } => {
            if method == "session/cancel" {
                cancel_session(&app, &params).await;
            }
        }
        wire::Inbound::Ignored => {}
        wire::Inbound::Invalid { id, message } => {
            wire::send(&wire_tx, wire::error(id, wire::INVALID_REQUEST, message)).await;
        }
    }
}

async fn initialize(id: Value, params: Value, wire_tx: &wire::Sender) {
    let Some(protocol_version) = params.get("protocolVersion").and_then(Value::as_u64) else {
        wire::send(
            wire_tx,
            wire::error(
                id,
                wire::INVALID_PARAMS,
                "initialize requires protocolVersion",
            ),
        )
        .await;
        return;
    };
    wire::send(
        wire_tx,
        wire::ok(
            id,
            json!({
                "protocolVersion": protocol_version.min(1),
                "agentCapabilities": {
                    "loadSession": false,
                    "promptCapabilities": {
                        "image": false,
                        "audio": false,
                        "embeddedContext": false
                    },
                    "mcpCapabilities": { "http": false, "sse": false }
                },
                "agentInfo": {
                    "name": "maju-antigravity",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }),
        ),
    )
    .await;
}

impl App {
    async fn model_catalog(&self) -> Result<Arc<ModelCatalog>, String> {
        self.catalog
            .get_or_try_init(|| async { catalog::discover(&self.agy_command).await.map(Arc::new) })
            .await
            .cloned()
    }
}

async fn session_new(app: Arc<App>, id: Value, params: Value, wire_tx: wire::Sender) {
    let Some(cwd) = params.get("cwd").and_then(Value::as_str) else {
        reject(&wire_tx, id, "session/new requires cwd").await;
        return;
    };
    let cwd = PathBuf::from(cwd);
    if !cwd.is_absolute() {
        reject(&wire_tx, id, "session/new cwd must be absolute").await;
        return;
    }
    let catalog = match app.model_catalog().await {
        Ok(catalog) => catalog,
        Err(error) => {
            wire::send(
                &wire_tx,
                wire::error(
                    id,
                    wire::INTERNAL_ERROR,
                    format!("could not discover Antigravity models: {error}"),
                ),
            )
            .await;
            return;
        }
    };
    let Some(default_model) = catalog.families.first() else {
        reject(&wire_tx, id, "Antigravity has no available models").await;
        return;
    };
    let session_id = format!("agy_{}", Uuid::new_v4().simple());
    let session = Session {
        cwd,
        system_prompt: params
            .get("systemPrompt")
            .and_then(Value::as_str)
            .map(str::to_string),
        model_id: default_model.id.clone(),
        effort: default_model.efforts().next().map(str::to_string),
        permission_mode: PermissionMode::Default,
        conversation_id: None,
        cancel_tx: None,
        busy: false,
        turn_sequence: 0,
        accumulated_input_tokens: 0,
        accumulated_output_tokens: 0,
        accumulated_cached_input_tokens: 0,
        accumulated_total_tokens: 0,
    };
    let response = session_configuration(&catalog, &session, &session_id);
    app.sessions.lock().await.insert(session_id, session);
    wire::send(&wire_tx, wire::ok(id, response)).await;
}

fn session_configuration(catalog: &ModelCatalog, session: &Session, session_id: &str) -> Value {
    let model_options = catalog
        .families
        .iter()
        .map(|family| json!({ "value": family.id, "displayName": family.label }))
        .collect::<Vec<_>>();
    let mut options = vec![json!({
        "id": "model",
        "configId": "model",
        "category": "model",
        "type": "select",
        "displayName": "Model",
        "currentValue": session.model_id,
        "options": model_options
    })];
    if let Some(family) = catalog.family(&session.model_id) {
        let efforts = family.efforts().collect::<Vec<_>>();
        if !efforts.is_empty() {
            options.push(json!({
                "id": "thought-level",
                "configId": "thought-level",
                "category": "thought_level",
                "type": "select",
                "displayName": "Reasoning effort",
                "description": "Only the values currently reported by Antigravity for this model.",
                "currentValue": session.effort,
                "options": efforts.into_iter().map(|effort| json!({
                    "value": effort,
                    "displayName": title_case(effort)
                })).collect::<Vec<_>>()
            }));
        }
    }
    options.push(json!({
        "id": "mode",
        "configId": "mode",
        "category": "mode",
        "type": "select",
        "displayName": "Tool permission mode",
        "currentValue": session.permission_mode.wire_value(),
        "options": [
            { "value": "default", "displayName": "Ask when needed" },
            { "value": "acceptEdits", "displayName": "Accept edits" },
            { "value": "bypassPermissions", "displayName": "Full access" },
            { "value": "plan", "displayName": "Plan only" }
        ]
    }));

    json!({
        "sessionId": session_id,
        "configOptions": options,
        "modes": {
            "currentModeId": session.permission_mode.wire_value(),
            "availableModes": [
                { "id": "default", "name": "Ask when needed" },
                { "id": "acceptEdits", "name": "Accept edits" },
                { "id": "bypassPermissions", "name": "Full access" },
                { "id": "plan", "name": "Plan only" }
            ]
        }
    })
}

async fn set_config_option(app: &Arc<App>, id: Value, params: Value, wire_tx: &wire::Sender) {
    let Some(session_id) = params.get("sessionId").and_then(Value::as_str) else {
        reject(wire_tx, id, "session/set_config_option requires sessionId").await;
        return;
    };
    let Some(config_id) = params.get("configId").and_then(Value::as_str) else {
        reject(wire_tx, id, "session/set_config_option requires configId").await;
        return;
    };
    let Some(value) = params.get("value").and_then(Value::as_str) else {
        reject(
            wire_tx,
            id,
            "Antigravity configuration values must be strings",
        )
        .await;
        return;
    };
    let catalog = match app.model_catalog().await {
        Ok(catalog) => catalog,
        Err(error) => {
            wire::send(wire_tx, wire::error(id, wire::INTERNAL_ERROR, error)).await;
            return;
        }
    };
    let mut sessions = app.sessions.lock().await;
    let Some(session) = sessions.get_mut(session_id) else {
        reject(wire_tx, id, "unknown Antigravity session").await;
        return;
    };
    let validation_error = match config_id {
        "model" => match catalog.family(value) {
            Some(family) => {
                session.model_id = family.id.clone();
                session.effort = family.efforts().next().map(str::to_string);
                session.conversation_id = None;
                None
            }
            None => Some(format!("model `{value}` is no longer available")),
        },
        "thought-level" => match catalog.family(&session.model_id) {
            Some(family) if family.efforts().any(|effort| effort == value) => {
                session.effort = Some(value.to_string());
                session.conversation_id = None;
                None
            }
            _ => Some(format!(
                "reasoning effort `{value}` is not available for model `{}`",
                session.model_id
            )),
        },
        "mode" => match PermissionMode::from_wire(value) {
            Some(mode) => {
                session.permission_mode = mode;
                None
            }
            None => Some(format!("permission mode `{value}` is not supported")),
        },
        _ => Some(format!("unknown Antigravity option `{config_id}`")),
    };
    if let Some(error) = validation_error {
        drop(sessions);
        reject(wire_tx, id, &error).await;
        return;
    }
    let response = session_configuration(&catalog, session, session_id);
    drop(sessions);
    wire::send(wire_tx, wire::ok(id, response)).await;
}

async fn set_model(app: &Arc<App>, id: Value, params: Value, wire_tx: &wire::Sender) {
    let Some(session_id) = params.get("sessionId").and_then(Value::as_str) else {
        reject(wire_tx, id, "session/set_model requires sessionId").await;
        return;
    };
    let Some(model_id) = params.get("modelId").and_then(Value::as_str) else {
        reject(wire_tx, id, "session/set_model requires modelId").await;
        return;
    };
    set_config_option(
        app,
        id,
        json!({ "sessionId": session_id, "configId": "model", "value": model_id }),
        wire_tx,
    )
    .await;
}

async fn session_prompt(app: Arc<App>, id: Value, params: Value, wire_tx: wire::Sender) {
    let Some(session_id) = params
        .get("sessionId")
        .and_then(Value::as_str)
        .map(str::to_string)
    else {
        reject(&wire_tx, id, "session/prompt requires sessionId").await;
        return;
    };
    let prompt = match prompt_to_text(params.get("prompt")) {
        Ok(prompt) if !prompt.trim().is_empty() => prompt,
        Ok(_) => {
            reject(&wire_tx, id, "session/prompt prompt must not be empty").await;
            return;
        }
        Err(error) => {
            reject(&wire_tx, id, &error).await;
            return;
        }
    };
    let snapshot = match acquire_prompt(&app, &session_id).await {
        Ok(snapshot) => snapshot,
        Err(error) => {
            reject(&wire_tx, id, &error).await;
            return;
        }
    };
    let catalog = match app.model_catalog().await {
        Ok(catalog) => catalog,
        Err(error) => {
            finish_prompt(&app, &session_id, None, None).await;
            wire::send(&wire_tx, wire::error(id, wire::INTERNAL_ERROR, error)).await;
            return;
        }
    };
    let Some(model) = catalog.family(&snapshot.model_id) else {
        finish_prompt(&app, &session_id, None, None).await;
        reject(&wire_tx, id, "selected Antigravity model is unavailable").await;
        return;
    };
    let outcome = agy::run_turn(
        RunRequest {
            agy_command: &app.agy_command,
            session_id: &session_id,
            turn_id: &snapshot.turn_id,
            cwd: &snapshot.cwd,
            system_prompt: snapshot.system_prompt.as_deref(),
            prompt: &prompt,
            model,
            effort: snapshot.effort.as_deref(),
            permission_mode: snapshot.permission_mode,
            conversation_id: snapshot.conversation_id.as_deref(),
        },
        snapshot.cancel_rx,
        &wire_tx,
    )
    .await;

    match outcome {
        Ok(outcome) => {
            let usage = finish_prompt(
                &app,
                &session_id,
                outcome.conversation_id,
                Some(outcome.usage),
            )
            .await;
            if let Some(usage) = usage {
                wire::send(
                    &wire_tx,
                    wire::usage_update(
                        &session_id,
                        json!({
                            "sessionUpdate": "usage_update",
                            "used": usage.total_tokens,
                            "contextLimit": 0,
                            "accumulatedInputTokens": usage.input_tokens,
                            "accumulatedOutputTokens": usage.output_tokens,
                            "accumulatedCachedInputTokens": usage.cache_read_tokens,
                            "accumulatedTotalTokens": usage.total_tokens,
                            "model": snapshot.model_id
                        }),
                    ),
                )
                .await;
            }
            wire::send(&wire_tx, wire::ok(id, json!({ "stopReason": "end_turn" }))).await;
        }
        Err(RunError::Cancelled) => {
            finish_prompt(&app, &session_id, None, None).await;
            wire::send(&wire_tx, wire::ok(id, json!({ "stopReason": "cancelled" }))).await;
        }
        Err(RunError::Failed(error)) => {
            finish_prompt(&app, &session_id, None, None).await;
            wire::send(&wire_tx, wire::error(id, wire::INTERNAL_ERROR, error)).await;
        }
    }
}

async fn acquire_prompt(app: &Arc<App>, session_id: &str) -> Result<PromptSnapshot, String> {
    let mut sessions = app.sessions.lock().await;
    let session = sessions
        .get_mut(session_id)
        .ok_or_else(|| "unknown Antigravity session".to_string())?;
    if session.busy {
        return Err("Antigravity session is already handling a prompt".to_string());
    }
    let (cancel_tx, cancel_rx) = watch::channel(false);
    session.cancel_tx = Some(cancel_tx);
    session.busy = true;
    session.turn_sequence = session.turn_sequence.saturating_add(1);
    Ok(PromptSnapshot {
        cwd: session.cwd.clone(),
        system_prompt: session.system_prompt.clone(),
        model_id: session.model_id.clone(),
        effort: session.effort.clone(),
        permission_mode: session.permission_mode,
        conversation_id: session.conversation_id.clone(),
        cancel_rx,
        turn_id: format!("{session_id}-{}", session.turn_sequence),
    })
}

#[derive(Debug)]
struct AccumulatedUsage {
    input_tokens: u64,
    output_tokens: u64,
    cache_read_tokens: u64,
    total_tokens: u64,
}

async fn finish_prompt(
    app: &Arc<App>,
    session_id: &str,
    conversation_id: Option<String>,
    usage: Option<agy::Usage>,
) -> Option<AccumulatedUsage> {
    let mut sessions = app.sessions.lock().await;
    let session = sessions.get_mut(session_id)?;
    session.busy = false;
    session.cancel_tx = None;
    if let Some(conversation_id) = conversation_id {
        session.conversation_id = Some(conversation_id);
    }
    let usage = usage?;
    session.accumulated_input_tokens = session
        .accumulated_input_tokens
        .saturating_add(usage.input_tokens);
    session.accumulated_output_tokens = session
        .accumulated_output_tokens
        .saturating_add(usage.output_tokens);
    session.accumulated_cached_input_tokens = session
        .accumulated_cached_input_tokens
        .saturating_add(usage.cache_read_tokens);
    session.accumulated_total_tokens = session
        .accumulated_total_tokens
        .saturating_add(usage.total_tokens);
    Some(AccumulatedUsage {
        input_tokens: session.accumulated_input_tokens,
        output_tokens: session.accumulated_output_tokens,
        cache_read_tokens: session.accumulated_cached_input_tokens,
        total_tokens: session.accumulated_total_tokens,
    })
}

async fn cancel_session(app: &Arc<App>, params: &Value) {
    let Some(session_id) = params.get("sessionId").and_then(Value::as_str) else {
        return;
    };
    if let Some(session) = app.sessions.lock().await.get_mut(session_id) {
        if let Some(cancel_tx) = session.cancel_tx.take() {
            let _ = cancel_tx.send(true);
        }
    }
}

async fn close_session(app: &Arc<App>, id: Value, params: Value, wire_tx: &wire::Sender) {
    let Some(session_id) = params.get("sessionId").and_then(Value::as_str) else {
        reject(wire_tx, id, "session/close requires sessionId").await;
        return;
    };
    if let Some(mut session) = app.sessions.lock().await.remove(session_id) {
        if let Some(cancel_tx) = session.cancel_tx.take() {
            let _ = cancel_tx.send(true);
        }
    }
    wire::send(wire_tx, wire::ok(id, Value::Null)).await;
}

fn prompt_to_text(raw: Option<&Value>) -> Result<String, String> {
    let blocks = raw
        .and_then(Value::as_array)
        .ok_or_else(|| "session/prompt requires a prompt array".to_string())?;
    let mut parts = Vec::new();
    for block in blocks {
        match block.get("type").and_then(Value::as_str) {
            Some("text") => {
                if let Some(text) = block.get("text").and_then(Value::as_str) {
                    parts.push(text.to_string());
                }
            }
            Some("resource_link") => {
                if let Some(uri) = block.get("uri").and_then(Value::as_str) {
                    parts.push(format!("Attached resource: {uri}"));
                }
            }
            _ => {}
        }
    }
    Ok(parts.join("\n\n"))
}

async fn reject(wire_tx: &wire::Sender, id: Value, message: &str) {
    wire::send(wire_tx, wire::error(id, wire::INVALID_PARAMS, message)).await;
}

fn title_case(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{ModelFamily, ModelVariant};
    use std::path::Path;

    fn catalog() -> ModelCatalog {
        ModelCatalog {
            families: vec![
                ModelFamily {
                    id: "gemini-3.6-flash".to_string(),
                    label: "Gemini 3.6 Flash".to_string(),
                    variants: vec![
                        ModelVariant {
                            effort: "high".to_string(),
                            raw_id: "gemini-3.6-flash-high".to_string(),
                        },
                        ModelVariant {
                            effort: "low".to_string(),
                            raw_id: "gemini-3.6-flash-low".to_string(),
                        },
                    ],
                    raw_id: "gemini-3.6-flash".to_string(),
                },
                ModelFamily {
                    id: "claude-opus-4-6-thinking".to_string(),
                    label: "Claude Opus 4.6 (Thinking)".to_string(),
                    variants: Vec::new(),
                    raw_id: "claude-opus-4-6-thinking".to_string(),
                },
            ],
        }
    }

    fn session(model_id: &str, effort: Option<&str>) -> Session {
        Session {
            cwd: Path::new("/tmp").to_path_buf(),
            system_prompt: None,
            model_id: model_id.to_string(),
            effort: effort.map(str::to_string),
            permission_mode: PermissionMode::Default,
            conversation_id: None,
            cancel_tx: None,
            busy: false,
            turn_sequence: 0,
            accumulated_input_tokens: 0,
            accumulated_output_tokens: 0,
            accumulated_cached_input_tokens: 0,
            accumulated_total_tokens: 0,
        }
    }

    #[test]
    fn configuration_exposes_model_specific_thought_levels_without_duplicates() {
        let config =
            session_configuration(&catalog(), &session("gemini-3.6-flash", Some("high")), "s1");
        let options = config["configOptions"].as_array().unwrap();
        let model = options
            .iter()
            .find(|option| option["category"] == "model")
            .unwrap();
        assert_eq!(model["options"].as_array().unwrap().len(), 2);
        assert_eq!(model["options"][0]["value"], "gemini-3.6-flash");
        let thought = options
            .iter()
            .find(|option| option["category"] == "thought_level")
            .unwrap();
        assert_eq!(thought["options"][0]["value"], "high");
        assert_eq!(thought["options"][1]["value"], "low");
    }

    #[test]
    fn configuration_hides_thought_level_for_single_variant_model() {
        let config =
            session_configuration(&catalog(), &session("claude-opus-4-6-thinking", None), "s1");
        assert!(!config["configOptions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|option| option["category"] == "thought_level"));
    }

    #[test]
    fn prompt_blocks_keep_text_and_resource_links() {
        let prompt = prompt_to_text(Some(&json!([
            { "type": "text", "text": "hello" },
            { "type": "resource_link", "uri": "file:///tmp/a.txt" }
        ])))
        .unwrap();
        assert_eq!(prompt, "hello\n\nAttached resource: file:///tmp/a.txt");
    }
}
