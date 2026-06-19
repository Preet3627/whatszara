mod whatszara;

use tauri::Manager;
use whatszara::orchestrator::WhatszaraOrchestrator;
use tokio::sync::Mutex;

struct OrchestratorState(Mutex<WhatszaraOrchestrator>);

#[tauri::command]
fn get_status(state: tauri::State<OrchestratorState>) -> Result<String, String> {
    let orch = state.0.blocking_lock();
    Ok(orch.status().to_string())
}

#[tauri::command]
async fn process_message(state: tauri::State<'_, OrchestratorState>, message: String) -> Result<String, String> {
    let message_copy = message.clone();
    let orch = state.0.lock().await;
    match orch.process_message(&message_copy).await {
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut orch = WhatszaraOrchestrator::new();
    orch.register_default_providers();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(OrchestratorState(Mutex::new(orch)))
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
        ])
        .setup(|app| {
            #[cfg(desktop)]
            {
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
