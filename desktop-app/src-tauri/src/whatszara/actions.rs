use serde::{Deserialize, Serialize};
use tokio::process::Command;

#[derive(Debug, Serialize, Deserialize)]
pub struct ActionResult {
    pub success: bool,
    pub output: String,
    pub error: String,
    pub action: String,
    pub params: std::collections::HashMap<String, String>,
}

impl ActionResult {
    #[allow(dead_code)]
    pub fn ok(action: &str, output: &str) -> Self {
        Self { success: true, output: output.to_string(), error: String::new(), action: action.to_string(), params: std::collections::HashMap::new() }
    }
    pub fn fail(action: &str, error: &str) -> Self {
        Self { success: false, output: String::new(), error: error.to_string(), action: action.to_string(), params: std::collections::HashMap::new() }
    }
}

// ── Shell (DISABLED by default, must be explicitly enabled) ──
pub struct ShellExecutor {
    pub timeout_secs: u64,
    pub blocked: Vec<String>,
}

impl Default for ShellExecutor {
    fn default() -> Self {
        Self { timeout_secs: 30, blocked: vec!["sudo".into(), "rm -rf /".into(), "dd".into(), "mkfs".into(), ":(){".into()] }
    }
}

impl ShellExecutor {
    pub fn is_blocked(&self, cmd: &str) -> bool {
        let lower = cmd.to_lowercase();
        self.blocked.iter().any(|b| lower.contains(&b.to_lowercase()))
    }
    pub async fn execute(&self, command: &str) -> ActionResult {
        if self.is_blocked(command) {
            return ActionResult::fail("execute_shell", "Command blocked by security policy");
        }
        let child = match Command::new("sh").arg("-c").arg(command)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => return ActionResult::fail("execute_shell", &e.to_string()),
        };
        let output = tokio::time::timeout(
            std::time::Duration::from_secs(self.timeout_secs),
            child.wait_with_output(),
        ).await;
        match output {
            Ok(Ok(out)) => ActionResult {
                success: out.status.success(),
                output: String::from_utf8_lossy(&out.stdout).to_string(),
                error: String::from_utf8_lossy(&out.stderr).to_string(),
                action: "execute_shell".into(),
                params: std::collections::HashMap::new(),
            },
            Ok(Err(e)) => ActionResult::fail("execute_shell", &e.to_string()),
            Err(_) => ActionResult::fail("execute_shell", &format!("Timed out after {}s", self.timeout_secs)),
        }
    }
}

// ── Apps ───────────────────────────────────────
pub struct AppLauncher;

impl AppLauncher {
    pub async fn open(name: &str) -> ActionResult {
        let cmd = if cfg!(target_os = "macos") {
            Command::new("open").arg("-a").arg(name).output().await
        } else if cfg!(target_os = "linux") {
            Command::new(name).output().await
        } else {
            Command::new("cmd").args(["/c", "start", name]).output().await
        };
        match cmd {
            Ok(out) => ActionResult {
                success: out.status.success(),
                output: if out.status.success() { format!("Opened {}", name) } else { String::new() },
                error: String::from_utf8_lossy(&out.stderr).to_string(),
                action: "open_app".into(),
                params: [("name".into(), name.to_string())].into(),
            },
            Err(e) => ActionResult::fail("open_app", &e.to_string()),
        }
    }
}

// ── Media ──────────────────────────────────────
pub struct MediaController;

impl MediaController {
    pub async fn get_volume() -> ActionResult {
        if !cfg!(target_os = "macos") {
            return ActionResult::fail("get_volume", "Unsupported platform");
        }
        let out = Command::new("osascript").arg("-e").arg("output volume of (get volume settings)").output().await;
        match out {
            Ok(o) => ActionResult {
                success: o.status.success(),
                output: String::from_utf8_lossy(&o.stdout).trim().to_string(),
                error: String::from_utf8_lossy(&o.stderr).to_string(),
                action: "get_volume".into(),
                params: std::collections::HashMap::new(),
            },
            Err(e) => ActionResult::fail("get_volume", &e.to_string()),
        }
    }

    pub async fn set_volume(level: u8) -> ActionResult {
        let level = level.min(100);
        if !cfg!(target_os = "macos") {
            return ActionResult::fail("set_volume", "Unsupported platform");
        }
        let out = Command::new("osascript").arg("-e").arg(format!("set volume output volume {}", level)).output().await;
        match out {
            Ok(o) => ActionResult {
                success: o.status.success(),
                output: format!("Volume set to {}", level),
                error: String::from_utf8_lossy(&o.stderr).to_string(),
                action: "set_volume".into(),
                params: [("level".into(), level.to_string())].into(),
            },
            Err(e) => ActionResult::fail("set_volume", &e.to_string()),
        }
    }

    pub async fn play(query: Option<&str>) -> ActionResult {
        Self::osascript_action("play", &query.map(|q| format!(r#"play track "{}""#, q)).unwrap_or("play".into())).await
    }

    pub async fn pause() -> ActionResult {
        Self::osascript_action("pause_media", r#"pause"#.into()).await
    }

    pub async fn next_track() -> ActionResult {
        Self::osascript_action("next_track", r#"next track"#.into()).await
    }

    pub async fn prev_track() -> ActionResult {
        Self::osascript_action("prev_track", r#"previous track"#.into()).await
    }

    async fn osascript_action(action: &str, script: &str) -> ActionResult {
        if !cfg!(target_os = "macos") {
            return ActionResult::fail(action, "Unsupported platform");
        }
        let out = Command::new("osascript").arg("-e").arg(format!(r#"tell application "Music" to {}"#, script)).output().await;
        match out {
            Ok(o) => ActionResult {
                success: o.status.success(),
                output: String::from_utf8_lossy(&o.stdout).to_string(),
                error: String::from_utf8_lossy(&o.stderr).to_string(),
                action: action.to_string(),
                params: std::collections::HashMap::new(),
            },
            Err(e) => ActionResult::fail(action, &e.to_string()),
        }
    }
}

// ── Desktop File Scanner ──────────────────────
pub struct DesktopScanner;

impl DesktopScanner {
    #[allow(dead_code)]
    pub async fn list_files(path: Option<&str>) -> ActionResult {
        let search_path = path.unwrap_or("~/Desktop");
        let expanded = shellexpand::tilde(search_path).to_string();
        let output = Command::new("sh")
            .arg("-c")
            .arg(format!(r#"ls -la "{}" 2>/dev/null | head -50"#, expanded))
            .output().await;
        match output {
            Ok(out) => ActionResult {
                success: true,
                output: String::from_utf8_lossy(&out.stdout).to_string(),
                error: String::from_utf8_lossy(&out.stderr).to_string(),
                action: "list_files".into(),
                params: [("path".into(), search_path.to_string())].into(),
            },
            Err(e) => ActionResult::fail("list_files", &e.to_string()),
        }
    }

    pub async fn list_images(path: Option<&str>) -> ActionResult {
        let search_path = path.unwrap_or("~/Desktop");
        let expanded = shellexpand::tilde(search_path).to_string();
        let output = Command::new("sh")
            .arg("-c")
            .arg(format!(r#"find "{}" -type f \( -iname "*.jpg" -o -iname "*.jpeg" -o -iname "*.png" -o -iname "*.gif" -o -iname "*.webp" \) 2>/dev/null | head -100"#, expanded))
            .output().await;
        match output {
            Ok(out) => {
                let files: Vec<&str> = std::str::from_utf8(&out.stdout).unwrap_or("").lines().collect();
                ActionResult {
                    success: true,
                    output: serde_json::json!({ "count": files.len(), "files": files }).to_string(),
                    error: String::new(),
                    action: "list_images".into(),
                    params: [("path".into(), search_path.to_string())].into(),
                }
            }
            Err(e) => ActionResult::fail("list_images", &e.to_string()),
        }
    }

    pub async fn get_desktop_paths() -> ActionResult {
        let paths = vec!["~/Desktop", "~/Pictures", "~/Downloads"];
        let result: Vec<_> = paths.iter().map(|p| {
            let expanded = shellexpand::tilde(p).to_string();
            (p.to_string(), std::path::Path::new(&expanded).exists())
        }).collect();
        ActionResult {
            success: true,
            output: serde_json::json!(result).to_string(),
            error: String::new(),
            action: "get_desktop_paths".into(),
            params: std::collections::HashMap::new(),
        }
    }
}
