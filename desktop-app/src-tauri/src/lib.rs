mod whatszara;

use std::io::BufRead;
use std::process::{Command, Child, Stdio};
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use tauri::Manager;
use whatszara::orchestrator::WhatszaraOrchestrator;
use whatszara::policy::ContactMode;
use tokio::sync::Mutex;

struct OrchestratorState(Mutex<WhatszaraOrchestrator>);
struct BridgeProcess {
    child: StdMutex<Option<Child>>,
    qr_code: StdMutex<String>,
    output: StdMutex<String>,
}

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
async fn send_reply(
    state: tauri::State<'_, OrchestratorState>,
    jid: String,
    message: String,
) -> Result<String, String> {
    let content = {
        let orch = state.0.lock().await;
        let result = orch.process_message(&message, &jid).await.map_err(|e| e.to_string())?;
        result["content"].as_str().unwrap_or("No response").to_string()
    };
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
    let payload = serde_json::json!({ "recipient": jid, "message": content });
    let resp = client.post("http://localhost:8080/api/send")
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Bridge send failed: {}", e))?;
    if !resp.status().is_success() {
        return Ok(serde_json::json!({"success": false, "error": "Bridge rejected send", "reply": content}).to_string());
    }
    Ok(serde_json::json!({"success": true, "reply": content}).to_string())
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
async fn set_model(state: tauri::State<'_, OrchestratorState>, provider: String, model: String) -> Result<String, String> {
    let mut orch = state.0.lock().await;
    match orch.providers.set_model(&provider, &model) {
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
async fn check_bridge(state: tauri::State<'_, Arc<BridgeProcess>>) -> Result<String, String> {
    let alive = {
        let mut guard = state.child.lock().unwrap();
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
        let qr_code = state.qr_code.lock().unwrap().clone();
        let has_qr = !qr_code.is_empty();

        let was_connected = state.output.lock().unwrap().contains("CONNECTED_SAVED");
        let status = if connected {
            if !was_connected {
                state.output.lock().unwrap().push_str("CONNECTED_SAVED\n");
                if let Ok(data) = std::fs::read(whatszara::whatsapp::session_db_path()) {
                    let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &data);
                    let _ = std::process::Command::new("security")
                        .args(["add-generic-password", "-s", "whatszara-wa-session", "-a", "whatszara", "-w", &b64, "-U"])
                        .output();
                }
            }
            "connected"
        } else if has_qr { "awaiting_scan" } else { "running" };
        Ok(serde_json::json!({"status": status, "qr": qr_code}).to_string())
    } else {
        Ok(serde_json::json!({"status": "stopped"}).to_string())
    }
}

#[tauri::command]
async fn logout_bridge(state: tauri::State<'_, Arc<BridgeProcess>>) -> Result<String, String> {
    {
        let mut guard = state.child.lock().unwrap();
        if let Some(mut child) = guard.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
    {
        let mut output = state.output.lock().unwrap();
        output.clear();
    }
    {
        let mut qr = state.qr_code.lock().unwrap();
        qr.clear();
    }
    let _ = std::process::Command::new("security")
        .args(["delete-generic-password", "-s", "whatszara-wa-session", "-a", "whatszara"])
        .output();
    let _ = std::fs::remove_file(whatszara::whatsapp::session_db_path());
    Ok(serde_json::json!({"success": true}).to_string())
}

#[tauri::command]
fn list_contacts() -> Result<String, String> {
    match whatszara::whatsapp::list_all_contacts() {
        Ok(contacts) => Ok(serde_json::to_string(&contacts).unwrap_or_default()),
        Err(e) => Ok(serde_json::json!({ "error": e }).to_string()),
    }
}

#[tauri::command]
fn list_messages(jid: String, limit: Option<usize>) -> Result<String, String> {
    match whatszara::whatsapp::list_messages(&jid, limit.unwrap_or(50)) {
        Ok(msgs) => Ok(serde_json::to_string(&msgs).unwrap_or_default()),
        Err(e) => Ok(serde_json::json!({ "error": e }).to_string()),
    }
}

fn save_keychain(data: &[u8]) -> Result<(), String> {
    use std::process::Command;
    let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, data);
    Command::new("security")
        .args(["add-generic-password", "-s", "whatszara-wa-session", "-a", "whatszara", "-w", &b64, "-U"])
        .output()
        .map_err(|e| format!("Keychain write error: {}", e))?;
    Ok(())
}

fn load_keychain() -> Result<Vec<u8>, String> {
    use std::process::Command;
    let out = Command::new("security")
        .args(["find-generic-password", "-s", "whatszara-wa-session", "-a", "whatszara", "-w"])
        .output()
        .map_err(|e| format!("Keychain read error: {}", e))?;
    if !out.status.success() {
        return Err("No keychain entry".into());
    }
    let b64 = String::from_utf8_lossy(&out.stdout).trim().to_string();
    base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &b64)
        .map_err(|e| format!("Base64 decode: {}", e))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut orch = WhatszaraOrchestrator::new();
    orch.register_default_providers();
    orch.policy.add_to_allowlist("self");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(OrchestratorState(Mutex::new(orch)))
        .manage(Arc::new(BridgeProcess {
            child: StdMutex::new(None),
            qr_code: StdMutex::new(String::new()),
            output: StdMutex::new(String::new()),
        }))
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
            set_model,
            get_policy,
            update_permissions,
            update_allowlist,
            update_contact_mode,
            check_bridge,
            logout_bridge,
            send_reply,
            list_contacts,
            list_messages,
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
                    if let Ok(data) = load_keychain() {
                        let db_path = bridge_path.join("store").join("whatsapp.db");
                        if let Some(parent) = db_path.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        let _ = std::fs::write(&db_path, &data);
                    }

                    let bridge_state = app.state::<Arc<BridgeProcess>>();
                    match Command::new("go")
                        .args(["run", "main.go"])
                        .current_dir(&bridge_path)
                        .stdout(Stdio::piped())
                        .stderr(Stdio::inherit())
                        .spawn()
                    {
                        Ok(mut child) => {
                            if let Some(stdout) = child.stdout.take() {
                                let state_clone = Arc::clone(&*bridge_state);
                                std::thread::spawn(move || {
                                    let reader = std::io::BufReader::new(stdout);
                                    for line in reader.lines() {
                                        if let Ok(line) = line {
                                            let mut output = state_clone.output.lock().unwrap();
                                            output.push_str(&line);
                                            output.push('\n');
                                            if line.starts_with("QR_CODE:") {
                                                let code = line.trim_start_matches("QR_CODE:").to_string();
                                                *state_clone.qr_code.lock().unwrap() = code;
                                            }
                                        }
                                    }
                                });
                            }
                            *bridge_state.child.lock().unwrap() = Some(child);
                        }
                        Err(e) => {
                            let mut output = bridge_state.output.lock().unwrap();
                            output.push_str(&format!("Failed to start bridge: {}\n", e));
                        }
                    }
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
