#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod models;
mod core;
use models::{BootstrapDto, CommandResult, ConnectionPageDto, CoreSessionDto, CoreSnapshotDto, EnvironmentDto, ErrorDto};
use std::sync::Mutex;
use tauri::State;

#[derive(Default)]
struct CoreState(Mutex<Option<core::ExternalCore>>);

fn valid_request_id(value: &str) -> bool {
    uuid::Uuid::parse_str(value).is_ok_and(|id| id.get_version_num() == 4)
}

#[tauri::command]
fn app_get_bootstrap(state: State<'_, CoreState>) -> CommandResult<BootstrapDto> {
    let mut boot = BootstrapDto::new();
    let guard = state.0.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_some() {
        boot.core_mode = Some("external".into());
        boot.core_state = "EXTERNAL_READONLY".into();
        boot.capabilities.external_read = true;
    }
    CommandResult::success(boot)
}

#[tauri::command]
fn system_get_environment() -> CommandResult<EnvironmentDto> {
    CommandResult::success(EnvironmentDto::inspect())
}

#[tauri::command]
async fn core_connect_external(endpoint: String, readonly: bool, request_id: String, state: State<'_, CoreState>) -> Result<CommandResult<CoreSessionDto>, String> {
    if !readonly || !valid_request_id(&request_id) {
        return Ok(CommandResult::failure(ErrorDto::new("INVALID_REQUEST", "仅支持带有效请求 ID 的只读连接", false)));
    }
    let candidate = match core::connect(&endpoint).await {
        Ok(v) => v,
        Err(e) => return Ok(CommandResult::failure(e)),
    };
    let session = CoreSessionDto { mode: "external".into(), state: "EXTERNAL_READONLY".into(), version: candidate.version.clone(), endpoint: candidate.endpoint.to_string() };
    *state.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(candidate);
    Ok(CommandResult::success(session))
}

#[tauri::command]
fn core_disconnect(request_id: String, state: State<'_, CoreState>) -> CommandResult<bool> {
    if !valid_request_id(&request_id) {
        return CommandResult::failure(ErrorDto::new("INVALID_REQUEST", "请求 ID 无效", false));
    }
    *state.0.lock().unwrap_or_else(|e| e.into_inner()) = None;
    CommandResult::success(true)
}

#[tauri::command]
async fn core_get_snapshot(state: State<'_, CoreState>) -> Result<CommandResult<CoreSnapshotDto>, String> {
    let connected = state.0.lock().unwrap_or_else(|e| e.into_inner()).clone();
    Ok(match connected {
        Some(c) => match core::snapshot(&c).await {
            Ok(s) => CommandResult::success(s),
            Err(e) => CommandResult::failure(e),
        },
        None => CommandResult::failure(ErrorDto::new("NO_CORE", "尚未连接 Mihomo 内核", false)),
    })
}

#[tauri::command]
async fn connections_list(cursor: Option<String>, limit: Option<usize>, filter: Option<String>, state: State<'_, CoreState>) -> Result<CommandResult<ConnectionPageDto>, String> {
    let offset = match cursor.as_deref().unwrap_or("0").parse::<usize>() {
        Ok(n) if n <= 100_000 => n,
        _ => return Ok(CommandResult::failure(ErrorDto::new("INVALID_CURSOR", "分页位置无效", false))),
    };
    let filter = filter.unwrap_or_default();
    if filter.chars().count() > 128 {
        return Ok(CommandResult::failure(ErrorDto::new("INVALID_FILTER", "筛选文本不能超过 128 字符", false)));
    }
    let limit = limit.unwrap_or(25).clamp(1, 100);
    let connected = state.0.lock().unwrap_or_else(|e| e.into_inner()).clone();
    Ok(match connected {
        Some(c) => match core::list_connections(&c, &filter, offset, limit).await {
            Ok(page) => CommandResult::success(page),
            Err(e) => CommandResult::failure(e),
        },
        None => CommandResult::failure(ErrorDto::new("NO_CORE", "尚未连接 Mihomo 内核", false)),
    })
}

fn main() {
    tauri::Builder::default()
        .manage(CoreState::default())
        .invoke_handler(tauri::generate_handler![app_get_bootstrap, system_get_environment, core_connect_external, core_disconnect, core_get_snapshot, connections_list])
        .run(tauri::generate_context!())
        .expect("failed to start desktop app");
}
