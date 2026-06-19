use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LLMMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LLMResponse {
    pub content: String,
    pub model: String,
    pub provider: String,
}

#[async_trait::async_trait]
pub trait LLMProvider: Send + Sync {
    fn name(&self) -> &str;
    async fn chat(&self, messages: &[LLMMessage], system_prompt: Option<&str>) -> Result<LLMResponse, String>;
    async fn list_models(&self) -> Result<Vec<String>, String>;
    fn default_model(&self) -> &str;
    fn set_model(&mut self, model: &str);
    fn current_model(&self) -> &str;
    fn set_endpoint(&mut self, _endpoint: &str) {}
    fn set_api_key(&mut self, _key: &str) {}
}

// ── Ollama ─────────────────────────────────────
pub struct OllamaProvider {
    pub endpoint: String,
    pub model: String,
}

#[async_trait::async_trait]
impl LLMProvider for OllamaProvider {
    fn name(&self) -> &str { "ollama" }
    fn current_model(&self) -> &str { &self.model }
    fn set_model(&mut self, model: &str) { self.model = model.to_string(); }
    fn set_endpoint(&mut self, endpoint: &str) { self.endpoint = endpoint.to_string(); }

    async fn chat(&self, messages: &[LLMMessage], system_prompt: Option<&str>) -> Result<LLMResponse, String> {
        let client = reqwest::Client::new();
        let mut payload_messages: Vec<serde_json::Value> = messages.iter().map(|m| {
            serde_json::json!({"role": m.role, "content": m.content})
        }).collect();
        if let Some(sp) = system_prompt {
            payload_messages.insert(0, serde_json::json!({"role": "system", "content": sp}));
        }
        let payload = serde_json::json!({
            "model": self.model,
            "messages": payload_messages,
            "stream": false,
        });
        let resp = client.post(format!("{}/api/chat", self.endpoint))
            .json(&payload)
            .timeout(std::time::Duration::from_secs(120))
            .send()
            .await
            .map_err(|e| format!("Ollama request failed: {}", e))?;
        let data: serde_json::Value = resp.json().await.map_err(|e| format!("Ollama parse failed: {}", e))?;
        let content = data["message"]["content"].as_str().unwrap_or("").to_string();
        Ok(LLMResponse { content, model: self.model.clone(), provider: "ollama".into() })
    }

    async fn list_models(&self) -> Result<Vec<String>, String> {
        let client = reqwest::Client::new();
        let resp = client.get(format!("{}/api/tags", self.endpoint))
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map_err(|_| "Ollama not available".to_string())?;
        let data: serde_json::Value = resp.json().await.map_err(|_| "Bad response".to_string())?;
        let models = data["models"].as_array().unwrap_or(&vec![]).iter()
            .filter_map(|m| m["name"].as_str().map(|s| s.to_string()))
            .collect();
        Ok(models)
    }

    fn default_model(&self) -> &str {
        if self.model.is_empty() { "llama3.2" } else { &self.model }
    }
}

// ── Claude ─────────────────────────────────────
pub struct ClaudeProvider {
    pub api_key: String,
    pub model: String,
}

#[async_trait::async_trait]
impl LLMProvider for ClaudeProvider {
    fn name(&self) -> &str { "claude" }
    fn current_model(&self) -> &str { &self.model }
    fn set_model(&mut self, model: &str) { self.model = model.to_string(); }

    async fn chat(&self, messages: &[LLMMessage], system_prompt: Option<&str>) -> Result<LLMResponse, String> {
        let client = reqwest::Client::new();
        let mut payload = serde_json::json!({
            "model": self.model,
            "max_tokens": 4096,
            "messages": messages.iter().map(|m| serde_json::json!({"role": m.role, "content": m.content})).collect::<Vec<_>>(),
        });
        if let Some(sp) = system_prompt {
            payload["system"] = serde_json::json!(sp);
        }
        let resp = client.post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&payload)
            .timeout(std::time::Duration::from_secs(120))
            .send()
            .await
            .map_err(|e| format!("Claude request failed: {}", e))?;
        let data: serde_json::Value = resp.json().await.map_err(|e| format!("Claude parse failed: {}", e))?;
        let content = data["content"].as_array().unwrap_or(&vec![]).iter()
            .filter_map(|b| b["text"].as_str())
            .collect::<Vec<_>>()
            .join("");
        Ok(LLMResponse { content, model: self.model.clone(), provider: "claude".into() })
    }

    async fn list_models(&self) -> Result<Vec<String>, String> {
        let client = reqwest::Client::new();
        let resp = client.get("https://api.anthropic.com/v1/models")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map_err(|_| "Claude API not available".to_string())?;
        let data: serde_json::Value = resp.json().await.map_err(|_| "Bad response".to_string())?;
        let models = data["data"].as_array().unwrap_or(&vec![]).iter()
            .filter_map(|m| m["id"].as_str().map(|s| s.to_string()))
            .collect();
        Ok(models)
    }

    fn default_model(&self) -> &str { &self.model }
}

// ── Groq ───────────────────────────────────────
pub struct GroqProvider {
    pub api_key: String,
    pub model: String,
}

#[async_trait::async_trait]
impl LLMProvider for GroqProvider {
    fn name(&self) -> &str { "groq" }
    fn current_model(&self) -> &str { &self.model }
    fn set_model(&mut self, model: &str) { self.model = model.to_string(); }

    async fn chat(&self, messages: &[LLMMessage], system_prompt: Option<&str>) -> Result<LLMResponse, String> {
        let client = reqwest::Client::new();
        let mut payload = serde_json::json!({
            "model": self.model,
            "messages": messages.iter().map(|m| serde_json::json!({"role": m.role, "content": m.content})).collect::<Vec<_>>(),
        });
        if let Some(sp) = system_prompt {
            payload["messages"].as_array_mut().unwrap().insert(0, serde_json::json!({"role": "system", "content": sp}));
        }
        let resp = client.post("https://api.groq.com/openai/v1/chat/completions")
            .bearer_auth(&self.api_key)
            .json(&payload)
            .timeout(std::time::Duration::from_secs(120))
            .send()
            .await
            .map_err(|e| format!("Groq request failed: {}", e))?;
        let data: serde_json::Value = resp.json().await.map_err(|e| format!("Groq parse failed: {}", e))?;
        let content = data["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string();
        Ok(LLMResponse { content, model: self.model.clone(), provider: "groq".into() })
    }

    async fn list_models(&self) -> Result<Vec<String>, String> {
        let client = reqwest::Client::new();
        let resp = client.get("https://api.groq.com/openai/v1/models")
            .bearer_auth(&self.api_key)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map_err(|_| "Groq API not available".to_string())?;
        let data: serde_json::Value = resp.json().await.map_err(|_| "Bad response".to_string())?;
        let models = data["data"].as_array().unwrap_or(&vec![]).iter()
            .filter_map(|m| m["id"].as_str().map(|s| s.to_string()))
            .collect();
        Ok(models)
    }

    fn default_model(&self) -> &str { &self.model }
}

// ── Grok ───────────────────────────────────────
pub struct GrokProvider {
    pub api_key: String,
    pub model: String,
}

#[async_trait::async_trait]
impl LLMProvider for GrokProvider {
    fn name(&self) -> &str { "grok" }
    fn current_model(&self) -> &str { &self.model }
    fn set_model(&mut self, model: &str) { self.model = model.to_string(); }

    async fn chat(&self, messages: &[LLMMessage], system_prompt: Option<&str>) -> Result<LLMResponse, String> {
        let client = reqwest::Client::new();
        let mut payload = serde_json::json!({
            "model": self.model,
            "messages": messages.iter().map(|m| serde_json::json!({"role": m.role, "content": m.content})).collect::<Vec<_>>(),
        });
        if let Some(sp) = system_prompt {
            payload["messages"].as_array_mut().unwrap().insert(0, serde_json::json!({"role": "system", "content": sp}));
        }
        let resp = client.post("https://api.x.ai/v1/chat/completions")
            .bearer_auth(&self.api_key)
            .json(&payload)
            .timeout(std::time::Duration::from_secs(120))
            .send()
            .await
            .map_err(|e| format!("Grok request failed: {}", e))?;
        let data: serde_json::Value = resp.json().await.map_err(|e| format!("Grok parse failed: {}", e))?;
        let content = data["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string();
        Ok(LLMResponse { content, model: self.model.clone(), provider: "grok".into() })
    }

    async fn list_models(&self) -> Result<Vec<String>, String> {
        let client = reqwest::Client::new();
        let resp = client.get("https://api.x.ai/v1/models")
            .bearer_auth(&self.api_key)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map_err(|_| "Grok API not available".to_string())?;
        let data: serde_json::Value = resp.json().await.map_err(|_| "Bad response".to_string())?;
        let models = data["data"].as_array().unwrap_or(&vec![]).iter()
            .filter_map(|m| m["id"].as_str().map(|s| s.to_string()))
            .collect();
        Ok(models)
    }

    fn default_model(&self) -> &str { &self.model }
}

// ── Gemini ─────────────────────────────────────
pub struct GeminiProvider {
    pub api_key: String,
    pub model: String,
}

#[async_trait::async_trait]
impl LLMProvider for GeminiProvider {
    fn name(&self) -> &str { "gemini" }
    fn current_model(&self) -> &str { &self.model }
    fn set_model(&mut self, model: &str) { self.model = model.to_string(); }

    async fn chat(&self, messages: &[LLMMessage], system_prompt: Option<&str>) -> Result<LLMResponse, String> {
        let client = reqwest::Client::new();
        let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}", self.model, self.api_key);
        let mut payload = serde_json::json!({
            "contents": messages.iter().map(|m| serde_json::json!({"role": m.role, "parts": [{"text": m.content}]})).collect::<Vec<_>>(),
        });
        if let Some(sp) = system_prompt {
            payload["systemInstruction"] = serde_json::json!({"parts": [{"text": sp}]});
        }
        let resp = client.post(&url)
            .json(&payload)
            .timeout(std::time::Duration::from_secs(120))
            .send()
            .await
            .map_err(|e| format!("Gemini request failed: {}", e))?;
        let data: serde_json::Value = resp.json().await.map_err(|e| format!("Gemini parse failed: {}", e))?;
        let content = data["candidates"].as_array().unwrap_or(&vec![]).iter()
            .filter_map(|c| c["content"]["parts"].as_array())
            .flatten()
            .filter_map(|p| p["text"].as_str())
            .collect::<Vec<_>>()
            .join("");
        Ok(LLMResponse { content, model: self.model.clone(), provider: "gemini".into() })
    }

    async fn list_models(&self) -> Result<Vec<String>, String> {
        let client = reqwest::Client::new();
        let url = format!("https://generativelanguage.googleapis.com/v1beta/models?key={}", self.api_key);
        let resp = client.get(&url)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map_err(|_| "Gemini API not available".to_string())?;
        let data: serde_json::Value = resp.json().await.map_err(|_| "Bad response".to_string())?;
        let models = data["models"].as_array().unwrap_or(&vec![]).iter()
            .filter_map(|m| m["name"].as_str().map(|s| {
                s.trim_start_matches("models/").to_string()
            }))
            .filter(|s| !s.contains("generateContent") && !s.contains("embedding"))
            .collect();
        Ok(models)
    }

    fn default_model(&self) -> &str { &self.model }
}

// ── Provider Registry ─────────────────────────
pub struct ProviderRegistry {
    pub providers: Vec<Box<dyn LLMProvider>>,
    pub active: usize,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self { providers: vec![], active: 0 }
    }

    pub fn register(&mut self, provider: Box<dyn LLMProvider>) {
        self.providers.push(provider);
    }

    pub fn active_provider(&self) -> &dyn LLMProvider {
        self.providers[self.active].as_ref()
    }

    pub fn set_active(&mut self, name: &str) -> Result<(), String> {
        for (i, p) in self.providers.iter().enumerate() {
            if p.name() == name {
                self.active = i;
                return Ok(());
            }
        }
        Err(format!("Provider '{}' not found", name))
    }

    pub fn list_names(&self) -> Vec<String> {
        self.providers.iter().map(|p| p.name().to_string()).collect()
    }

    pub fn set_model(&mut self, provider: &str, model: &str) -> Result<(), String> {
        for p in &mut self.providers {
            if p.name() == provider {
                p.set_model(model);
                return Ok(());
            }
        }
        Err(format!("Provider '{}' not found", provider))
    }

    #[allow(dead_code)]
    pub fn current_model_of(&self, provider: &str) -> Option<String> {
        self.providers.iter().find(|p| p.name() == provider).map(|p| p.current_model().to_string())
    }

    pub async fn list_all_models(&self) -> Vec<(String, Vec<String>, String)> {
        let mut results = vec![];
        for p in &self.providers {
            let models = p.list_models().await.unwrap_or_else(|_| vec![p.default_model().to_string()]);
            let current = p.current_model().to_string();
            let effective = if current.is_empty() { models.first().cloned().unwrap_or_else(|| current) } else { current };
            results.push((p.name().to_string(), models, effective));
        }
        results
    }

    pub async fn chat(&self, messages: &[LLMMessage], system_prompt: Option<&str>) -> Result<LLMResponse, String> {
        if self.providers.is_empty() {
            return Err("No LLM providers configured".into());
        }
        self.active_provider().chat(messages, system_prompt).await
    }
}
