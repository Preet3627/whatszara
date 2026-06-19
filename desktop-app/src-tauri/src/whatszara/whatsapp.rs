use serde::{Deserialize, Serialize};
use rusqlite::Connection;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct Contact {
    pub jid: String,
    pub name: String,
    pub phone: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Chat {
    pub jid: String,
    pub name: String,
    pub last_message: Option<String>,
    pub last_active: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub chat_jid: String,
    pub sender: String,
    pub content: String,
    pub timestamp: String,
    pub media_type: Option<String>,
    pub is_from_me: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ContactInfo {
    pub jid: String,
    pub name: String,
    pub last_active: Option<String>,
}

fn db_path() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.pop();
    p.push("whatsapp-bridge");
    p.push("store");
    p.push("messages.db");
    p
}

fn connect() -> Result<Connection, String> {
    let path = db_path();
    Connection::open(&path).map_err(|e| format!("Failed to open DB at {:?}: {}", path, e))
}

pub fn search_contacts(query: &str) -> Result<Vec<Contact>, String> {
    let conn = connect()?;
    let pattern = format!("%{}%", query);
    let mut stmt = conn
        .prepare("SELECT jid, name, phone FROM contacts WHERE name LIKE ?1 OR phone LIKE ?1")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([&pattern], |row| {
            Ok(Contact {
                jid: row.get(0)?,
                name: row.get(1)?,
                phone: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut contacts = Vec::new();
    for row in rows {
        contacts.push(row.map_err(|e| e.to_string())?);
    }
    Ok(contacts)
}

pub fn list_chats(limit: usize) -> Result<Vec<Chat>, String> {
    let conn = connect()?;
    let mut stmt = conn
        .prepare("SELECT jid, name, NULL, last_message_time FROM chats ORDER BY last_message_time DESC LIMIT ?1")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([&(limit as i64)], |row| {
            Ok(Chat {
                jid: row.get(0)?,
                name: row.get(1)?,
                last_message: row.get(2)?,
                last_active: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut chats = Vec::new();
    for row in rows {
        chats.push(row.map_err(|e| e.to_string())?);
    }
    Ok(chats)
}

pub fn list_messages(chat_jid: &str, limit: usize) -> Result<Vec<Message>, String> {
    let conn = connect()?;
    let mut stmt = conn
        .prepare("SELECT id, chat_jid, sender, content, timestamp, media_type, is_from_me FROM messages WHERE chat_jid = ?1 ORDER BY timestamp ASC LIMIT ?2")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(rusqlite::params![chat_jid, limit as i64], |row| {
            Ok(Message {
                id: row.get(0)?,
                chat_jid: row.get(1)?,
                sender: row.get(2)?,
                content: row.get(3)?,
                timestamp: row.get(4)?,
                media_type: row.get(5)?,
                is_from_me: row.get::<_, bool>(6)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut msgs = Vec::new();
    for row in rows {
        msgs.push(row.map_err(|e| e.to_string())?);
    }
    Ok(msgs)
}

pub fn get_new_incoming_messages_since(rowid: i64, limit: usize) -> Result<Vec<Message>, String> {
    let conn = connect()?;
    let mut stmt = conn
        .prepare("SELECT id, chat_jid, sender, content, timestamp, media_type, is_from_me FROM messages WHERE rowid > ?1 AND is_from_me = 0 ORDER BY rowid ASC LIMIT ?2")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(rusqlite::params![rowid, limit as i64], |row| {
            Ok(Message {
                id: row.get(0)?,
                chat_jid: row.get(1)?,
                sender: row.get(2)?,
                content: row.get(3)?,
                timestamp: row.get(4)?,
                media_type: row.get(5)?,
                is_from_me: row.get::<_, bool>(6)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut msgs = Vec::new();
    for row in rows {
        msgs.push(row.map_err(|e| e.to_string())?);
    }
    Ok(msgs)
}

pub fn get_max_incoming_rowid() -> Result<i64, String> {
    let conn = connect()?;
    conn.query_row(
        "SELECT COALESCE(MAX(rowid), 0) FROM messages WHERE is_from_me = 0",
        [],
        |row| row.get(0),
    )
    .map_err(|e| e.to_string())
}

pub fn list_all_contacts() -> Result<Vec<ContactInfo>, String> {
    let conn = connect()?;
    let mut seen = HashMap::new();
    if let Ok(mut stmt) = conn.prepare("SELECT jid, name FROM contacts") {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        }) {
            for row in rows.flatten() {
                seen.insert(row.0, row.1);
            }
        }
    }
    if let Ok(mut stmt) = conn.prepare("SELECT jid, COALESCE(name, ''), last_message_time FROM chats ORDER BY last_message_time DESC") {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok(ContactInfo {
                jid: row.get(0)?,
                name: {
                    let n: String = row.get(1)?;
                    if n.is_empty() { seen.get(&row.get::<_, String>(0)?).cloned().unwrap_or_default() } else { n }
                },
                last_active: row.get(2)?,
            })
        }) {
            let mut contacts: Vec<ContactInfo> = rows.filter_map(|r| r.ok()).collect();
            for (jid, name) in seen {
                if !contacts.iter().any(|c| c.jid == jid) {
                    contacts.push(ContactInfo { jid, name, last_active: None });
                }
            }
            return Ok(contacts);
        }
    }
    Ok(seen.into_iter().map(|(jid, name)| ContactInfo { jid, name, last_active: None }).collect())
}


