use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::watch;

use crate::catalog::ModelFamily;
use crate::wire;

const INLINE_PROMPT_LIMIT: usize = 8 * 1024;
const PROCESS_EXIT_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_TOOL_OUTPUT_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionMode {
    Default,
    AcceptEdits,
    BypassPermissions,
    Plan,
}

impl PermissionMode {
    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "default" => Some(Self::Default),
            "acceptEdits" => Some(Self::AcceptEdits),
            "bypassPermissions" => Some(Self::BypassPermissions),
            "plan" => Some(Self::Plan),
            _ => None,
        }
    }

    pub fn wire_value(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::AcceptEdits => "acceptEdits",
            Self::BypassPermissions => "bypassPermissions",
            Self::Plan => "plan",
        }
    }
}

pub struct RunRequest<'a> {
    pub agy_command: &'a str,
    pub session_id: &'a str,
    pub turn_id: &'a str,
    pub cwd: &'a Path,
    pub system_prompt: Option<&'a str>,
    pub prompt: &'a str,
    pub model: &'a ModelFamily,
    pub effort: Option<&'a str>,
    pub permission_mode: PermissionMode,
    pub conversation_id: Option<&'a str>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub thinking_tokens: u64,
    pub cache_read_tokens: u64,
    pub total_tokens: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct RunOutcome {
    pub conversation_id: Option<String>,
    pub response: String,
    pub usage: Usage,
}

#[derive(Debug, PartialEq, Eq)]
pub enum RunError {
    Cancelled,
    Failed(String),
}

struct PromptPayload {
    argument: String,
    additional_dir: Option<PathBuf>,
    _file: Option<tempfile::NamedTempFile>,
}

pub async fn run_turn(
    request: RunRequest<'_>,
    mut cancel: watch::Receiver<bool>,
    wire_tx: &wire::Sender,
) -> Result<RunOutcome, RunError> {
    use std::io::{Read, Seek, SeekFrom};

    let prompt_payload = prepare_prompt(
        request.cwd,
        request.system_prompt,
        request.prompt,
        INLINE_PROMPT_LIMIT,
    )
    .map_err(RunError::Failed)?;
    let args = command_args(&request, &prompt_payload);

    let mut command = Command::new(request.agy_command);
    command.args(&args);
    command.current_dir(request.cwd);
    command.stdin(Stdio::null());
    command.stdout(Stdio::piped());
    // A short-lived `agy` updater descendant can inherit stderr. Capturing it
    // in a regular file avoids waiting forever for pipe EOF after the main CLI
    // process has already exited.
    let mut stderr_file = tempfile::tempfile().map_err(|error| {
        RunError::Failed(format!("could not capture Antigravity errors: {error}"))
    })?;
    command.stderr(stderr_file.try_clone().map_err(|error| {
        RunError::Failed(format!("could not capture Antigravity errors: {error}"))
    })?);
    command.kill_on_drop(true);
    configure_process(&mut command);

    let mut child = command.spawn().map_err(|error| {
        RunError::Failed(format!(
            "could not start Antigravity CLI `{}`: {error}",
            request.agy_command
        ))
    })?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| RunError::Failed("Antigravity stdout was not captured".to_string()))?;
    let mut lines = BufReader::new(stdout).lines();
    let mut state = StreamState::default();
    loop {
        tokio::select! {
            changed = cancel.changed() => {
                if changed.is_ok() && *cancel.borrow() {
                    terminate_process(&mut child).await;
                    return Err(RunError::Cancelled);
                }
            }
            line = lines.next_line() => {
                match line {
                    Ok(Some(line)) => process_stream_line(
                        &line,
                        request.session_id,
                        request.turn_id,
                        wire_tx,
                        &mut state,
                    ).await,
                    Ok(None) => break,
                    Err(error) => {
                        terminate_process(&mut child).await;
                        return Err(RunError::Failed(format!("could not read Antigravity output: {error}")));
                    }
                }
                if state.result_received {
                    break;
                }
            }
        }
    }

    let status = tokio::time::timeout(PROCESS_EXIT_TIMEOUT, child.wait())
        .await
        .map_err(|_| RunError::Failed("Antigravity did not exit after closing stdout".to_string()))?
        .map_err(|error| RunError::Failed(format!("could not wait for Antigravity: {error}")))?;
    stderr_file
        .seek(SeekFrom::Start(0))
        .map_err(|error| RunError::Failed(format!("could not seek Antigravity errors: {error}")))?;
    let mut stderr_bytes = Vec::new();
    stderr_file
        .take(MAX_TOOL_OUTPUT_BYTES as u64)
        .read_to_end(&mut stderr_bytes)
        .map_err(|error| RunError::Failed(format!("could not read Antigravity errors: {error}")))?;
    let stderr_text = String::from_utf8_lossy(&stderr_bytes).trim().to_string();

    if let Some(error) = state.result_error {
        return Err(RunError::Failed(error));
    }
    if !status.success() {
        return Err(RunError::Failed(if stderr_text.is_empty() {
            format!("Antigravity exited with {status}")
        } else {
            format!("Antigravity exited with {status}: {stderr_text}")
        }));
    }

    if state.streamed_response.is_empty() && !state.result_response.is_empty() {
        emit_message_chunk(
            request.session_id,
            request.turn_id,
            &state.result_response,
            wire_tx,
        )
        .await;
    }

    Ok(RunOutcome {
        conversation_id: state.conversation_id,
        response: if state.result_response.is_empty() {
            state.streamed_response
        } else {
            state.result_response
        },
        usage: state.usage,
    })
}

fn command_args(request: &RunRequest<'_>, payload: &PromptPayload) -> Vec<String> {
    let mut args = vec![
        "--output-format".to_string(),
        "stream-json".to_string(),
        "--print-timeout".to_string(),
        "30m".to_string(),
        "--disable-slash-commands".to_string(),
        "--add-dir".to_string(),
        request.cwd.display().to_string(),
    ];
    if let Some(directory) = &payload.additional_dir {
        args.push("--add-dir".to_string());
        args.push(directory.display().to_string());
    }
    let (model, effort) = request.model.command_selection(request.effort);
    args.push("--model".to_string());
    args.push(model.to_string());
    if let Some(effort) = effort {
        args.push("--effort".to_string());
        args.push(effort.to_string());
    }
    match request.permission_mode {
        PermissionMode::Default => {}
        PermissionMode::AcceptEdits => {
            args.push("--mode".to_string());
            args.push("accept-edits".to_string());
        }
        PermissionMode::BypassPermissions => {
            args.push("--dangerously-skip-permissions".to_string());
        }
        PermissionMode::Plan => {
            args.push("--mode".to_string());
            args.push("plan".to_string());
        }
    }
    if let Some(conversation_id) = request.conversation_id {
        args.push("--conversation".to_string());
        args.push(conversation_id.to_string());
    }
    args.push("--print".to_string());
    args.push(payload.argument.clone());
    args
}

fn prepare_prompt(
    cwd: &Path,
    system_prompt: Option<&str>,
    prompt: &str,
    inline_limit: usize,
) -> Result<PromptPayload, String> {
    let mut complete = format!(
        "Working directory for this session: {}\n\
         Work in that directory unless the user explicitly names another location. \
         Read and follow project instruction files such as AGENTS.md before changing files.\n",
        cwd.display()
    );
    if let Some(system_prompt) = system_prompt.filter(|value| !value.trim().is_empty()) {
        complete.push_str("\nSystem instructions:\n");
        complete.push_str(system_prompt);
        complete.push('\n');
    }
    complete.push_str("\nUser request:\n");
    complete.push_str(prompt);

    if complete.len() <= inline_limit {
        return Ok(PromptPayload {
            argument: complete,
            additional_dir: None,
            _file: None,
        });
    }

    let mut file = tempfile::Builder::new()
        .prefix("maju-antigravity-")
        .suffix(".md")
        .tempfile()
        .map_err(|error| format!("could not create Antigravity prompt file: {error}"))?;
    file.write_all(complete.as_bytes())
        .map_err(|error| format!("could not write Antigravity prompt file: {error}"))?;
    file.flush()
        .map_err(|error| format!("could not flush Antigravity prompt file: {error}"))?;
    let path = file.path().to_path_buf();
    let mention_path = path.to_string_lossy().replace('\\', "/");
    Ok(PromptPayload {
        argument: format!(
            "Read the complete UTF-8 task from @{mention_path} and carry it out. \
             Treat the file contents as the user's exact request."
        ),
        additional_dir: path.parent().map(Path::to_path_buf),
        _file: Some(file),
    })
}

#[derive(Default)]
struct StreamState {
    conversation_id: Option<String>,
    streamed_response: String,
    result_response: String,
    result_error: Option<String>,
    result_received: bool,
    usage: Usage,
}

async fn process_stream_line(
    line: &str,
    session_id: &str,
    turn_id: &str,
    wire_tx: &wire::Sender,
    state: &mut StreamState,
) {
    let Ok(value) = serde_json::from_str::<Value>(line) else {
        tracing::debug!("ignoring non-JSON Antigravity output: {line}");
        return;
    };
    match value.get("event").and_then(Value::as_str) {
        Some("init") => {
            state.conversation_id = value
                .get("conversation_id")
                .and_then(Value::as_str)
                .map(str::to_string);
        }
        Some("step_update") => {
            let update = &value["step_update"];
            match update.get("step_type").and_then(Value::as_str) {
                Some("agent_response") => {
                    if let Some(delta) = update.get("text_delta").and_then(Value::as_str) {
                        state.streamed_response.push_str(delta);
                        emit_message_chunk(session_id, turn_id, delta, wire_tx).await;
                    }
                }
                Some("tool") => emit_tool_update(session_id, update, wire_tx).await,
                _ => {}
            }
        }
        Some("result") => {
            state.result_received = true;
            let result = &value["result"];
            if let Some(conversation_id) = result.get("conversation_id").and_then(Value::as_str) {
                state.conversation_id = Some(conversation_id.to_string());
            }
            state.result_response = result
                .get("response")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            state.usage = parse_usage(result.get("usage"));
            if result.get("status").and_then(Value::as_str) == Some("ERROR") {
                state.result_error = Some(
                    result
                        .get("error")
                        .and_then(Value::as_str)
                        .unwrap_or("Antigravity reported an internal error")
                        .to_string(),
                );
            }
        }
        _ => {}
    }
}

async fn emit_message_chunk(session_id: &str, turn_id: &str, text: &str, wire_tx: &wire::Sender) {
    if text.is_empty() {
        return;
    }
    wire::send(
        wire_tx,
        wire::session_update(
            session_id,
            json!({
                "sessionUpdate": "agent_message_chunk",
                "messageId": format!("{turn_id}-message"),
                "content": { "type": "text", "text": text }
            }),
        ),
    )
    .await;
}

async fn emit_tool_update(session_id: &str, update: &Value, wire_tx: &wire::Sender) {
    let index = update
        .get("step_index")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let tool_id = format!("agy-step-{index}");
    let tool_name = update
        .get("tool_name")
        .and_then(Value::as_str)
        .unwrap_or("Antigravity tool");
    let state = update
        .get("state")
        .and_then(Value::as_str)
        .unwrap_or("ACTIVE");
    let info = &update["tool_info"];
    match state {
        "ACTIVE" => {
            wire::send(
                wire_tx,
                wire::session_update(
                    session_id,
                    json!({
                        "sessionUpdate": "tool_call",
                        "toolCallId": tool_id,
                        "title": tool_name,
                        "kind": tool_kind(tool_name),
                        "status": "in_progress",
                        "rawInput": info.get("parameters").cloned().unwrap_or(Value::Null),
                    }),
                ),
            )
            .await;
        }
        "DONE" | "ERROR" => {
            let failed = state == "ERROR";
            let output = if failed {
                info.get("error").cloned().unwrap_or(Value::Null)
            } else {
                info.get("output").cloned().unwrap_or(Value::Null)
            };
            wire::send(
                wire_tx,
                wire::session_update(
                    session_id,
                    json!({
                        "sessionUpdate": "tool_call_update",
                        "toolCallId": tool_id,
                        "status": if failed { "failed" } else { "completed" },
                        "rawOutput": clamp_json_output(output),
                    }),
                ),
            )
            .await;
        }
        _ => {}
    }
}

fn tool_kind(name: &str) -> &'static str {
    let name = name.to_ascii_lowercase();
    if name.contains("read") || name.contains("view") || name.contains("list") {
        "read"
    } else if name.contains("write") || name.contains("replace") || name.contains("edit") {
        "edit"
    } else if name.contains("delete") {
        "delete"
    } else if name.contains("search") || name.contains("grep") || name.contains("find") {
        "search"
    } else if name.contains("command") || name.contains("shell") || name.contains("execute") {
        "execute"
    } else if name.contains("browser") || name.contains("url") {
        "fetch"
    } else {
        "other"
    }
}

fn clamp_json_output(value: Value) -> Value {
    match value {
        Value::String(mut text) if text.len() > MAX_TOOL_OUTPUT_BYTES => {
            let mut cut = MAX_TOOL_OUTPUT_BYTES;
            while cut > 0 && !text.is_char_boundary(cut) {
                cut -= 1;
            }
            text.truncate(cut);
            text.push_str("\n[truncated]");
            Value::String(text)
        }
        other => other,
    }
}

fn parse_usage(value: Option<&Value>) -> Usage {
    let value = value.unwrap_or(&Value::Null);
    Usage {
        input_tokens: value
            .get("input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or_default(),
        output_tokens: value
            .get("output_tokens")
            .and_then(Value::as_u64)
            .unwrap_or_default(),
        thinking_tokens: value
            .get("thinking_tokens")
            .and_then(Value::as_u64)
            .unwrap_or_default(),
        cache_read_tokens: value
            .get("cache_read_tokens")
            .and_then(Value::as_u64)
            .unwrap_or_default(),
        total_tokens: value
            .get("total_tokens")
            .and_then(Value::as_u64)
            .unwrap_or_default(),
    }
}

fn configure_process(command: &mut Command) {
    #[cfg(unix)]
    {
        command.process_group(0);
    }
    #[cfg(windows)]
    // Do not set CREATE_NO_WINDOW here. The `agy` launcher can stop emitting
    // output when that flag is applied to it directly. The adapter is already
    // launched as a hidden Tauri sidecar, so no console window is exposed.
    let _ = command;
}

async fn terminate_process(child: &mut Child) {
    let Some(pid) = child.id() else {
        return;
    };
    #[cfg(unix)]
    {
        use nix::sys::signal::{killpg, Signal};
        use nix::unistd::Pid;
        let _ = killpg(Pid::from_raw(pid as i32), Signal::SIGTERM);
    }
    #[cfg(windows)]
    {
        let mut taskkill = Command::new("taskkill");
        taskkill.args(["/PID", &pid.to_string(), "/T", "/F"]);
        taskkill.stdin(Stdio::null());
        taskkill.stdout(Stdio::null());
        taskkill.stderr(Stdio::null());
        taskkill.creation_flags(0x0800_0000);
        let _ = tokio::time::timeout(Duration::from_secs(3), taskkill.status()).await;
    }
    let _ = child.start_kill();
    let _ = tokio::time::timeout(PROCESS_EXIT_TIMEOUT, child.wait()).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{ModelFamily, ModelVariant};

    fn request<'a>(model: &'a ModelFamily, cwd: &'a Path) -> RunRequest<'a> {
        RunRequest {
            agy_command: "agy",
            session_id: "session",
            turn_id: "turn",
            cwd,
            system_prompt: None,
            prompt: "hello",
            model,
            effort: Some("low"),
            permission_mode: PermissionMode::BypassPermissions,
            conversation_id: Some("conversation"),
        }
    }

    #[test]
    fn command_keeps_prompt_small_and_separates_model_effort() {
        let cwd = Path::new("/tmp/project");
        let model = ModelFamily {
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
        };
        let payload = prepare_prompt(cwd, None, "hello", INLINE_PROMPT_LIMIT).unwrap();
        let args = command_args(&request(&model, cwd), &payload);
        assert!(args
            .windows(2)
            .any(|pair| pair == ["--model", "gemini-3.6-flash"]));
        assert!(args.windows(2).any(|pair| pair == ["--effort", "low"]));
        assert!(args
            .iter()
            .any(|arg| arg == "--dangerously-skip-permissions"));
        assert!(args
            .windows(2)
            .any(|pair| pair == ["--conversation", "conversation"]));
    }

    #[test]
    fn long_unicode_prompt_uses_utf8_file_instead_of_argv() {
        let prompt = "한글".repeat(10_000);
        let payload = prepare_prompt(Path::new("/tmp/project"), None, &prompt, 100).unwrap();
        assert!(payload.argument.len() < 512);
        assert!(payload.argument.contains('@'));
        let file = payload._file.as_ref().unwrap();
        let contents = std::fs::read_to_string(file.path()).unwrap();
        assert!(contents.contains(&prompt));
    }

    #[test]
    fn stream_parser_preserves_usage_fields() {
        let usage = parse_usage(Some(&json!({
            "input_tokens": 10,
            "output_tokens": 3,
            "thinking_tokens": 2,
            "cache_read_tokens": 7,
            "total_tokens": 13
        })));
        assert_eq!(usage.input_tokens, 10);
        assert_eq!(usage.thinking_tokens, 2);
        assert_eq!(usage.cache_read_tokens, 7);
        assert_eq!(usage.total_tokens, 13);
    }
}
