//! §4c — **backend breadth**: local inference backends.
//!
//! The item's whole point is that [`crate::InferenceBackend`] already isolates the
//! provider, so adding local models must not touch [`crate::run_task`]. It does not:
//! both backends here are ordinary trait impls.
//!
//! * [`OpenAiCompatBackend`] — any server exposing OpenAI's `/v1/chat/completions`.
//!   That covers **llama.cpp's `llama-server`**, **vLLM**, **LM Studio**, **Ollama's
//!   OpenAI-compatible endpoint**, and hosted providers. An API key is optional, since
//!   local servers usually have none.
//! * [`OllamaBackend`] — Ollama's **native** `/api/chat`, whose response shape differs
//!   (`message.content`, not `choices[0].message.content`) and whose images are a bare
//!   base64 array rather than data URLs.
//!
//! **Platform:** the *trait seam* is `[XP]`; the runtimes themselves are per-OS binaries
//! the user installs. Nothing here bundles a model or an inference runtime.
//!
//! **Documented gaps (not faked):** no streaming (the agent loop consumes a whole reply);
//! no tool-calling API (the agent's protocol is JSON-in-text by design, which is exactly
//! why it works on small local models).

use anyhow::{anyhow, bail, Context, Result};
use base64::Engine as _;
use bytes::Bytes;
use serde_json::{json, Value};

use crate::{Content, InferenceBackend, Message};

fn truncate(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// Any server speaking OpenAI's `/v1/chat/completions`.
pub struct OpenAiCompatBackend {
    endpoint: String,
    model: String,
    /// `None` for a local server with no auth.
    api_key: Option<String>,
    max_tokens: u32,
    label: String,
}

impl OpenAiCompatBackend {
    /// `base` is the server root, e.g. `http://localhost:8080` (llama-server) or
    /// `http://localhost:11434/v1` (Ollama's compat endpoint).
    pub fn new(base: &str, model: impl Into<String>) -> Self {
        let base = base.trim_end_matches('/');
        let endpoint = if base.ends_with("/chat/completions") {
            base.to_string()
        } else if base.ends_with("/v1") {
            format!("{base}/chat/completions")
        } else {
            format!("{base}/v1/chat/completions")
        };
        OpenAiCompatBackend {
            endpoint,
            model: model.into(),
            api_key: None,
            max_tokens: 1024,
            label: "openai-compat".to_string(),
        }
    }

    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    pub fn with_max_tokens(mut self, n: u32) -> Self {
        self.max_tokens = n;
        self
    }

    /// Override the `name()` prefix (e.g. "llama.cpp", "vllm").
    pub fn with_label(mut self, l: impl Into<String>) -> Self {
        self.label = l.into();
        self
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    fn body(&self, messages: &[Message]) -> Value {
        json!({
            "model": self.model,
            "max_tokens": self.max_tokens,
            "messages": messages.iter().map(openai_message).collect::<Vec<_>>(),
        })
    }
}

/// OpenAI content encoding: a plain string when the message is text-only, otherwise a
/// parts array with `image_url` data URLs.
fn openai_message(m: &Message) -> Value {
    if let [Content::Text(t)] = m.content.as_slice() {
        return json!({ "role": m.role.as_str(), "content": t });
    }
    let parts: Vec<Value> = m
        .content
        .iter()
        .map(|c| match c {
            Content::Text(t) => json!({"type": "text", "text": t}),
            Content::ImagePng(png) => {
                let b64 = base64::engine::general_purpose::STANDARD.encode(png);
                json!({"type": "image_url", "image_url": {"url": format!("data:image/png;base64,{b64}")}})
            }
        })
        .collect();
    json!({ "role": m.role.as_str(), "content": parts })
}

#[async_trait::async_trait]
impl InferenceBackend for OpenAiCompatBackend {
    async fn complete(&self, messages: &[Message]) -> Result<String> {
        let body = serde_json::to_vec(&self.body(messages)).context("serializing request")?;
        let auth = self.api_key.as_ref().map(|k| format!("Bearer {k}"));
        let mut headers: Vec<(&str, &str)> = vec![("Content-Type", "application/json")];
        if let Some(a) = &auth {
            headers.push(("Authorization", a.as_str()));
        }

        let resp = manuk_net::request("POST", &self.endpoint, &headers, Bytes::from(body))
            .await
            .with_context(|| format!("POST to {}", self.endpoint))?;

        if resp.status != 200 {
            bail!(
                "{} HTTP {}: {}",
                self.label,
                resp.status,
                truncate(&resp.text(), 400)
            );
        }
        let v: Value = serde_json::from_slice(&resp.body).context("parsing response")?;
        v["choices"][0]["message"]["content"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| {
                anyhow!(
                    "no choices[0].message.content in response: {}",
                    truncate(&resp.text(), 300)
                )
            })
    }

    fn name(&self) -> String {
        format!("{}:{}", self.label, self.model)
    }
}

/// Ollama's **native** chat API (`/api/chat`).
pub struct OllamaBackend {
    endpoint: String,
    model: String,
}

impl OllamaBackend {
    /// `base` defaults to `http://localhost:11434` when empty.
    pub fn new(base: &str, model: impl Into<String>) -> Self {
        let base = if base.trim().is_empty() {
            "http://localhost:11434"
        } else {
            base.trim_end_matches('/')
        };
        OllamaBackend {
            endpoint: format!("{base}/api/chat"),
            model: model.into(),
        }
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    fn body(&self, messages: &[Message]) -> Value {
        json!({
            "model": self.model,
            // The agent loop wants one whole reply, not a token stream.
            "stream": false,
            "messages": messages.iter().map(ollama_message).collect::<Vec<_>>(),
        })
    }
}

/// Ollama takes text in `content` and images as a **bare base64 array**, not data URLs.
fn ollama_message(m: &Message) -> Value {
    let mut text = String::new();
    let mut images: Vec<String> = Vec::new();
    for c in &m.content {
        match c {
            Content::Text(t) => {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(t);
            }
            Content::ImagePng(png) => {
                images.push(base64::engine::general_purpose::STANDARD.encode(png));
            }
        }
    }
    if images.is_empty() {
        json!({ "role": m.role.as_str(), "content": text })
    } else {
        json!({ "role": m.role.as_str(), "content": text, "images": images })
    }
}

#[async_trait::async_trait]
impl InferenceBackend for OllamaBackend {
    async fn complete(&self, messages: &[Message]) -> Result<String> {
        let body = serde_json::to_vec(&self.body(messages)).context("serializing request")?;
        let resp = manuk_net::request(
            "POST",
            &self.endpoint,
            &[("Content-Type", "application/json")],
            Bytes::from(body),
        )
        .await
        .with_context(|| format!("POST to {}", self.endpoint))?;

        if resp.status != 200 {
            bail!("ollama HTTP {}: {}", resp.status, truncate(&resp.text(), 400));
        }
        let v: Value = serde_json::from_slice(&resp.body).context("parsing ollama response")?;
        // Native shape: {"message": {"role": "...", "content": "..."}}
        v["message"]["content"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| {
                anyhow!(
                    "no message.content in ollama response: {}",
                    truncate(&resp.text(), 300)
                )
            })
    }

    fn name(&self) -> String {
        format!("ollama:{}", self.model)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Role;

    #[test]
    fn openai_endpoint_is_derived_from_several_base_shapes() {
        let e = |b: &str| OpenAiCompatBackend::new(b, "m").endpoint().to_string();
        assert_eq!(e("http://localhost:8080"), "http://localhost:8080/v1/chat/completions");
        assert_eq!(e("http://localhost:8080/"), "http://localhost:8080/v1/chat/completions");
        assert_eq!(e("http://localhost:11434/v1"), "http://localhost:11434/v1/chat/completions");
        // An already-complete endpoint is left alone.
        assert_eq!(
            e("http://x/v1/chat/completions"),
            "http://x/v1/chat/completions"
        );
    }

    #[test]
    fn ollama_defaults_to_the_local_daemon() {
        assert_eq!(
            OllamaBackend::new("", "llama3").endpoint(),
            "http://localhost:11434/api/chat"
        );
        assert_eq!(
            OllamaBackend::new("http://box:11434/", "llama3").endpoint(),
            "http://box:11434/api/chat"
        );
    }

    #[test]
    fn openai_text_only_message_is_a_string_but_images_become_a_parts_array() {
        let m = Message::text(Role::User, "hi");
        assert_eq!(openai_message(&m)["content"], "hi");

        let m = Message {
            role: Role::User,
            content: vec![Content::Text("look".into()), Content::ImagePng(vec![1, 2, 3])],
        };
        let v = openai_message(&m);
        assert_eq!(v["content"][0]["type"], "text");
        assert_eq!(v["content"][1]["type"], "image_url");
        assert!(v["content"][1]["image_url"]["url"]
            .as_str()
            .unwrap()
            .starts_with("data:image/png;base64,"));
    }

    /// Ollama's image encoding differs from OpenAI's — a bare base64 array, no data URL.
    #[test]
    fn ollama_images_are_a_bare_base64_array() {
        let m = Message {
            role: Role::User,
            content: vec![Content::Text("look".into()), Content::ImagePng(vec![1, 2, 3])],
        };
        let v = ollama_message(&m);
        assert_eq!(v["content"], "look");
        let img = v["images"][0].as_str().unwrap();
        assert!(!img.starts_with("data:"), "must not be a data URL");
        assert_eq!(
            base64::engine::general_purpose::STANDARD.decode(img).unwrap(),
            vec![1, 2, 3]
        );
    }

    #[test]
    fn ollama_never_streams_because_the_agent_loop_wants_one_reply() {
        let b = OllamaBackend::new("", "m");
        assert_eq!(b.body(&[Message::text(Role::User, "x")])["stream"], false);
    }

    /// A minimal HTTP/1.1 server that answers any POST with `body`, so a backend can be
    /// exercised end-to-end without installing an inference runtime.
    async fn spawn_http(body: &'static str) -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap().to_string();
        tokio::spawn(async move {
            while let Ok((mut s, _)) = l.accept().await {
                tokio::spawn(async move {
                    // Read the request head + body; we do not need to parse it.
                    let mut buf = vec![0u8; 65536];
                    let _ = s.read(&mut buf).await;
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = s.write_all(resp.as_bytes()).await;
                    let _ = s.flush().await;
                });
            }
        });
        addr
    }

    /// §4c acceptance: **the agent completes a task via a local backend with no
    /// `run_task` change.** The backend is a plain trait impl; `run_task` is untouched.
    #[tokio::test]
    async fn the_agent_completes_a_task_through_a_local_ollama_backend() {
        // Ollama's native reply shape, carrying the agent's JSON action in `content`.
        let addr = spawn_http(
            r#"{"model":"llama3","message":{"role":"assistant","content":"{\"action\":\"finish\",\"answer\":\"the price is 42\"}"},"done":true}"#,
        )
        .await;

        let backend = OllamaBackend::new(&format!("http://{addr}"), "llama3");
        assert_eq!(backend.name(), "ollama:llama3");

        let mut browser = crate::AgentBrowser::new(400, 300);
        browser
            .navigate("data:text/html,<title>Shop</title><body><p>Price: 42</p></body>")
            .await
            .unwrap();

        let cfg = crate::AgentConfig {
            max_steps: 2,
            send_screenshots: false,
            ..crate::AgentConfig::default()
        };
        // The *unchanged* run_task, driven by a local backend.
        let outcome = crate::run_task(&mut browser, &backend, "find the price", &cfg)
            .await
            .unwrap();
        assert_eq!(outcome.answer.as_deref(), Some("the price is 42"));
        assert_eq!(outcome.steps, 1);
    }

    /// The same, through the OpenAI-compatible path (llama.cpp / vLLM / LM Studio).
    #[tokio::test]
    async fn the_agent_completes_a_task_through_an_openai_compatible_local_server() {
        let addr = spawn_http(
            r#"{"choices":[{"message":{"role":"assistant","content":"{\"action\":\"finish\",\"answer\":\"done locally\"}"}}]}"#,
        )
        .await;

        let backend = OpenAiCompatBackend::new(&format!("http://{addr}"), "qwen2.5")
            .with_label("llama.cpp");
        let mut browser = crate::AgentBrowser::new(400, 300);
        browser.navigate("data:text/html,<body>x</body>").await.unwrap();

        let cfg = crate::AgentConfig {
            max_steps: 2,
            send_screenshots: false,
            ..crate::AgentConfig::default()
        };
        let outcome = crate::run_task(&mut browser, &backend, "t", &cfg).await.unwrap();
        assert_eq!(outcome.answer.as_deref(), Some("done locally"));
    }

    /// A non-200 from a local server surfaces the status and body rather than a
    /// mystery parse failure.
    #[tokio::test]
    async fn a_server_error_is_reported_with_status_and_body() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap().to_string();
        tokio::spawn(async move {
            while let Ok((mut s, _)) = l.accept().await {
                let mut b = vec![0u8; 4096];
                let _ = s.read(&mut b).await;
                let body = "model not found";
                let resp = format!(
                    "HTTP/1.1 404 Not Found\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = s.write_all(resp.as_bytes()).await;
            }
        });

        let backend = OllamaBackend::new(&format!("http://{addr}"), "nope");
        let err = backend.complete(&[Message::text(Role::User, "x")]).await.unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("404"), "{msg}");
        assert!(msg.contains("model not found"), "{msg}");
    }

    #[test]
    fn names_identify_the_provider_and_model() {
        assert_eq!(OllamaBackend::new("", "llama3").name(), "ollama:llama3");
        assert_eq!(
            OpenAiCompatBackend::new("http://x", "qwen")
                .with_label("llama.cpp")
                .name(),
            "llama.cpp:qwen"
        );
    }
}
