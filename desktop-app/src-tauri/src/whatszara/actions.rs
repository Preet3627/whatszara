use serde::{Deserialize, Serialize};
use tokio::process::Command;

#[derive(Debug, Serialize, Deserialize)]
pub struct ActionResult {
    pub success: bool,
    pub output: String,
    pub error: String,
}

// ── Shell ──────────────────────────────────────
pub struct ShellExecutor {
    pub timeout_secs: u64,
    pub blocked: Vec<String>,
}

impl Default for ShellExecutor {
    fn default() -> Self {
        Self {
            timeout_secs: 30,
            blocked: vec!["sudo".into(), "rm -rf /".into(), "dd".into(), "mkfs".into()],
        }
    }
}

impl ShellExecutor {
    pub fn is_blocked(&self, cmd: &str) -> bool {
        let lower = cmd.to_lowercase();
        self.blocked.iter().any(|b| lower.contains(&b.to_lowercase()))
    }

    pub async fn execute(&self, command: &str) -> ActionResult {
        if self.is_blocked(command) {
            return ActionResult { success: false, output: String::new(), error: "Command blocked by security policy".into() };
        }
        let child = Command::new("sh").arg("-c").arg(command)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn();
        let mut child = match child {
            Ok(c) => c,
            Err(e) => return ActionResult { success: false, output: String::new(), error: e.to_string() },
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
            },
            Ok(Err(e)) => ActionResult { success: false, output: String::new(), error: e.to_string() },
            Err(_) => ActionResult { success: false, output: String::new(), error: format!("Timed out after {}s", self.timeout_secs) },
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
                output: String::from_utf8_lossy(&out.stdout).to_string(),
                error: String::from_utf8_lossy(&out.stderr).to_string(),
            },
            Err(e) => ActionResult { success: false, output: String::new(), error: e.to_string() },
        }
    }
}

// ── Media ──────────────────────────────────────
pub struct MediaController;

impl MediaController {
    pub async fn get_volume() -> ActionResult {
        if cfg!(target_os = "macos") {
            let out = Command::new("osascript").arg("-e").arg("output volume of (get volume settings)").output().await;
            match out {
                Ok(o) => ActionResult {
                    success: o.status.success(),
                    output: String::from_utf8_lossy(&o.stdout).trim().to_string(),
                    error: String::from_utf8_lossy(&o.stderr).to_string(),
                },
                Err(e) => ActionResult { success: false, output: String::new(), error: e.to_string() },
            }
        } else {
            ActionResult { success: false, output: String::new(), error: "Unsupported platform".into() }
        }
    }

    pub async fn set_volume(level: u8) -> ActionResult {
        let level = level.min(100);
        if cfg!(target_os = "macos") {
            let out = Command::new("osascript").arg("-e").arg(format!("set volume output volume {}", level)).output().await;
            match out {
                Ok(o) => ActionResult {
                    success: o.status.success(),
                    output: level.to_string(),
                    error: String::from_utf8_lossy(&o.stderr).to_string(),
                },
                Err(e) => ActionResult { success: false, output: String::new(), error: e.to_string() },
            }
        } else {
            ActionResult { success: false, output: String::new(), error: "Unsupported platform".into() }
        }
    }

    pub async fn play_music(query: Option<&str>) -> ActionResult {
        if cfg!(target_os = "macos") {
            let script = if let Some(q) = query {
                format!(r#"tell application "Music" to play track "{}""#, q)
            } else {
                r#"tell application "Music" to play"#.into()
            };
            let out = Command::new("osascript").arg("-e").arg(&script).output().await;
            match out {
                Ok(o) => ActionResult {
                    success: o.status.success(),
                    output: String::from_utf8_lossy(&o.stdout).to_string(),
                    error: String::from_utf8_lossy(&o.stderr).to_string(),
                },
                Err(e) => ActionResult { success: false, output: String::new(), error: e.to_string() },
            }
        } else {
            ActionResult { success: false, output: String::new(), error: "Unsupported platform".into() }
        }
    }

    pub async fn pause() -> ActionResult {
        Self::osascript(r#"tell application "Music" to pause"#).await
    }

    pub async fn next_track() -> ActionResult {
        Self::osascript(r#"tell application "Music" to next track"#).await
    }

    pub async fn prev_track() -> ActionResult {
        Self::osascript(r#"tell application "Music" to previous track"#).await
    }

    async fn osascript(script: &str) -> ActionResult {
        if !cfg!(target_os = "macos") {
            return ActionResult { success: false, output: String::new(), error: "Unsupported platform".into() };
        }
        match Command::new("osascript").arg("-e").arg(script).output().await {
            Ok(o) => ActionResult {
                success: o.status.success(),
                output: String::from_utf8_lossy(&o.stdout).to_string(),
                error: String::from_utf8_lossy(&o.stderr).to_string(),
            },
            Err(e) => ActionResult { success: false, output: String::new(), error: e.to_string() },
        }
    }
}

// ── Desktop ────────────────────────────────────
pub struct DesktopScanner;

impl DesktopScanner {
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
                }
            }
            Err(e) => ActionResult { success: false, output: String::new(), error: e.to_string() },
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
        }
    }
}
