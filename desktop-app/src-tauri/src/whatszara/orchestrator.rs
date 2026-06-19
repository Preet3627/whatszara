use super::llm::{self, LLMMessage, ProviderRegistry};
use super::permissions::{PermissionEngine, RiskLevel};
use super::actions::{ShellExecutor, AppLauncher, MediaController, DesktopScanner, ActionResult};
use super::undo::{ActionJournal, ReverseAction};
use serde_json;
use std::collections::HashMap;

pub struct WhatszaraOrchestrator {
    pub providers: ProviderRegistry,
    pub permissions: PermissionEngine,
    pub shell: ShellExecutor,
    pub actions_journal: ActionJournal,
}

impl WhatszaraOrchestrator {
    pub fn new() -> Self {
        Self {
            providers: ProviderRegistry::new(),
            permissions: PermissionEngine::new(),
            shell: ShellExecutor::default(),
            actions_journal: ActionJournal::new(1000),
        }
    }

    pub fn register_default_providers(&mut self) {
        self.providers.register(Box::new(llm::OllamaProvider {
            endpoint: "http://localhost:11434".into(),
            model: "llama3".into(),
        }));
        if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
            self.providers.register(Box::new(llm::ClaudeProvider {
                api_key: key, model: "claude-sonnet-4-20250514".into(),
            }));
        }
        if let Ok(key) = std::env::var("GROQ_API_KEY") {
            self.providers.register(Box::new(llm::GroqProvider {
                api_key: key, model: "llama-3.3-70b-versatile".into(),
            }));
        }
        if let Ok(key) = std::env::var("XAI_API_KEY") {
            self.providers.register(Box::new(llm::GrokProvider {
                api_key: key, model: "grok-3-beta".into(),
            }));
        }
        if let Ok(key) = std::env::var("GEMINI_API_KEY") {
            self.providers.register(Box::new(llm::GeminiProvider {
                api_key: key, model: "gemini-2.5-flash-001".into(),
            }));
        }
    }

    pub async fn process_message(&self, message: &str) -> Result<serde_json::Value, String> {
        let system = "You are Whatszara, a desktop assistant controlled via WhatsApp. \
            You can execute shell commands, open applications, control volume and media playback, \
            and list files on the desktop. Available tools: execute_shell, open_app, get_volume, \
            set_volume, play_music, pause_music, next_track, prev_track, list_images, get_desktop_paths. \
            Respond concisely and report results clearly.";

        let history = vec![
            LLMMessage { role: "user".into(), content: message.into() },
        ];

        match self.providers.chat(&history, Some(system)).await {
            Ok(resp) => Ok(serde_json::json!({
                "success": true,
                "content": resp.content,
                "model": resp.model,
                "provider": resp.provider,
            })),
            Err(e) => Ok(serde_json::json!({ "success": false, "error": e })),
        }
    }

    pub async fn handle_action(&mut self, action: &str, params: HashMap<String, String>, contact: &str) -> serde_json::Value {
        let mut req = self.permissions.evaluate(action, params.clone(), contact);
        let risk = req.risk_level;

        let approved = self.permissions.approve(&mut req,
            risk == RiskLevel::Low,
            if risk == RiskLevel::Low { Some(1.0) } else { None },
            risk == RiskLevel::Low,
        );

        if !approved {
            let required = req.requires_action();
            return serde_json::json!({
                "success": false,
                "error": "Action requires verification",
                "requires": required,
                "risk_level": format!("{:?}", risk).to_lowercase(),
            });
        }

        let result = self.execute_action(action, &params).await;

        let reverse = self.build_reverse(action, &params, &result);
        self.actions_journal.record(
            action, params, serde_json::to_value(&result).unwrap_or_default(),
            reverse, &format!("{:?}", risk).to_lowercase(), contact,
        );

        serde_json::to_value(&result).unwrap_or_default()
    }

    async fn execute_action(&self, action: &str, params: &HashMap<String, String>) -> ActionResult {
        match action {
            "execute_shell" => {
                let cmd = params.get("command").map(|s| s.as_str()).unwrap_or("");
                self.shell.execute(cmd).await
            }
            "open_app" => {
                let name = params.get("name").map(|s| s.as_str()).unwrap_or("");
                AppLauncher::open(name).await
            }
            "get_volume" => MediaController::get_volume().await,
            "set_volume" => {
                let level: u8 = params.get("level").and_then(|v| v.parse().ok()).unwrap_or(50);
                MediaController::set_volume(level).await
            }
            "play_music" => {
                let query = params.get("query").map(|s| s.as_str());
                MediaController::play_music(query).await
            }
            "pause_music" => MediaController::pause().await,
            "next_track" => MediaController::next_track().await,
            "prev_track" => MediaController::prev_track().await,
            "list_images" => {
                let path = params.get("path").map(|s| s.as_str());
                DesktopScanner::list_images(path).await
            }
            "get_desktop_paths" => DesktopScanner::get_desktop_paths().await,
            _ => ActionResult { success: false, output: String::new(), error: format!("Unknown action: {}", action) },
        }
    }

    fn build_reverse(&self, action: &str, _params: &HashMap<String, String>, _result: &ActionResult) -> Option<ReverseAction> {
        match action {
            "set_volume" => Some(ReverseAction {
                action: "set_volume".into(),
                params: [("level".into(), "50".into())].into(),
            }),
            "play_music" => Some(ReverseAction {
                action: "pause_music".into(),
                params: HashMap::new(),
            }),
            _ => None,
        }
    }

    pub async fn undo_last(&mut self, contact: &str) -> serde_json::Value {
        let ids: Vec<String> = self.actions_journal.reversible().iter()
            .filter(|e| e.contact_jid == contact)
            .map(|e| e.action_id.clone())
            .take(1)
            .collect();

        if ids.is_empty() {
            return serde_json::json!({ "success": false, "error": "No reversible actions found" });
        }

        let action_id = &ids[0];
        let entry = self.actions_journal.get(action_id).cloned();
        if let Some(entry) = entry {
            if let Some(ref reverse) = entry.reverse_action.clone() {
                let mut p = HashMap::new();
                for (k, v) in &reverse.params {
                    p.insert(k.clone(), v.clone());
                }
                let result = self.execute_action(&reverse.action, &p).await;
                self.actions_journal.mark_reversed(action_id);
                return serde_json::json!({
                    "success": true,
                    "undone_action": entry.action_type,
                    "reverse_result": result,
                });
            }
        }
        serde_json::json!({ "success": false, "error": "No reversible actions found" })
    }

    pub fn status(&self) -> serde_json::Value {
        let names = self.providers.list_names();
        let active = names.get(self.providers.active).cloned();
        serde_json::json!({
            "active_provider": active,
            "available_providers": names,
            "journal_entries": self.actions_journal.len(),
            "reversible_actions": self.actions_journal.reversible().len(),
        })
    }
}
