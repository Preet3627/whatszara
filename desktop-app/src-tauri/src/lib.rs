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
    store_dir: StdMutex<Option<std::path::PathBuf>>,
}

#[tauri::command]
fn get_status(state: tauri::State<OrchestratorState>) -> Result<String, String> {
    let orch = state.0.blocking_lock();
    Ok(orch.status().to_string())
}

#[tauri::command]
async fn process_message(state: tauri::State<'_, OrchestratorState>, message: String, contact: String) -> Result<String, String> {
    let mut orch = state.0.lock().await;
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
        let mut orch = state.0.lock().await;
        let result = orch.process_message(&message, &jid).await.map_err(|e| e.to_string())?;
        result["content"].as_str().unwrap_or("No response").to_string()
    };
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
    let payload = serde_json::json!({ "recipient": jid, "message": content });
    let mut req = client.post("http://localhost:8080/api/send")
        .json(&payload);
    if let Ok(key) = std::env::var("API_KEY") {
        req = req.header("Authorization", format!("Bearer {}", key));
    }
    let resp = req.send()
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
    auto_save_config(&orch);
    Ok(serde_json::json!({ "success": true }).to_string())
}

#[tauri::command]
async fn update_allowlist(
    state: tauri::State<'_, OrchestratorState>,
    action: String,
    jid: String,
) -> Result<String, String> {
    let mut orch = state.0.lock().await;
    let result = match action.as_str() {
        "add" => { orch.policy.add_to_allowlist(&jid); Ok(serde_json::json!({ "success": true }).to_string()) }
        "remove" => { orch.policy.remove_from_allowlist(&jid); Ok(serde_json::json!({ "success": true }).to_string()) }
        _ => Ok(serde_json::json!({ "success": false, "error": "Invalid action, use 'add' or 'remove'" }).to_string()),
    };
    auto_save_config(&orch);
    result
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
    auto_save_config(&orch);
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
                let db_path = state.store_dir.lock().unwrap().as_ref().map(|p| p.join("whatsapp.db"));
                if let Some(ref path) = db_path {
                    if let Ok(data) = std::fs::read(path) {
                        let _ = save_keychain(&data, "whatszara-wa-session");
                    }
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
    let _ = delete_keychain("whatszara-wa-session");
    if let Some(store) = state.store_dir.lock().unwrap().as_ref() {
        let _ = std::fs::remove_file(store.join("whatsapp.db"));
    }
    Ok(serde_json::json!({"success": true}).to_string())
}

#[tauri::command]
async fn get_pending_actions(state: tauri::State<'_, OrchestratorState>) -> Result<String, String> {
    let orch = state.0.lock().await;
    Ok(serde_json::to_string(&orch.pending_actions()).unwrap_or_default())
}

#[tauri::command]
async fn approve_action(state: tauri::State<'_, OrchestratorState>, id: String) -> Result<String, String> {
    let mut orch = state.0.lock().await;
    let result = orch.approve_pending_action(&id).await;
    Ok(result.to_string())
}

#[tauri::command]
async fn reject_action(state: tauri::State<'_, OrchestratorState>, id: String) -> Result<String, String> {
    let mut orch = state.0.lock().await;
    let result = orch.reject_pending_action(&id);
    Ok(result.to_string())
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

#[tauri::command]
async fn set_ollama_endpoint(state: tauri::State<'_, OrchestratorState>, endpoint: String) -> Result<String, String> {
    let mut orch = state.0.lock().await;
    for p in &mut orch.providers.providers {
        if p.name() == "ollama" {
            p.set_endpoint(&endpoint);
            return Ok(serde_json::json!({"success": true}).to_string());
        }
    }
    Ok(serde_json::json!({"success": false, "error": "Ollama provider not found"}).to_string())
}

#[tauri::command]
async fn save_config(state: tauri::State<'_, OrchestratorState>) -> Result<String, String> {
    let policy = state.0.lock().await.policy.to_json();
    let data = serde_json::to_vec(&policy).map_err(|e| e.to_string())?;
    save_keychain(&data, "whatszara-config").ok();
    Ok(serde_json::json!({"success": true}).to_string())
}

#[tauri::command]
async fn load_config(state: tauri::State<'_, OrchestratorState>) -> Result<String, String> {
    let data = match load_keychain("whatszara-config") {
        Ok(d) => d,
        Err(_) => return Ok(serde_json::json!({"success": false, "error": "No config saved"}).to_string()),
    };
    let config: serde_json::Value = serde_json::from_slice(&data).map_err(|e| e.to_string())?;
    let mut orch = state.0.lock().await;
    if let Some(perms) = config["tool_permissions"].as_object() {
        if let Some(v) = perms.get("shell").and_then(|v| v.as_bool()) { orch.policy.tool_permissions.shell_enabled = v; }
        if let Some(v) = perms.get("file_access").and_then(|v| v.as_bool()) { orch.policy.tool_permissions.file_access_enabled = v; }
        if let Some(v) = perms.get("media_control").and_then(|v| v.as_bool()) { orch.policy.tool_permissions.media_control_enabled = v; }
        if let Some(v) = perms.get("app_launching").and_then(|v| v.as_bool()) { orch.policy.tool_permissions.app_launching_enabled = v; }
        if let Some(v) = perms.get("whatsapp").and_then(|v| v.as_bool()) { orch.policy.tool_permissions.whatsapp_enabled = v; }
    }
    if let Some(allowlist) = config["allowlist"].as_array() {
        for entry in allowlist {
            if let Some(jid) = entry.as_str() {
                orch.policy.add_to_allowlist(jid);
            }
        }
    }
    if let Some(modes) = config["contact_modes"].as_object() {
        for (jid, mode_str) in modes {
            let mode = match mode_str.as_str() {
                Some("assistant") => ContactMode::Assistant,
                Some("chat") => ContactMode::Chat,
                Some("summarize") => ContactMode::Summarize,
                Some("blocked") => ContactMode::Blocked,
                _ => continue,
            };
            orch.policy.set_contact_mode(jid, mode);
        }
    }
    Ok(serde_json::json!({"success": true}).to_string())
}

#[tauri::command]
async fn clear_config() -> Result<String, String> {
    delete_keychain("whatszara-config").ok();
    Ok(serde_json::json!({"success": true}).to_string())
}

#[tauri::command]
async fn set_api_key(state: tauri::State<'_, OrchestratorState>, provider: String, key: String) -> Result<String, String> {
    let mut orch = state.0.lock().await;
    for p in &mut orch.providers.providers {
        if p.name() == provider {
            p.set_api_key(&key);
            return Ok(serde_json::json!({"success": true}).to_string());
        }
    }
    Ok(serde_json::json!({"success": false, "error": "Provider not found"}).to_string())
}

fn save_keychain(data: &[u8], service: &str) -> Result<(), String> {
    let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, data);
    let entry = keyring::Entry::new(service, "whatszara")
        .map_err(|e| format!("Keyring entry error: {}", e))?;
    entry.set_password(&b64)
        .map_err(|e| format!("Keyring write error: {}", e))
}

fn load_keychain(service: &str) -> Result<Vec<u8>, String> {
    let entry = keyring::Entry::new(service, "whatszara")
        .map_err(|e| format!("Keyring entry error: {}", e))?;
    let b64 = entry.get_password()
        .map_err(|e| format!("Keyring read error: {}", e))?;
    base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &b64)
        .map_err(|e| format!("Base64 decode: {}", e))
}

fn delete_keychain(service: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(service, "whatszara")
        .map_err(|e| format!("Keyring entry error: {}", e))?;
    entry.delete_credential()
        .map_err(|e| format!("Keyring delete error: {}", e))
}

fn auto_save_config(orch: &WhatszaraOrchestrator) {
    if let Ok(data) = serde_json::to_vec(&orch.policy.to_json()) {
        let _ = save_keychain(&data, "whatszara-config");
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut orch = WhatszaraOrchestrator::new();
    orch.register_default_providers();
    orch.policy.add_to_allowlist("self");
    if let Ok(data) = load_keychain("whatszara-config") {
        if let Ok(config) = serde_json::from_slice::<serde_json::Value>(&data) {
            if let Some(perms) = config["tool_permissions"].as_object() {
                if let Some(v) = perms.get("shell").and_then(|v| v.as_bool()) { orch.policy.tool_permissions.shell_enabled = v; }
                if let Some(v) = perms.get("file_access").and_then(|v| v.as_bool()) { orch.policy.tool_permissions.file_access_enabled = v; }
                if let Some(v) = perms.get("media_control").and_then(|v| v.as_bool()) { orch.policy.tool_permissions.media_control_enabled = v; }
                if let Some(v) = perms.get("app_launching").and_then(|v| v.as_bool()) { orch.policy.tool_permissions.app_launching_enabled = v; }
                if let Some(v) = perms.get("whatsapp").and_then(|v| v.as_bool()) { orch.policy.tool_permissions.whatsapp_enabled = v; }
            }
            if let Some(allowlist) = config["allowlist"].as_array() {
                for entry in allowlist {
                    if let Some(jid) = entry.as_str() {
                        orch.policy.add_to_allowlist(jid);
                    }
                }
            }
            if let Some(modes) = config["contact_modes"].as_object() {
                for (jid, mode_str) in modes {
                    let mode = match mode_str.as_str() {
                        Some("assistant") => ContactMode::Assistant,
                        Some("chat") => ContactMode::Chat,
                        Some("summarize") => ContactMode::Summarize,
                        Some("blocked") => ContactMode::Blocked,
                        _ => continue,
                    };
                    orch.policy.set_contact_mode(jid, mode);
                }
            }
        }
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(OrchestratorState(Mutex::new(orch)))
        .manage(Arc::new(BridgeProcess {
            child: StdMutex::new(None),
            qr_code: StdMutex::new(String::new()),
            output: StdMutex::new(String::new()),
            store_dir: StdMutex::new(None),
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
            set_ollama_endpoint,
            save_config,
            load_config,
            clear_config,
            set_api_key,
            get_pending_actions,
            approve_action,
            reject_action,
        ])
        .setup(|app| {
            #[cfg(desktop)]
            {
                // Determine bridge binary, args, and working directory
                let (bridge_cmd, bridge_args, bridge_cwd): (String, Vec<String>, std::path::PathBuf) = if cfg!(debug_assertions) {
                    let repo_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("../../whatsapp-bridge")
                        .canonicalize()
                        .unwrap_or_else(|_| std::path::PathBuf::from("../whatsapp-bridge"));
                    ("go".into(), vec!["run".into(), "main.go".into()], repo_path)
                } else {
                    let exe_name = if cfg!(target_os = "windows") { "whatsapp-bridge-windows.exe" }
                        else if cfg!(target_os = "linux") { "whatsapp-bridge-linux" }
                        else { "whatsapp-bridge-darwin" };
                    let exe = app.path().resource_dir()
                        .map(|p| p.join("bin").join(exe_name))
                        .unwrap_or_else(|_| std::path::PathBuf::from(exe_name));
                    let data_dir = app.path().app_data_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
                    (exe.to_string_lossy().to_string(), vec![], data_dir)
                };

                // Restore session from keychain
                if let Ok(data) = load_keychain("whatszara-wa-session") {
                    let store_dir = bridge_cwd.join("store");
                    let _ = std::fs::create_dir_all(&store_dir);
                    let _ = std::fs::write(store_dir.join("whatsapp.db"), &data);
                }

                let bridge_state = app.state::<Arc<BridgeProcess>>();
                {
                    let mut store = bridge_state.store_dir.lock().unwrap();
                    *store = Some(bridge_cwd.join("store"));
                }
                match Command::new(&bridge_cmd)
                    .args(&bridge_args)
                    .current_dir(&bridge_cwd)
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
