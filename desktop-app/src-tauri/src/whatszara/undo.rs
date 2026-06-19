use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReverseAction {
    pub action: String,
    pub params: std::collections::HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ActionEntry {
    pub action_id: String,
    pub action_type: String,
    pub parameters: std::collections::HashMap<String, String>,
    pub result: serde_json::Value,
    pub reverse_action: Option<ReverseAction>,
    pub risk_level: String,
    pub contact_jid: String,
    pub timestamp: String,
    pub reversed: bool,
    pub reversed_at: Option<String>,
}

pub struct ActionJournal {
    entries: VecDeque<ActionEntry>,
    max_entries: usize,
    counter: u64,
}

impl ActionJournal {
    pub fn new(max_entries: usize) -> Self {
        Self { entries: VecDeque::new(), max_entries, counter: 0 }
    }

    pub fn record(
        &mut self,
        action_type: &str,
        parameters: std::collections::HashMap<String, String>,
        result: serde_json::Value,
        reverse_action: Option<ReverseAction>,
        risk_level: &str,
        contact_jid: &str,
    ) -> String {
        self.counter += 1;
        let action_id = format!("act_{}_{}", Utc::now().timestamp(), self.counter);
        let entry = ActionEntry {
            action_id: action_id.clone(),
            action_type: action_type.to_string(),
            parameters,
            result,
            reverse_action,
            risk_level: risk_level.to_string(),
            contact_jid: contact_jid.to_string(),
            timestamp: Utc::now().to_rfc3339(),
            reversed: false,
            reversed_at: None,
        };
        self.entries.push_back(entry);
        while self.entries.len() > self.max_entries {
            self.entries.pop_front();
        }
        action_id
    }

    pub fn mark_reversed(&mut self, action_id: &str) -> Option<&ActionEntry> {
        for entry in &mut self.entries {
            if entry.action_id == action_id {
                entry.reversed = true;
                entry.reversed_at = Some(Utc::now().to_rfc3339());
                return Some(&*entry);
            }
        }
        None
    }

    pub fn get(&self, action_id: &str) -> Option<&ActionEntry> {
        self.entries.iter().rev().find(|e| e.action_id == action_id)
    }

    pub fn recent(&self, limit: usize) -> Vec<&ActionEntry> {
        self.entries.iter().rev().take(limit).collect()
    }

    pub fn reversible(&self) -> Vec<&ActionEntry> {
        self.entries.iter().rev().filter(|e| !e.reversed && e.reverse_action.is_some()).collect()
    }

    pub fn len(&self) -> usize { self.entries.len() }
}
