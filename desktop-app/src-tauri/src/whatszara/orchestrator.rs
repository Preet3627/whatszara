use super::llm::{self, LLMMessage, ProviderRegistry};
use super::policy::{ContactMode, PolicyEngine};
use super::actions::{ShellExecutor, AppLauncher, MediaController, DesktopScanner, ActionResult};
use super::undo::{ActionJournal, ReverseAction};
use serde_json;
use std::collections::HashMap;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PendingAction {
    pub id: String,
    pub action: String,
    pub params: HashMap<String, String>,
    pub risk_level: String,
    pub requires: Vec<String>,
    pub contact_jid: String,
    pub reason: String,
}

#[derive(Debug)]
struct ToolCall {
    action: String,
    params: HashMap<String, String>,
}

fn parse_tool_call(content: &str) -> Option<ToolCall> {
    // Look for JSON block like: {"tool":"execute_shell","params":{"command":"ls -la"}}
    if let Some(start) = content.rfind("{\"tool\"") {
        if let Some(end) = content[start..].find('}') {
            let json_str = &content[start..=start + end];
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) {
                let action = val["tool"].as_str()?.to_string();
                let params = val["params"].as_object()?;
                let mut map = HashMap::new();
                for (k, v) in params {
                    if let Some(s) = v.as_str() {
                        map.insert(k.clone(), s.to_string());
                    }
                }
                return Some(ToolCall { action, params: map });
            }
        }
    }
    // Fallback: look for `tool: action, params: {...}` pattern
    if let Some(start) = content.rfind("tool:") {
        let after = &content[start + 5..];
        let action = after.split(',').next()?.trim().to_string();
        if let Some(pstart) = after.find("params:") {
            let psection = &after[pstart + 7..];
            if let Some(pend) = psection.find('}') {
                let pjson = &psection[..=pend];
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(pjson) {
                    let mut map = HashMap::new();
                    if let Some(obj) = val.as_object() {
                        for (k, v) in obj {
                            if let Some(s) = v.as_str() {
                                map.insert(k.clone(), s.to_string());
                            }
                        }
                    }
                    return Some(ToolCall { action, params: map });
                }
            }
        }
    }
    None
}

pub struct WhatszaraOrchestrator {
    pub providers: ProviderRegistry,
    pub policy: PolicyEngine,
    pub shell: ShellExecutor,
    pub actions_journal: ActionJournal,
    pending_actions: Vec<PendingAction>,
    action_counter: u64,
    pub auto_read_enabled: bool,
    pub last_rowid: i64,
}

impl WhatszaraOrchestrator {
    pub fn new() -> Self {
        Self {
            providers: ProviderRegistry::new(),
            policy: PolicyEngine::new(),
            shell: ShellExecutor::default(),
            actions_journal: ActionJournal::new(1000),
            pending_actions: Vec::new(),
            action_counter: 0,
            auto_read_enabled: false,
            last_rowid: 0,
        }
    }

    pub fn pending_actions(&self) -> &[PendingAction] {
        &self.pending_actions
    }

    pub fn remove_pending_action(&mut self, id: &str) -> Option<PendingAction> {
        if let Some(idx) = self.pending_actions.iter().position(|a| a.id == id) {
            Some(self.pending_actions.remove(idx))
        } else {
            None
        }
    }

    pub fn register_default_providers(&mut self) {
        let ollama_endpoint = std::env::var("OLLAMA_ENDPOINT").unwrap_or_else(|_| "http://localhost:11434".into());
        self.providers.register(Box::new(llm::OllamaProvider {
            endpoint: ollama_endpoint,
            model: "".into(),
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

    pub async fn process_message(&mut self, message: &str, contact_jid: &str) -> Result<serde_json::Value, String> {
        let mode = self.policy.get_contact_mode(contact_jid);

        match mode {
            ContactMode::Blocked => {
                return Ok(serde_json::json!({
                    "success": false,
                    "error": "This contact is blocked",
                    "contact_mode": "blocked",
                }));
            }
            ContactMode::Summarize => {
                let history = vec![
                    LLMMessage { role: "user".into(), content: message.into() },
                ];
                let system = "Summarize the following WhatsApp message concisely in 2-3 sentences. \
                    Do not execute any actions. Only return a summary of the message.";

                return match self.providers.chat(&history, Some(system)).await {
                    Ok(resp) => Ok(serde_json::json!({
                        "success": true,
                        "content": resp.content,
                        "model": resp.model,
                        "provider": resp.provider,
                        "contact_mode": "summarize",
                    })),
                    Err(e) => Ok(serde_json::json!({
                        "success": false,
                        "error": e,
                        "contact_mode": "summarize",
                    })),
                };
            }
            ContactMode::Chat => {
                let history = vec![
                    LLMMessage { role: "user".into(), content: message.into() },
                ];
                let system = "You are Whatszara, a WhatsApp-connected AI assistant. \
                    You can only chat and answer questions. You cannot execute any desktop actions. \
                    Respond helpfully and concisely.";

                return match self.providers.chat(&history, Some(system)).await {
                    Ok(resp) => Ok(serde_json::json!({
                        "success": true,
                        "content": resp.content,
                        "model": resp.model,
                        "provider": resp.provider,
                        "contact_mode": "chat",
                    })),
                    Err(e) => Ok(serde_json::json!({
                        "success": false,
                        "error": e,
                        "contact_mode": "chat",
                    })),
                };
            }
            ContactMode::Assistant => {
                let system = "You are Whatszara, a desktop assistant controlled via WhatsApp. \
                    You can execute shell commands, open applications, control volume and media playback, \
                    and list files on the desktop. Available tools: execute_shell, open_app, get_volume, \
                    set_volume, play_music, pause_music, next_track, prev_track, list_images, get_desktop_paths. \
                    Respond concisely and report results clearly. \
                    To execute a tool, include JSON in your response like: \
                    {\"tool\":\"execute_shell\",\"params\":{\"command\":\"ls -la\"}} \
                    Only use tools listed above. Never execute dangerous commands.";

                let history = vec![
                    LLMMessage { role: "user".into(), content: message.into() },
                ];

                return match self.providers.chat(&history, Some(system)).await {
                    Ok(resp) => {
                        let content = resp.content.clone();
                        let mut result = serde_json::json!({
                            "success": true,
                            "content": content,
                            "model": resp.model,
                            "provider": resp.provider,
                            "contact_mode": "assistant",
                            "has_pending_actions": false,
                            "auto_executed": false,
                        });

                        if let Some(tool_call) = parse_tool_call(&resp.content) {
                            let (_proposal, decision) = self.policy.propose(
                                &tool_call.action,
                                tool_call.params.clone(),
                                contact_jid,
                                "AI-triggered action from Assistant mode",
                            );
                            if decision.allowed {
                                let action_result = self.execute_action(&tool_call.action, &tool_call.params).await;
                                let reverse = self.build_reverse(&tool_call.action, &tool_call.params, &action_result);
                                self.actions_journal.record(
                                    &tool_call.action, tool_call.params,
                                    serde_json::to_value(&action_result).unwrap_or_default(),
                                    reverse, &decision.risk_level, contact_jid,
                                );
                                result["auto_executed"] = serde_json::json!(true);
                                result["action_result"] = serde_json::to_value(&action_result).unwrap_or_default();
                            } else if !decision.requires_verification.is_empty() {
                                self.action_counter += 1;
                                let pending = PendingAction {
                                    id: format!("pa_{}", self.action_counter),
                                    action: tool_call.action.clone(),
                                    params: tool_call.params.clone(),
                                    risk_level: decision.risk_level.clone(),
                                    requires: decision.requires_verification.clone(),
                                    contact_jid: contact_jid.to_string(),
                                    reason: decision.reason.clone(),
                                };
                                self.pending_actions.push(pending.clone());
                                result["has_pending_actions"] = serde_json::json!(true);
                                result["pending_action"] = serde_json::to_value(&pending).unwrap_or_default();
                            } else {
                                result["action_blocked"] = serde_json::json!(true);
                                result["block_reason"] = serde_json::json!(decision.reason);
                            }
                        }
                        Ok(result)
                    }
                    Err(e) => Ok(serde_json::json!({ "success": false, "error": e })),
                };
            }
        }
    }

    pub async fn handle_action(&mut self, action: &str, params: HashMap<String, String>, contact: &str) -> serde_json::Value {
        let reason = format!("Action requested by {}", contact);
        let (_proposal, decision) = self.policy.propose(action, params.clone(), contact, &reason);

        if !decision.allowed {
            return serde_json::json!({
                "success": false,
                "error": decision.reason,
                "requires_verification": decision.requires_verification,
                "risk_level": decision.risk_level,
                "action": action,
            });
        }

        let result = self.execute_action(action, &params).await;

        let reverse = self.build_reverse(action, &params, &result);
        self.actions_journal.record(
            action, params, serde_json::to_value(&result).unwrap_or_default(),
            reverse, &decision.risk_level, contact,
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
                MediaController::play(query).await
            }
            "pause_music" => MediaController::pause().await,
            "next_track" => MediaController::next_track().await,
            "prev_track" => MediaController::prev_track().await,
            "list_images" => {
                let path = params.get("path").map(|s| s.as_str());
                DesktopScanner::list_images(path).await
            }
            "get_desktop_paths" => DesktopScanner::get_desktop_paths().await,
            _ => ActionResult { success: false, output: String::new(), error: format!("Unknown action: {}", action), action: action.to_string(), params: params.clone() },
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

    pub async fn approve_pending_action(&mut self, id: &str) -> serde_json::Value {
        let pending = match self.remove_pending_action(id) {
            Some(p) => p,
            None => return serde_json::json!({"success": false, "error": "Pending action not found"}),
        };
        let (_proposal, decision) = self.policy.propose(
            &pending.action, pending.params.clone(), &pending.contact_jid, &pending.reason,
        );
        if !decision.allowed {
            return serde_json::json!({
                "success": false, "error": decision.reason, "action": pending.action,
            });
        }
        let action_result = self.execute_action(&pending.action, &pending.params).await;
        let reverse = self.build_reverse(&pending.action, &pending.params, &action_result);
        self.actions_journal.record(
            &pending.action, pending.params,
            serde_json::to_value(&action_result).unwrap_or_default(),
            reverse, &decision.risk_level, &pending.contact_jid,
        );
        serde_json::json!({"success": true, "action_result": action_result})
    }

    pub fn reject_pending_action(&mut self, id: &str) -> serde_json::Value {
        match self.remove_pending_action(id) {
            Some(_) => serde_json::json!({"success": true, "rejected": true}),
            None => serde_json::json!({"success": false, "error": "Pending action not found"}),
        }
    }

    pub fn start_auto_read(&mut self) {
        self.auto_read_enabled = true;
    }

    pub fn stop_auto_read(&mut self) {
        self.auto_read_enabled = false;
    }

    pub fn status(&self) -> serde_json::Value {
        let names = self.providers.list_names();
        let active = names.get(self.providers.active).cloned();
        serde_json::json!({
            "active_provider": active,
            "available_providers": names,
            "journal_entries": self.actions_journal.len(),
            "reversible_actions": self.actions_journal.reversible().len(),
            "policy": self.policy.to_json(),
            "auto_read_enabled": self.auto_read_enabled,
            "last_rowid": self.last_rowid,
        })
    }
}
