mod whatszara;

use std::process::{Command, Child, Stdio};
use std::sync::Mutex as StdMutex;
use tauri::Manager;
use whatszara::orchestrator::WhatszaraOrchestrator;
use whatszara::policy::ContactMode;
use tokio::sync::Mutex;

struct OrchestratorState(Mutex<WhatszaraOrchestrator>);
struct BridgeProcess(StdMutex<Option<Child>>);

#[tauri::command]
fn get_status(state: tauri::State<OrchestratorState>) -> Result<String, String> {
    let orch = state.0.blocking_lock();
    Ok(orch.status().to_string())
}

#[tauri::command]
async fn process_message(state: tauri::State<'_, OrchestratorState>, message: String, contact: String) -> Result<String, String> {
    let orch = state.0.lock().await;
    match orch.process_message(&message, &contact).await {
        Ok(r) => Ok(r.to_string()),
        Err(e) => Ok(serde_json::json!({ "success": false, "error": e }).to_string()),
    }
}

#[tauri::command]
async fn handle_action(
    state: tauri::State<'_, OrchestratorState>,
    action: String,
    params: String,
    contact: String,
) -> Result<String, String> {
    let params_map: std::collections::HashMap<String, String> =
        serde_json::from_str(&params).unwrap_or_default();
    let mut orch = state.0.lock().await;
    Ok(orch.handle_action(&action, params_map, &contact).await.to_string())
}

#[tauri::command]
async fn undo_last(state: tauri::State<'_, OrchestratorState>, contact: String) -> Result<String, String> {
    let mut orch = state.0.lock().await;
    Ok(orch.undo_last(&contact).await.to_string())
}

#[tauri::command]
fn list_providers(state: tauri::State<OrchestratorState>) -> Result<String, String> {
    let orch = state.0.blocking_lock();
    Ok(serde_json::json!(orch.providers.list_names()).to_string())
}

#[tauri::command]
async fn list_models(state: tauri::State<'_, OrchestratorState>) -> Result<String, String> {
    let orch = state.0.lock().await;
    let models = orch.providers.list_all_models().await;
    Ok(serde_json::json!(models).to_string())
}

#[tauri::command]
async fn set_active_provider(state: tauri::State<'_, OrchestratorState>, name: String) -> Result<String, String> {
    let mut orch = state.0.lock().await;
    match orch.providers.set_active(&name) {
        Ok(()) => Ok(serde_json::json!({ "success": true }).to_string()),
        Err(e) => Ok(serde_json::json!({ "success": false, "error": e }).to_string()),
    }
}

#[tauri::command]
fn list_chats(limit: usize) -> Result<String, String> {
    match whatszara::whatsapp::list_chats(limit) {
        Ok(chats) => Ok(serde_json::to_string(&chats).unwrap_or_default()),
        Err(e) => Ok(serde_json::json!({ "error": e }).to_string()),
    }
}

#[tauri::command]
fn search_contacts(query: String) -> Result<String, String> {
    match whatszara::whatsapp::search_contacts(&query) {
        Ok(contacts) => Ok(serde_json::to_string(&contacts).unwrap_or_default()),
        Err(e) => Ok(serde_json::json!({ "error": e }).to_string()),
    }
}

#[tauri::command]
fn get_policy(state: tauri::State<OrchestratorState>) -> Result<String, String> {
    let orch = state.0.blocking_lock();
    Ok(orch.policy.to_json().to_string())
}

#[tauri::command]
async fn update_permissions(
    state: tauri::State<'_, OrchestratorState>,
    shell: Option<bool>,
    file_access: Option<bool>,
    media_control: Option<bool>,
    app_launching: Option<bool>,
    whatsapp: Option<bool>,
) -> Result<String, String> {
    let mut orch = state.0.lock().await;
    if let Some(v) = shell { orch.policy.tool_permissions.shell_enabled = v; }
    if let Some(v) = file_access { orch.policy.tool_permissions.file_access_enabled = v; }
    if let Some(v) = media_control { orch.policy.tool_permissions.media_control_enabled = v; }
    if let Some(v) = app_launching { orch.policy.tool_permissions.app_launching_enabled = v; }
    if let Some(v) = whatsapp { orch.policy.tool_permissions.whatsapp_enabled = v; }
    Ok(serde_json::json!({ "success": true }).to_string())
}

#[tauri::command]
async fn update_allowlist(
    state: tauri::State<'_, OrchestratorState>,
    action: String,
    jid: String,
) -> Result<String, String> {
    let mut orch = state.0.lock().await;
    match action.as_str() {
        "add" => { orch.policy.add_to_allowlist(&jid); Ok(serde_json::json!({ "success": true }).to_string()) }
        "remove" => { orch.policy.remove_from_allowlist(&jid); Ok(serde_json::json!({ "success": true }).to_string()) }
        _ => Ok(serde_json::json!({ "success": false, "error": "Invalid action, use 'add' or 'remove'" }).to_string()),
    }
}

#[tauri::command]
async fn update_contact_mode(
    state: tauri::State<'_, OrchestratorState>,
    jid: String,
    mode: String,
) -> Result<String, String> {
    let contact_mode = match mode.as_str() {
        "assistant" => ContactMode::Assistant,
        "chat" => ContactMode::Chat,
        "summarize" => ContactMode::Summarize,
        "blocked" => ContactMode::Blocked,
        _ => return Ok(serde_json::json!({ "success": false, "error": "Invalid mode, use: assistant, chat, summarize, blocked" }).to_string()),
    };
    let mut orch = state.0.lock().await;
    orch.policy.set_contact_mode(&jid, contact_mode);
    Ok(serde_json::json!({ "success": true }).to_string())
}

#[tauri::command]
async fn check_bridge(state: tauri::State<'_, BridgeProcess>) -> Result<String, String> {
    let alive = {
        let mut guard = state.0.lock().unwrap();
        if guard.is_none() {
            return Ok(serde_json::json!({"status": "stopped"}).to_string());
        }
        let mut child = guard.take().unwrap();
        match child.try_wait() {
            Ok(Some(status)) => {
                return Ok(serde_json::json!({
                    "status": "error",
                    "error": format!("Bridge exited with code {:?}", status.code())
                }).to_string());
            }
            Ok(None) => {
                *guard = Some(child);
                true
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Ok(serde_json::json!({
                    "status": "error",
                    "error": e.to_string()
                }).to_string());
            }
        }
    };

    if alive {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(3))
            .build()
            .map_err(|e| e.to_string())?;
        let connected = client.get("http://localhost:8080/api/send").send().await.is_ok();
        let status = if connected { "connected" } else { "running" };
        Ok(serde_json::json!({"status": status}).to_string())
    } else {
        Ok(serde_json::json!({"status": "stopped"}).to_string())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut orch = WhatszaraOrchestrator::new();
    orch.register_default_providers();
    orch.policy.add_to_allowlist("self");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(OrchestratorState(Mutex::new(orch)))
        .manage(BridgeProcess(StdMutex::new(None)))
        .invoke_handler(tauri::generate_handler![
            get_status,
            process_message,
            handle_action,
            undo_last,
            list_providers,
            list_models,
            set_active_provider,
            list_chats,
            search_contacts,
            get_policy,
            update_permissions,
            update_allowlist,
            update_contact_mode,
            check_bridge,
        ])
        .setup(|app| {
            #[cfg(desktop)]
            {
                let bridge_dir = app.path().resource_dir()
                    .map(|p| p.join("../../whatsapp-bridge"))
                    .unwrap_or_else(|_| std::path::PathBuf::from("../whatsapp-bridge"));

                let bridge_path = if cfg!(debug_assertions) {
                    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("../../whatsapp-bridge")
                        .canonicalize()
                        .unwrap_or(bridge_dir.clone())
                } else {
                    bridge_dir
                };

                if bridge_path.join("main.go").exists() {
                    let child = Command::new("go")
                        .args(["run", "main.go"])
                        .current_dir(&bridge_path)
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .spawn()
                        .ok();
                    let state = app.state::<BridgeProcess>();
                    *state.0.lock().unwrap() = child;
                }

                use tauri::tray::{TrayIconBuilder, TrayIconEvent};
                let _tray = TrayIconBuilder::new()
                    .tooltip("Whatszara")
                    .on_tray_icon_event(|tray, event| {
                        let TrayIconEvent::Click { button, button_state, .. } = event else {
                            return;
                        };
                        if button == tauri::tray::MouseButton::Left
                            && button_state == tauri::tray::MouseButtonState::Up
                        {
                            let app = tray.app_handle();
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    })
                    .build(app)?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
