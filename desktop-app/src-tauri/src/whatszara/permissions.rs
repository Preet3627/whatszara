use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

impl RiskLevel {
    pub fn requires_captcha(&self) -> bool {
        matches!(self, RiskLevel::Medium | RiskLevel::High)
    }

    pub fn requires_recaptcha(&self) -> bool {
        matches!(self, RiskLevel::High)
    }

    pub fn requires_confirmation(&self) -> bool {
        matches!(self, RiskLevel::High)
    }
}

fn action_risk_map() -> HashMap<&'static str, RiskLevel> {
    let mut m = HashMap::new();
    m.insert("get_volume", RiskLevel::Low);
    m.insert("list_images", RiskLevel::Low);
    m.insert("get_desktop_paths", RiskLevel::Low);
    m.insert("get_system_info", RiskLevel::Low);
    m.insert("list_messages", RiskLevel::Low);
    m.insert("search_contacts", RiskLevel::Low);
    m.insert("open_app", RiskLevel::Medium);
    m.insert("play_music", RiskLevel::Medium);
    m.insert("pause_music", RiskLevel::Medium);
    m.insert("next_track", RiskLevel::Medium);
    m.insert("prev_track", RiskLevel::Medium);
    m.insert("set_volume", RiskLevel::Medium);
    m.insert("send_message", RiskLevel::Medium);
    m.insert("send_images", RiskLevel::Medium);
    m.insert("execute_shell", RiskLevel::High);
    m.insert("delete_file", RiskLevel::High);
    m.insert("install_software", RiskLevel::High);
    m
}

pub struct PermissionRequest {
    pub action: String,
    pub params: HashMap<String, String>,
    pub contact_jid: String,
    pub risk_level: RiskLevel,
    pub captcha_passed: Option<bool>,
    pub recaptcha_score: Option<f64>,
    pub confirmed: Option<bool>,
}

impl PermissionRequest {
    pub fn is_approved(&self) -> bool {
        match self.risk_level {
            RiskLevel::Low => true,
            RiskLevel::Medium => self.captcha_passed.unwrap_or(false),
            RiskLevel::High => {
                self.captcha_passed.unwrap_or(false)
                    && self.recaptcha_score.unwrap_or(0.0) >= 0.5
                    && self.confirmed.unwrap_or(false)
            }
        }
    }

    pub fn requires_action(&self) -> Vec<&str> {
        let mut r = vec![];
        if self.risk_level.requires_captcha() { r.push("image_captcha"); }
        if self.risk_level.requires_recaptcha() { r.push("recaptcha"); }
        if self.risk_level.requires_confirmation() { r.push("confirmation"); }
        r
    }
}

pub struct PermissionEngine {
    pub overrides: HashMap<String, HashMap<String, RiskLevel>>,
}

impl PermissionEngine {
    pub fn new() -> Self {
        Self { overrides: HashMap::new() }
    }

    pub fn evaluate(&self, action: &str, params: HashMap<String, String>, contact_jid: &str) -> PermissionRequest {
        let risk = self.overrides.get(contact_jid)
            .and_then(|m| m.get(action))
            .copied()
            .or_else(|| action_risk_map().get(action).copied())
            .unwrap_or(RiskLevel::High);

        PermissionRequest {
            action: action.to_string(),
            params,
            contact_jid: contact_jid.to_string(),
            risk_level: risk,
            captcha_passed: None,
            recaptcha_score: None,
            confirmed: None,
        }
    }

    pub fn approve(&mut self, req: &mut PermissionRequest, captcha: bool, recaptcha: Option<f64>, confirmed: bool) -> bool {
        req.captcha_passed = Some(captcha);
        req.recaptcha_score = recaptcha;
        req.confirmed = Some(confirmed);
        req.is_approved()
    }

    pub fn set_override(&mut self, contact: &str, action: &str, level: RiskLevel) {
        self.overrides.entry(contact.to_string()).or_default().insert(action.to_string(), level);
    }
}
